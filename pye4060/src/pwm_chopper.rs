//! Side chain and PWM chopper gain law of the Pye 4060 (circuit 845751/01).
//!
//! The 4060 is a *feedback* compressor (manual Fig. 2.4, BBC diagram 2): the
//! side chain samples the programme chain after the chopper, LC filter and
//! amplifier, so the control loop reacts to its own output.
//!
//! ```text
//!  post-chopper signal -> T4 772201 -> OA47 bridge (MR4-7, full wave)
//!     -> ratio amplifier VT15/VT16 (SWB2/SWB3 resistor sets)
//!     -> C23 22 uF, charged by VT13/VT14, discharged by SWA (R49-R54)
//!     -> VT12 / blocking oscillator T3 -> 250 kHz mark:space pulse
//!     -> VT2 V405 shunt switch: audio passes for the "mark" fraction
//! ```
//!
//! Model, in terms of the normalised C23 voltage `V`:
//! - Chopper gain is hyperbolic, `g = 1 / (1 + V)`, i.e. the mark:space
//!   ratio shrinks as the control voltage rises. With an exponential RC
//!   discharge this law reproduces the measured recovery shape (near-linear
//!   in dB while deep, exponential tail near unity) to 0.07 dB RMS at every
//!   depth with a single time constant, where a dB-domain law does not.
//! - The ratio amplifier produces a power law of the rectified output,
//!   `V_target = (|s| / turnover)^n - 1`, n = ratio - 1. In the loop this
//!   gives exactly 20 log10(1 + V) = n (L_out - L_turnover), the constant
//!   compression ratios of manual Fig. 2.2.
//! - C23 charges through a transistor stage: proportional (tau_a) with a
//!   current limit (slew), and discharges through the decay resistor.
//! - The blocking oscillator has a minimum pulse width: attenuation tops out
//!   around 21.5 dB, approached softly.
//!
//! Because the gain is applied inside the loop, ripple on C23 between
//! waveform peaks modulates the audio at audio rate. That is the 4060's
//! harmonic signature (manual Fig. 2.5): odd-order, rising at low
//! frequencies and with short decay settings.

#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyeRatio {
    Ratio1_1,
    Ratio2_1,
    Ratio3_1,
    Ratio5_1,
    Limit,
}

impl PyeRatio {
    /// Exponent of the ratio amplifier (compression ratio minus one).
    /// Limit: the manual specifies "greater than 20:1"; 25:1 matches the
    /// measured limiting slope.
    #[inline]
    pub fn exponent(self) -> f64 {
        match self {
            PyeRatio::Ratio1_1 => 0.0,
            PyeRatio::Ratio2_1 => 1.0,
            PyeRatio::Ratio3_1 => 2.0,
            PyeRatio::Ratio5_1 => 4.0,
            PyeRatio::Limit => 24.0,
        }
    }

    /// Turnover relative to the threshold dial. In the limit position the
    /// turnover is "automatically set 8 dB above the level indicated by the
    /// threshold control" (manual ch. 2, Fig. 2.2). In the 2:1 position the
    /// lower SWB2 shunt resistor (R62 910R vs R61 2k7 / R60 8k2) shifts the
    /// effective OA47/VT16 conduction threshold up by ~1.6 dB.
    #[inline]
    pub fn turnover_offset_db(self) -> f64 {
        match self {
            PyeRatio::Limit => 8.0,
            PyeRatio::Ratio2_1 => 1.6,
            _ => 0.0,
        }
    }

    /// Exact integer power `r^(ratio - 1)` without transcendental `powf`.
    #[inline(always)]
    pub fn pow_ratio(self, r: f64) -> f64 {
        match self {
            PyeRatio::Ratio1_1 => 1.0,
            PyeRatio::Ratio2_1 => r,
            PyeRatio::Ratio3_1 => r * r,
            PyeRatio::Ratio5_1 => {
                let r2 = r * r;
                r2 * r2
            }
            PyeRatio::Limit => {
                let r2 = r * r;
                let r4 = r2 * r2;
                let r8 = r4 * r4;
                r8 * r8 * r8
            }
        }
    }

    /// SWB1 opens the side chain in the linear (1:1) position.
    #[inline]
    pub fn is_linear(self) -> bool {
        self == PyeRatio::Ratio1_1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyeDecay {
    Decay100,
    Decay200,
    Decay400,
    Decay800,
    Decay1600,
    Decay3200,
}

impl PyeDecay {
    /// C23 discharge time constant for each SWA position.
    ///
    /// The panel is marked 1-32 x100 ms, but those are nominal recovery
    /// times: the resistor ladder (R54 1k8 ... R49 120k against C23 22 uF,
    /// plus the surrounding network and electrolytic tolerance) does not
    /// scale exactly 2:1. These are the time constants fitted to the
    /// reference unit's recovery curves under the hyperbolic gain law.
    #[inline]
    pub fn tau_seconds(self) -> f64 {
        match self {
            PyeDecay::Decay100 => 0.053,
            PyeDecay::Decay200 => 0.096,
            PyeDecay::Decay400 => 0.155,
            PyeDecay::Decay800 => 0.327,
            PyeDecay::Decay1600 => 0.785,
            PyeDecay::Decay3200 => 1.685,
        }
    }
}

/// Proportional charging time constant of C23 (VT13/VT14 emitter path).
const CHARGE_TAU: f64 = 0.4e-3;
/// Charge-current limit, in normalised volts per second. Sets the attack:
/// < 0.5 ms (63 %) in compression and ~1 ms in limit, per the BBC spec
/// sheet ("Compression less than 0.5 m.secs. Limit 1 m.sec. +/-0.5").
const CHARGE_SLEW: f64 = 4_500.0;
/// The ratio amplifier saturates against its supply.
const TARGET_MAX: f64 = 14.0;
/// Maximum attenuation of the chopper (minimum pulse width), in dB.
pub const MAX_GR_DB: f64 = 21.5;
/// Germanium bridge: the OA47 knee rounds the turnover over ~1 dB.
const KNEE: f64 = 0.06;

/// Per-sample coefficients derived from the switch settings.
#[derive(Clone, Copy, Debug)]
pub struct ControlCoeffs {
    pub ratio: PyeRatio,
    pub exponent: f64,
    pub inv_turnover: f64,
    pub decay_k: f64,
    pub active: bool,
}

impl ControlCoeffs {
    pub fn new(ratio: PyeRatio, decay: PyeDecay, fs: f64) -> Self {
        Self {
            ratio,
            exponent: ratio.exponent(),
            inv_turnover: 10f64.powf(-ratio.turnover_offset_db() / 20.0),
            decay_k: 1.0 - (-1.0 / (decay.tau_seconds() * fs)).exp(),
            active: !ratio.is_linear(),
        }
    }
}

/// Ratio amplifier: rectified post-chopper level -> demanded C23 voltage.
/// `level` is the full-wave rectified side-chain signal in internal units
/// (1.0 = compression turnover at the dialled threshold).
#[inline(always)]
pub fn detector_target(level: f64, c: &ControlCoeffs) -> f64 {
    if !c.active {
        return 0.0;
    }
    let r = level * c.inv_turnover;
    if r <= 0.9 {
        return 0.0;
    }
    let e = c.ratio.pow_ratio(r) - 1.0;
    // Smooth max(e, 0): the diode bridge does not switch on abruptly.
    let k = KNEE * c.exponent;
    let v = 0.5 * (e + (e * e + k * k).sqrt()) - 0.5 * k;
    v.clamp(0.0, TARGET_MAX)
}

/// The C23 timing capacitor and the chopper gain it commands.
#[derive(Clone, Copy, Debug)]
pub struct ControlCell {
    v: f64,
    charge_k: f64,
    slew: f64,
    v_max: f64,
}

impl Default for ControlCell {
    fn default() -> Self {
        Self::new(48_000.0)
    }
}

impl ControlCell {
    pub fn new(fs: f64) -> Self {
        let g_min = 10f64.powf(-MAX_GR_DB / 20.0);
        let mut s = Self { v: 0.0, charge_k: 0.0, slew: 0.0, v_max: 1.0 / g_min - 1.0 };
        s.set_sample_rate(fs);
        s
    }

    pub fn set_sample_rate(&mut self, fs: f64) {
        self.charge_k = 1.0 - (-1.0 / (CHARGE_TAU * fs)).exp();
        self.slew = CHARGE_SLEW / fs;
    }

    pub fn reset(&mut self) {
        self.v = 0.0;
    }

    /// Charge increment the detector would push into C23 this sample.
    #[inline(always)]
    pub fn charge(&self, target: f64) -> f64 {
        if target > self.v {
            ((target - self.v) * self.charge_k).min(self.slew)
        } else {
            0.0
        }
    }

    /// Apply a charge increment and the decay discharge (fraction `decay_k`).
    #[inline(always)]
    pub fn step(&mut self, charge: f64, decay_k: f64) {
        self.v += charge;
        self.v -= self.v * decay_k;
        if self.v < 1e-9 {
            self.v = 0.0;
        }
    }

    /// Chopper mark fraction, i.e. the programme-chain gain.
    #[inline(always)]
    pub fn gain(&self) -> f64 {
        // Minimum pulse width: the effective voltage saturates softly at v_max.
        let v = self.v_max * (self.v / self.v_max).tanh();
        1.0 / (1.0 + v)
    }

    pub fn voltage(&self) -> f64 {
        self.v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Closed loop at the internal rate: y = g * x, detector on |y|.
    fn run(ratio: PyeRatio, decay: PyeDecay, x: &[f64], fs: f64) -> (Vec<f64>, Vec<f64>) {
        let c = ControlCoeffs::new(ratio, decay, fs);
        let mut cell = ControlCell::new(fs);
        let mut y = Vec::with_capacity(x.len());
        let mut g = Vec::with_capacity(x.len());
        for &s in x {
            let out = s * cell.gain();
            let t = detector_target(out.abs(), &c);
            cell.step(cell.charge(t), c.decay_k);
            y.push(out);
            g.push(cell.gain());
        }
        (y, g)
    }

    fn sine(db: f64, f: f64, secs: f64, fs: f64) -> Vec<f64> {
        let a = 10f64.powf(db / 20.0);
        (0..(secs * fs) as usize).map(|i| a * (std::f64::consts::TAU * f * i as f64 / fs).sin()).collect()
    }

    fn peak_db(v: &[f64]) -> f64 {
        20.0 * v.iter().fold(0.0f64, |m, s| m.max(s.abs())).log10()
    }

    #[test]
    fn static_ratios_follow_fig_2_2() {
        let fs = 192_000.0;
        for (ratio, r) in [(PyeRatio::Ratio2_1, 2.0), (PyeRatio::Ratio3_1, 3.0), (PyeRatio::Ratio5_1, 5.0)] {
            let lo = peak_db(&run(ratio, PyeDecay::Decay3200, &sine(6.0, 1000.0, 1.0, fs), fs).0[150_000..]);
            let hi = peak_db(&run(ratio, PyeDecay::Decay3200, &sine(12.0, 1000.0, 1.0, fs), fs).0[150_000..]);
            let measured = 6.0 / (hi - lo);
            assert!((measured - r).abs() < 0.2 * r, "{ratio:?}: {measured:.2}:1");
        }
    }

    #[test]
    fn limit_turnover_is_8db_up() {
        let fs = 192_000.0;
        // 6 dB over the dial: below the limit turnover, untouched.
        let y = run(PyeRatio::Limit, PyeDecay::Decay400, &sine(6.0, 1000.0, 0.5, fs), fs).0;
        assert!(peak_db(&y[60_000..]) > 5.8);
        // 20 dB over the dial: held to ~+8.5 dB.
        let y = run(PyeRatio::Limit, PyeDecay::Decay400, &sine(20.0, 1000.0, 0.5, fs), fs).0;
        let p = peak_db(&y[60_000..]);
        assert!((7.8..9.5).contains(&p), "limit output {p}");
    }

    #[test]
    fn attack_meets_bbc_spec() {
        let fs = 192_000.0;
        for (ratio, max_ms) in [(PyeRatio::Ratio3_1, 0.5), (PyeRatio::Limit, 1.5)] {
            let mut x = sine(-10.0, 1000.0, 0.01, fs);
            x.extend(sine(18.0, 1000.0, 0.05, fs));
            let (_, g) = run(ratio, PyeDecay::Decay400, &x, fs);
            let n0 = (0.01 * fs) as usize;
            let final_db = -20.0 * g[g.len() - 1].log10();
            let t63 = g[n0..].iter().position(|&v| -20.0 * v.log10() >= 0.63 * final_db).unwrap();
            let ms = t63 as f64 / fs * 1000.0;
            assert!(ms <= max_ms, "{ratio:?}: 63% after {ms:.2} ms");
        }
    }

    #[test]
    fn attenuation_saturates_near_21_5_db() {
        let fs = 192_000.0;
        let (_, g) = run(PyeRatio::Limit, PyeDecay::Decay400, &sine(40.0, 1000.0, 0.3, fs), fs);
        let gr = -20.0 * g[g.len() - 1].log10();
        assert!((19.0..=MAX_GR_DB).contains(&gr), "max GR {gr}");
    }
}
