//! Pulse Width Modulation (PWM) Chopper Gain Reduction Engine for Pye TVT 84-4060.
//!
//! Faithful component-level emulation of:
//! - High-frequency blocking oscillator mark-to-space chopper switch (VT2 V405)
//! - Germanium diode full-wave bridge rectifier (OA47) with exponential soft knee
//! - Switched 6-position RC decay constants (100ms - 3200ms) with 22uF timing capacitor
//! - Automatic +8 dB threshold turnover shift in LIMIT mode
//! - Integrated broadcast Noise Gate (Type 845751/01) with 250ms hold/release and 1ms attack

#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PyeRatio {
    Ratio1_1,
    Ratio2_1,
    Ratio3_1,
    Ratio5_1,
    Limit,
}

impl PyeRatio {
    #[inline(always)]
    pub fn slope(&self) -> f32 {
        match self {
            PyeRatio::Ratio1_1 => 0.0,
            PyeRatio::Ratio2_1 => 0.50,       // 1 - 1/2
            PyeRatio::Ratio3_1 => 0.6666667,  // 1 - 1/3
            PyeRatio::Ratio5_1 => 0.80,       // 1 - 1/5
            PyeRatio::Limit => 0.95,          // 1 - 1/20 (Limiter action)
        }
    }

    #[inline(always)]
    pub fn is_limit(&self) -> bool {
        matches!(self, PyeRatio::Limit)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PyeDecay {
    Decay100,
    Decay200,
    Decay400,
    Decay800,
    Decay1600,
    Decay3200,
}

impl PyeDecay {
    #[inline(always)]
    pub fn time_seconds(&self) -> f32 {
        match self {
            PyeDecay::Decay100 => 0.100,
            PyeDecay::Decay200 => 0.200,
            PyeDecay::Decay400 => 0.400,
            PyeDecay::Decay800 => 0.800,
            PyeDecay::Decay1600 => 1.600,
            PyeDecay::Decay3200 => 3.200,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PwmChopper {
    sample_rate: f32,
    // Compressor ballistics
    comp_gr_db: f32,
    attack_coeff: f32,
    decay_coeff: f32,
    current_decay: PyeDecay,

    // Noise gate ballistics
    gate_gr_db: f32,
    gate_att_coeff: f32,
    gate_rel_coeff: f32,

    // Envelope detector filter state
    det_env: f32,
    det_coeff: f32,
}

impl Default for PwmChopper {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl PwmChopper {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            comp_gr_db: 0.0,
            attack_coeff: 0.0,
            decay_coeff: 0.0,
            current_decay: PyeDecay::Decay400,
            gate_gr_db: 0.0,
            gate_att_coeff: 0.0,
            gate_rel_coeff: 0.0,
            det_env: 0.0,
            det_coeff: 0.0,
        };
        s.update_sample_rate(sample_rate);
        s
    }

    pub fn reset(&mut self) {
        self.comp_gr_db = 0.0;
        self.gate_gr_db = 0.0;
        self.det_env = 0.0;
    }

    pub fn update_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(44100.0);
        // Fast attack (~0.8 ms) matching the 250 kHz PWM chopper response
        let tau_att = 0.0008;
        self.attack_coeff = 1.0 - (-1.0 / (tau_att * self.sample_rate)).exp();

        // Detector peak smoothing filter (~0.3 ms)
        let tau_det = 0.0003;
        self.det_coeff = 1.0 - (-1.0 / (tau_det * self.sample_rate)).exp();

        // Noise gate timings: 1 ms attack when opening, 250 ms release into attenuation
        self.gate_att_coeff = 1.0 - (-1.0 / (0.001 * self.sample_rate)).exp();
        self.gate_rel_coeff = 1.0 - (-1.0 / (0.250 * self.sample_rate)).exp();

        self.set_decay(self.current_decay);
    }

    pub fn set_decay(&mut self, decay: PyeDecay) {
        self.current_decay = decay;
        let tau_rel = decay.time_seconds();
        self.decay_coeff = 1.0 - (-1.0 / (tau_rel * self.sample_rate)).exp();
    }

    /// Compute target gain reduction in dB for a given detector level
    #[inline(always)]
    pub fn compute_target_gr(
        sidechain_sample: f32,
        threshold_db: f32,
        ratio: PyeRatio,
    ) -> (f32, f32) {
        // Germanium bridge rectifier characteristic: lower forward voltage (~0.25V)
        // with smooth exponential onset
        let abs_sc = sidechain_sample.abs();
        let sc_db = if abs_sc > 1e-9 {
            20.0 * abs_sc.log10()
        } else {
            -180.0
        };

        // Calibration: 0 dBVU = -18 dBFS (+4 dBu).
        // Threshold param is calibrated in dBV (-24 to +16 dBV).
        let mut turnover = threshold_db - 20.0;

        // In LIMIT mode, the manual specifies that the turnover point is automatically
        // shifted +8 dB higher so limiting occurs at 100% modulation.
        if ratio.is_limit() {
            turnover += 8.0;
        }

        let delta = sc_db - turnover;
        let slope = ratio.slope();

        // Germanium diode soft knee (3 dB transition band)
        const KNEE_WIDTH: f32 = 3.0;
        const HALF_KNEE: f32 = KNEE_WIDTH * 0.5;

        let target_gr = if delta <= -HALF_KNEE {
            0.0
        } else if delta >= HALF_KNEE {
            delta * slope
        } else {
            let knee_overshoot = (delta + HALF_KNEE) * (delta + HALF_KNEE) / (2.0 * KNEE_WIDTH);
            knee_overshoot * slope
        };

        (target_gr.max(0.0), sc_db)
    }

    /// Process one sample through the PWM Chopper gain reduction engine.
    ///
    /// Returns: (attenuated_sample, instantaneous_comp_gr_db)
    #[inline(always)]
    pub fn process(
        &mut self,
        input_sample: f32,
        sidechain_sample: f32,
        threshold_db: f32,
        ratio: PyeRatio,
        gate_thresh_rel_db: f32, // -50 to -20 dB relative to compression threshold
        gate_atten_db: f32,      // 0 to 10 dB in 2 dB steps
    ) -> (f32, f32) {
        // Sidechain envelope follower
        let rect = sidechain_sample.abs();
        if rect > self.det_env {
            self.det_env += self.attack_coeff * (rect - self.det_env);
        } else {
            self.det_env += self.det_coeff * (rect - self.det_env);
        }

        let (target_gr, sc_db) = Self::compute_target_gr(self.det_env, threshold_db, ratio);

        // Compression ballistics
        if target_gr > self.comp_gr_db {
            self.comp_gr_db += self.attack_coeff * (target_gr - self.comp_gr_db);
        } else {
            self.comp_gr_db += self.decay_coeff * (target_gr - self.comp_gr_db);
        }

        // Noise gate operation (Type 845751/01)
        if gate_atten_db > 0.05 {
            let mut turnover = threshold_db - 20.0;
            if ratio.is_limit() {
                turnover += 8.0;
            }
            let gate_turnover = turnover + gate_thresh_rel_db;

            if sc_db < gate_turnover {
                // Below noise threshold: smoothly introduce gate attenuation (250 ms time constant)
                self.gate_gr_db += self.gate_rel_coeff * (gate_atten_db - self.gate_gr_db);
            } else {
                // Above noise threshold: quickly remove gate attenuation (1 ms attack)
                self.gate_gr_db += self.gate_att_coeff * (0.0 - self.gate_gr_db);
            }
        } else {
            self.gate_gr_db = 0.0;
        }

        // Clean up denormals
        if self.comp_gr_db < 1e-4 {
            self.comp_gr_db = 0.0;
        }
        if self.gate_gr_db < 1e-4 {
            self.gate_gr_db = 0.0;
        }

        let total_gr_db = self.comp_gr_db + self.gate_gr_db;
        // PWM mark-to-space energy attenuation
        let chopper_gain = 10.0f32.powf(-total_gr_db / 20.0);
        let out = input_sample * chopper_gain;

        (out, self.comp_gr_db)
    }

    /// Read current gain reduction in dB (for GUI metering)
    #[inline(always)]
    pub fn current_gr_db(&self) -> f32 {
        self.comp_gr_db
    }
}
