//! Side chain and PWM chopper gain law of the Pye 4060 (circuit 845751/01).
//!
//! The 4060 is a *feedback* compressor (manual Fig. 2.4, BBC diagram 2): the
//! side chain samples the programme chain after the chopper and LC filter,
//! so the control loop reacts to its own output.
//!
//! ```text
//!  post-chopper signal -> T4 772201 -> OA47 bridge (MR4-7, full wave)
//!     -> ratio amplifier VT15/VT16 (SWB2/SWB3 resistor sets)
//!     -> C23 22 uF, charged by VT13/VT14, discharged by SWA (R49-R54)
//!     -> VT12 / blocking oscillator T3 -> 250 kHz mark:space pulse
//!     -> VT2 V405 shunt switch: audio passes for the "mark" fraction
//! ```
//!
//! Model, in terms of the normalised C23 voltage `w` (white-box reading of
//! the reference unit's model, see README):
//! - The ratio amplifier is *linear with a bias*: `w_target = k |s| - b`.
//!   SWB2/SWB3 swap resistor sets, i.e. they change the amplifier's gain
//!   and the OA47/VT16 conduction threshold, not an exponent. The
//!   compression ratio emerges from that slope and bias against the
//!   chopper law; so does the 2:1 position's higher knee.
//! - At rest C23 sits at the VT12 bias, `w = 0.76`. The blocking
//!   oscillator turns `w` into a mark fraction through a smooth monotonic
//!   law, `g = exp(P(ln w))` (4th-order fit), unity up to w = 0.767 and
//!   bottoming out at the minimum pulse width, -21.5 dB.
//! - C23 charges through the VT13/VT14 junction, so the charging current
//!   rises expansively with the overdrive: the small top-ups that refill
//!   C23 on every waveform peak are slow (5 ms), large steps are fast and
//!   end up current-limited (the BBC attack spec). C23 decays exponentially
//!   back to the rest bias through the SWA resistor.
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
    /// Ratio amplifier gain `k` and bias `b`: `w_target = k |s| - b`, with
    /// `|s|` the rectified side chain in internal units (1.0 = turnover at
    /// the dialled threshold). The effective turnovers come out at +2.6 dB
    /// (2:1), +0.1 dB (3:1), +0.4 dB (5:1) and +8.2 dB (Limit: "automatically
    /// set 8 dB above the level indicated by the threshold control", manual
    /// ch. 2), and the Limit slope gives "greater than 20:1".
    #[inline]
    pub fn detector(self) -> (f64, f64) {
        match self {
            PyeRatio::Ratio1_1 => (0.0, 0.0),
            PyeRatio::Ratio2_1 => (0.300_402, -0.36),
            PyeRatio::Ratio3_1 => (0.967_369, 0.208),
            PyeRatio::Ratio5_1 => (2.448_843, 1.8),
            PyeRatio::Limit => (6.586_020, 16.2),
        }
    }

    /// C23 charge-current limit, normalised volts per second. Sets the
    /// attack: < 0.5 ms (63 %) in compression and ~1 ms in limit, per the
    /// BBC spec sheet ("Compression less than 0.5 m.secs. Limit 1 m.sec.
    /// +/-0.5"); the limit resistor set passes less charging current.
    #[inline]
    pub fn charge_slew(self) -> f64 {
        match self {
            PyeRatio::Limit => 600.0,
            _ => 3_000.0,
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
    /// C23 decay time constant towards the rest bias for each SWA position.
    ///
    /// The panel is marked 1-32 x100 ms, but those are nominal recovery
    /// times: the resistor ladder (R54 1k8 ... R49 120k against C23 22 uF,
    /// plus the surrounding network and electrolytic tolerance) does not
    /// scale exactly 2:1. These are the reference unit's constants.
    #[inline]
    pub fn tau_seconds(self) -> f64 {
        match self {
            PyeDecay::Decay100 => 0.0585,
            PyeDecay::Decay200 => 0.1064,
            PyeDecay::Decay400 => 0.1707,
            PyeDecay::Decay800 => 0.3604,
            PyeDecay::Decay1600 => 0.8335,
            PyeDecay::Decay3200 => 1.8535,
        }
    }

    /// Side-chain sensitivity of each SWA position relative to 400 ms.
    /// The reference unit's static curve shifts with the decay switch
    /// (+0.2 dB at 100/200 ms, -0.3 dB at 800 ms, +1.5 dB at 3.2 s); the
    /// shape does not follow a simple charge-source loading of C23, so the
    /// measured trims are used as they are.
    #[inline]
    pub fn sc_trim(self) -> f64 {
        match self {
            PyeDecay::Decay100 | PyeDecay::Decay200 => 1.023_29,
            PyeDecay::Decay400 => 1.0,
            PyeDecay::Decay800 => 0.961_61,
            PyeDecay::Decay1600 => 0.991_97,
            PyeDecay::Decay3200 => 1.184_40,
        }
    }

    /// Drive of the post-chopper amplifier's even-order term relative to
    /// 400 ms (the reference unit's H2 rises at the long settings).
    #[inline]
    pub fn h2_drive(self) -> f64 {
        match self {
            PyeDecay::Decay100 | PyeDecay::Decay200 => 0.988_55,
            PyeDecay::Decay400 => 1.0,
            PyeDecay::Decay800 => 1.059_26,
            PyeDecay::Decay1600 => 1.174_90,
            PyeDecay::Decay3200 => 1.621_81,
        }
    }
}

/// Small-signal charging time constant of C23 (VT13/VT14 emitter path).
/// Together with the decay it sets how far C23 sags between waveform
/// peaks, i.e. the static curve at each decay position.
const CHARGE_TAU: f64 = 5.0e-3;
/// Overdrive at which the junction's charging current doubles.
const CHARGE_KNEE: f64 = 0.1;
/// C23 rest voltage (VT12 bias).
const W_REST: f64 = 0.76;
/// Below this the blocking oscillator runs at full mark (g = 1).
const W_UNITY: f64 = 0.767_228_665;
/// Minimum pulse width: the law's input saturates here (-21.4 dB).
const W_MAX: f64 = 3.75;
/// Floor of the mark fraction (-21.5 dB).
const G_MIN: f64 = 0.084_040_152_517_314;
/// Blocking-oscillator law `ln g = P(ln w)`, highest power first.
const LAW: [f64; 5] = [0.293_753_281_193_313, -0.609_876_693_643_099, 0.623_292_474_919_351, -1.879_975_446_705_421, -0.554_692_551_266_39];
/// Maximum attenuation of the chopper (minimum pulse width), in dB.
pub const MAX_GR_DB: f64 = 21.5;

/// Per-sample coefficients derived from the switch settings.
#[derive(Clone, Copy, Debug)]
pub struct ControlCoeffs {
    pub slope: f64,
    pub bias: f64,
    pub decay_k: f64,
    /// Charge-current limit per internal sample.
    pub slew: f64,
    pub h2_drive: f64,
    pub active: bool,
}

impl ControlCoeffs {
    pub fn new(ratio: PyeRatio, decay: PyeDecay, fs: f64) -> Self {
        let (k, b) = ratio.detector();
        Self {
            slope: k * decay.sc_trim(),
            bias: b,
            decay_k: 1.0 - (-1.0 / (decay.tau_seconds() * fs)).exp(),
            slew: ratio.charge_slew() / fs,
            h2_drive: decay.h2_drive(),
            active: !ratio.is_linear(),
        }
    }
}

/// Ratio amplifier: rectified side-chain level -> demanded C23 voltage.
/// `level` is in internal units (1.0 = compression turnover at the dialled
/// threshold). Only demands above the present C23 voltage charge it.
#[inline(always)]
pub fn detector_target(level: f64, c: &ControlCoeffs) -> f64 {
    if !c.active {
        return 0.0;
    }
    c.slope * level - c.bias
}

/// The C23 timing capacitor and the chopper gain it commands.
#[derive(Clone, Copy, Debug)]
pub struct ControlCell {
    w: f64,
    charge_k: f64,
}

impl Default for ControlCell {
    fn default() -> Self {
        Self::new(48_000.0)
    }
}

impl ControlCell {
    pub fn new(fs: f64) -> Self {
        let mut s = Self { w: W_REST, charge_k: 0.0 };
        s.set_sample_rate(fs);
        s
    }

    pub fn set_sample_rate(&mut self, fs: f64) {
        self.charge_k = 1.0 - (-1.0 / (CHARGE_TAU * fs)).exp();
    }

    pub fn reset(&mut self) {
        self.w = W_REST;
    }

    /// Charge increment the detector would push into C23 this sample.
    #[inline(always)]
    pub fn charge(&self, target: f64, c: &ControlCoeffs) -> f64 {
        if target > self.w {
            let e = target - self.w;
            (e * self.charge_k * (1.0 + e / CHARGE_KNEE)).min(c.slew)
        } else {
            0.0
        }
    }

    /// Apply a charge increment and the decay towards the rest bias
    /// (fraction `decay_k`).
    #[inline(always)]
    pub fn step(&mut self, charge: f64, decay_k: f64) {
        self.w += charge;
        self.w -= (self.w - W_REST) * decay_k;
    }

    /// Chopper mark fraction, i.e. the programme-chain gain.
    #[inline(always)]
    pub fn gain(&self) -> f64 {
        if self.w <= W_UNITY {
            return 1.0;
        }
        let u = self.w.min(W_MAX).ln();
        let p = (((LAW[0] * u + LAW[1]) * u + LAW[2]) * u + LAW[3]) * u + LAW[4];
        p.exp().clamp(G_MIN, 1.0)
    }

    pub fn voltage(&self) -> f64 {
        self.w
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
            cell.step(cell.charge(t, &c), c.decay_k);
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
        for (ratio, min_ms, max_ms) in [(PyeRatio::Ratio3_1, 0.0, 0.5), (PyeRatio::Limit, 0.5, 1.5)] {
            let mut x = sine(-10.0, 1000.0, 0.01, fs);
            x.extend(sine(18.0, 1000.0, 0.05, fs));
            let (_, g) = run(ratio, PyeDecay::Decay400, &x, fs);
            let n0 = (0.01 * fs) as usize;
            let final_db = -20.0 * g[g.len() - 1].log10();
            let t63 = g[n0..].iter().position(|&v| -20.0 * v.log10() >= 0.63 * final_db).unwrap();
            let ms = t63 as f64 / fs * 1000.0;
            assert!((min_ms..=max_ms).contains(&ms), "{ratio:?}: 63% after {ms:.2} ms");
        }
    }

    #[test]
    fn attenuation_saturates_near_21_5_db() {
        let fs = 192_000.0;
        let (_, g) = run(PyeRatio::Limit, PyeDecay::Decay400, &sine(40.0, 1000.0, 0.3, fs), fs);
        let gr = -20.0 * g[g.len() - 1].log10();
        assert!((19.0..=MAX_GR_DB).contains(&gr), "max GR {gr}");
    }

    #[test]
    fn rest_is_unity_and_recovers_to_it() {
        let fs = 192_000.0;
        let mut x = sine(12.0, 1000.0, 0.2, fs);
        x.extend(std::iter::repeat_n(0.0, (1.5 * fs) as usize));
        let (_, g) = run(PyeRatio::Ratio3_1, PyeDecay::Decay100, &x, fs);
        assert!(g[(0.19 * fs) as usize] < 0.5);
        assert_eq!(*g.last().unwrap(), 1.0);
    }
}
