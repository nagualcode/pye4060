//! Transformer & Discrete Transistor Stage Physics for Pye TVT 84-4060.
//!
//! Models:
//! - Input Transformer T1 771273A (LF phase rolloff & HF air resonance)
//! - 5-Pole LC Low-Pass Filter (L1, L2 52.5mH, C5-C7 0.0033uF) stripping carrier above 18 kHz
//! - Discrete Push-Pull Output Driver (VT9 2N4062, VT10 2N3053) & Output Transformer T2 772246
//! - Authentic 1/f magnetic flux weighting and Class-A even-harmonic (H2) asymmetry
//! - Defeatable British (50 Hz) and US (60 Hz) power-rail analog hum modeling

use crate::biquad::{Biquad, Coefficients};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnalogMode {
    Off,
    Hz50,
    Hz60,
}

#[derive(Clone, Copy, Debug)]
pub struct TransformerStage {
    sample_rate: f32,

    // Input transformer T1 filters
    t1_hp: Biquad,
    t1_air: Biquad,

    // Sidechain transformer T4 filters (completely isolated from audio path)
    t4_hp: Biquad,
    t4_air: Biquad,

    // 5-Pole LC Low-Pass Filter
    lc_stage1: Biquad,
    lc_stage2: Biquad,
    lc_pole_s: f32,
    lc_pole_a: f32,

    // Discrete output stage flux weighting
    flux_pre: Biquad,
    flux_post: Biquad,
    dc_block: Biquad,

    // Analog hum generator phase
    hum_phase: f32,
}

impl Default for TransformerStage {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl TransformerStage {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            t1_hp: Biquad::default(),
            t1_air: Biquad::default(),
            t4_hp: Biquad::default(),
            t4_air: Biquad::default(),
            lc_stage1: Biquad::default(),
            lc_stage2: Biquad::default(),
            lc_pole_s: 0.0,
            lc_pole_a: 0.0,
            flux_pre: Biquad::default(),
            flux_post: Biquad::default(),
            dc_block: Biquad::default(),
            hum_phase: 0.0,
        };
        s.update_sample_rate(sample_rate, sample_rate * 2.0);
        s
    }

    pub fn reset(&mut self) {
        self.t1_hp.reset();
        self.t1_air.reset();
        self.t4_hp.reset();
        self.t4_air.reset();
        self.lc_stage1.reset();
        self.lc_stage2.reset();
        self.lc_pole_s = 0.0;
        self.flux_pre.reset();
        self.flux_post.reset();
        self.dc_block.reset();
        self.hum_phase = 0.0;
    }

    pub fn update_sample_rate(&mut self, base_sr: f32, oversampled_sr: f32) {
        let base_sr = base_sr.max(44100.0);
        let over_sr = oversampled_sr.max(88200.0);
        self.sample_rate = base_sr;

        // T1 input transformer (base_sr):
        // - Subsonic rolloff (-2.3 dB @ 20 Hz, -0.3 dB @ 50 Hz, -0.02 dB @ 100 Hz)
        self.t1_hp = Biquad::new(Coefficients::low_shelf(30.0, -2.7, 0.707, base_sr));
        // - Subtle 10 kHz transformer air lift (+0.4 dB)
        self.t1_air = Biquad::new(Coefficients::peaking(9500.0, 0.45, 0.8, base_sr));

        // T4 sidechain transformer (base_sr):
        self.t4_hp = Biquad::new(Coefficients::low_shelf(30.0, -2.7, 0.707, base_sr));
        self.t4_air = Biquad::new(Coefficients::peaking(9500.0, 0.45, 0.8, base_sr));

        // 5-Pole LC Anti-Carrier Filter (base_sr):
        // Conforms to Pye manual: +0.5 to -1.0 dB (30 Hz to 15 kHz) with sharp LC rolloff above 16 kHz
        let fc1 = (18500.0f32).min(base_sr * 0.45);
        let fc2 = (20500.0f32).min(base_sr * 0.47);
        self.lc_stage1 = Biquad::new(Coefficients::low_pass(fc1, 0.707, base_sr));
        self.lc_stage2 = Biquad::new(Coefficients::low_pass(fc2, 0.707, base_sr));

        // 1st order pole at 21 kHz
        let fc3 = (21000.0f32).min(base_sr * 0.48);
        let w_pole = 2.0 * std::f32::consts::PI * fc3 / base_sr;
        self.lc_pole_a = (-w_pole).exp();

        // Output transformer T2 1/f flux weighting (oversampled_sr)
        let flux_pre_coeffs = Coefficients::low_shelf(120.0, 2.5, 0.707, over_sr);
        let flux_post_coeffs = flux_pre_coeffs.invert();
        self.flux_pre = Biquad::new(flux_pre_coeffs);
        self.flux_post = Biquad::new(flux_post_coeffs);

        // Subsonic DC block (5 Hz, oversampled_sr)
        self.dc_block = Biquad::new(Coefficients::high_pass(5.0, 0.707, over_sr));
    }

    /// Process input signal through Input Transformer T1 conditioning
    #[inline(always)]
    pub fn process_input_transformer(&mut self, x: f32) -> f32 {
        let y1 = self.t1_hp.process(x);
        self.t1_air.process(y1)
    }

    /// Process sidechain signal through Sidechain Transformer T4 conditioning
    #[inline(always)]
    pub fn process_sidechain_transformer(&mut self, x: f32) -> f32 {
        let y1 = self.t4_hp.process(x);
        self.t4_air.process(y1)
    }

    /// Process chopped signal through 5-Pole LC Low-Pass Filter
    #[inline(always)]
    pub fn process_lc_filter(&mut self, x: f32) -> f32 {
        let y1 = self.lc_stage1.process(x);
        let y2 = self.lc_stage2.process(y1);

        // 1st order single pole
        self.lc_pole_s = (1.0 - self.lc_pole_a) * y2 + self.lc_pole_a * self.lc_pole_s;
        if self.lc_pole_s.abs() < 1e-25 {
            self.lc_pole_s = 0.0;
        }
        self.lc_pole_s
    }

    /// Process through Discrete Class-AB Push-Pull Output Stage & Transformer T2
    /// with authentic Class-A H2 asymmetry and magnetic flux compression
    #[inline(always)]
    pub fn process_output_saturation(&mut self, x: f32) -> f32 {
        // Pre-emphasis: low frequencies undergo higher magnetic flux
        let x_flux = self.flux_pre.process(x);

        // Discrete transistor Class-AB / Class-A asymmetry non-linearity
        // Generates dominant musical 2nd harmonic (H2) and subtle 3rd harmonic (H3)
        const ALPHA_EVEN: f32 = 0.0055;
        const BETA_ODD: f32 = 0.0018;

        let abs_x = x_flux.abs();
        let even_term = ALPHA_EVEN * (x_flux * x_flux) / (1.0 + abs_x);
        let odd_term = BETA_ODD * (x_flux * x_flux * x_flux);
        let y_sat = x_flux - even_term - odd_term;

        // Post-emphasis: exact mathematical inverse restores flat frequency response
        // at linear levels while preserving low-end saturation bloom at high levels
        let y_post = self.flux_post.process(y_sat);

        // Subsonic DC block removes any DC shift from the even-order rectification
        self.dc_block.process(y_post)
    }

    /// Compute power supply analog hum sample
    #[inline(always)]
    pub fn generate_analog_hum(&mut self, mode: AnalogMode) -> f32 {
        match mode {
            AnalogMode::Off => 0.0,
            AnalogMode::Hz50 => {
                let f0 = 50.0;
                let inc = 2.0 * std::f32::consts::PI * f0 / self.sample_rate;
                self.hum_phase += inc;
                if self.hum_phase >= 2.0 * std::f32::consts::PI {
                    self.hum_phase -= 2.0 * std::f32::consts::PI;
                }
                // Fundamental + 2nd harmonic (100 Hz) + 3rd harmonic (150 Hz) at -86 dBFS
                const HUM_AMP: f32 = 0.00005; // -86 dBFS
                HUM_AMP * (self.hum_phase.sin() + 0.5 * (2.0 * self.hum_phase).sin() + 0.25 * (3.0 * self.hum_phase).sin())
            }
            AnalogMode::Hz60 => {
                let f0 = 60.0;
                let inc = 2.0 * std::f32::consts::PI * f0 / self.sample_rate;
                self.hum_phase += inc;
                if self.hum_phase >= 2.0 * std::f32::consts::PI {
                    self.hum_phase -= 2.0 * std::f32::consts::PI;
                }
                // Fundamental + 2nd harmonic (120 Hz) + 3rd harmonic (180 Hz) at -86 dBFS
                const HUM_AMP: f32 = 0.00005; // -86 dBFS
                HUM_AMP * (self.hum_phase.sin() + 0.5 * (2.0 * self.hum_phase).sin() + 0.25 * (3.0 * self.hum_phase).sin())
            }
        }
    }
}
