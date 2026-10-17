//! Programme-chain analog stages of the Pye 4060 (circuit 845751/01).
//!
//! Internal units: 1.0 = sine peak at the compression turnover for the
//! dialled threshold. The threshold control is "a pair of opposed
//! attenuators, one preceding the input amplifier and the other controlling
//! the amount of feedback over the output amplifier" (BBC Ops Manual), so
//! the chain between them always works relative to the turnover and the
//! overall gain stays unity.
//!
//! - T1 771273A + coupling: low-frequency corner (the measured response
//!   needs two ~11.8 Hz poles, one each side of the chopper).
//! - VT1 BC113 input amplifier: single class-A stage, wide headroom.
//! - LC anti-carrier filter (L1/L2 52.5 mH, C5-C7 3.3 nF) with T2: fitted
//!   5-pole response plus the T2 leakage resonance near 10.7 kHz;
//!   +0.45 dB at 10 kHz, -0.3 dB at 15 kHz, -6.7 dB at 18 kHz, -12 dB at
//!   20 kHz, ~30 dB/octave beyond (manual: +0.5/-1 dB 30 Hz-15 kHz).
//! - VT3/VT4/VT19 + VT5 post-chopper amplifier: class-A, dominant H2,
//!   saturates about 21 dB above the turnover. It sits inside the threshold
//!   control's feedback network, which keeps leaking signal around the
//!   stage once it clips, so overload rounds off instead of hitting a wall.
//! - VT6-VT9 push-pull driver into T2 772246: symmetric soft saturation and
//!   low-frequency core flux at high output levels (rated +24 dBm).

use crate::biquad::{Biquad, OnePole};

/// Post-chopper amplifier clip point, internal units (~ +21 dB).
const POST_CLIP: f64 = 11.2;
/// Signal passed around the clipped stage by its feedback network.
const POST_LEAK: f64 = 0.16;
/// Second-order coefficient of the post-chopper amplifier: H2 is -64 dB
/// at the turnover and rises 1 dB per dB of level.
const POST_H2: f64 = 0.0012;
/// Input amplifier: tiny H2, and enough headroom that the chopper always
/// gets to act first (no pre-chopper clipping is measurable up to +44 dB).
const PRE_H2: f64 = 0.0001;
const PRE_CLIP: f64 = 300.0;
/// Output stage saturation onset, dBFS-linear (+24 dBm rated with
/// 0 VU = +4 dBu = -18 dBFS puts the hard limit near +2 dBFS; the
/// push-pull pair rounds off progressively above ~+4 dBFS).
const OUT_SAT: f64 = 2.0;
/// Chopper control feedthrough into the programme chain (switch drive
/// coupling through VT2), relative to a full swing of the mark fraction.
const FEEDTHROUGH: f64 = 0.004;

/// Asymmetric soft clipper with a fairly hard knee (|x|^6 law).
#[inline(always)]
fn soft_clip(x: f64, pos: f64, neg: f64) -> f64 {
    let c = if x >= 0.0 { pos } else { neg };
    let r = x / c;
    let r2 = r * r;
    let r6 = r2 * r2 * r2;
    if r2 < 0.09 {
        // Below the knee: second-order binomial expansion, error < 2e-9.
        return x * (1.0 - r6 * ((1.0 / 6.0) - (7.0 / 72.0) * r6));
    }
    x / (1.0 + r6).sqrt().cbrt()
}

/// Feedback amplifier: hard knee, plus the signal its feedback network
/// leaks around the stage once it saturates.
#[inline(always)]
fn soft_clip_leaky(x: f64, pos: f64, neg: f64, leak: f64) -> f64 {
    (1.0 - leak) * soft_clip(x, pos, neg) + leak * x
}

#[derive(Clone, Copy, Debug)]
pub struct ProgrammeChain {
    t1_hp: OnePole,
    lc_real: OnePole,
    lc_pair1: Biquad,
    lc_pair2: Biquad,
    t2_peak: Biquad,
    post_hp: OnePole,
    last_gain: f64,
}

impl Default for ProgrammeChain {
    fn default() -> Self {
        Self::new(192_000.0)
    }
}

impl ProgrammeChain {
    pub fn new(fs: f64) -> Self {
        Self {
            t1_hp: OnePole::high_pass(11.54, fs),
            lc_real: OnePole::low_pass_matched(33_041.0, fs),
            lc_pair1: Biquad::low_pass_matched(9_005.0, 0.710, fs),
            lc_pair2: Biquad::low_pass_matched(15_897.0, 3.074, fs),
            t2_peak: Biquad::peaking(10_739.0, 1.0, 2.41, fs),
            post_hp: OnePole::high_pass(11.54, fs),
            last_gain: 1.0,
        }
    }

    pub fn reset(&mut self) {
        self.t1_hp.reset();
        self.lc_real.reset();
        self.lc_pair1.reset();
        self.lc_pair2.reset();
        self.t2_peak.reset();
        self.post_hp.reset();
        self.last_gain = 1.0;
    }

    /// T1 + input amplifier. Returns the signal arriving at the chopper.
    #[inline(always)]
    pub fn input(&mut self, u: f64) -> f64 {
        let u = self.t1_hp.process(u);
        soft_clip(u + PRE_H2 * u * u, PRE_CLIP, PRE_CLIP * 1.08)
    }

    /// Chopper, LC filter and post-chopper amplifier. `gain` is the mark
    /// fraction for this sample; `noise` is input-referred hiss that the
    /// chopper attenuates along with the programme.
    #[inline(always)]
    pub fn chop(&mut self, u: f64, gain: f64, noise: f64) -> f64 {
        // Switch-drive feedthrough follows changes in the mark fraction;
        // the coupling capacitors pass only the changes.
        let ft = FEEDTHROUGH * (self.last_gain - gain);
        self.last_gain = gain;
        let s = (u + noise) * gain + ft;
        let s = self.lc_pair2.process(self.lc_pair1.process(self.lc_real.process(s)));
        let s = self.t2_peak.process(s);
        // Single-ended Class-A collector stage (VT3-VT5) clips asymmetrically
        // on one rail first; C16a&b (post_hp) AC-couples its output.
        let h2 = POST_H2 * (0.45 + 0.55 * gain * gain);
        let s = soft_clip_leaky(s + h2 * s * s, POST_CLIP, POST_CLIP * 2.4, POST_LEAK);
        self.post_hp.process(s)
    }
}

/// VT6-VT9 push-pull pair and output transformer T2, at output level.
#[derive(Clone, Copy, Debug)]
pub struct OutputStage {
    flux_lp: OnePole,
    dc_hp: OnePole,
}

impl Default for OutputStage {
    fn default() -> Self {
        Self::new(192_000.0)
    }
}

impl OutputStage {
    pub fn new(fs: f64) -> Self {
        Self { flux_lp: OnePole::low_pass(40.0, fs), dc_hp: OnePole::high_pass(2.0, fs) }
    }

    pub fn reset(&mut self) {
        self.flux_lp.reset();
        self.dc_hp.reset();
    }

    #[inline(always)]
    pub fn process(&mut self, y: f64) -> f64 {
        // Core flux is the integral of the winding voltage: only the low
        // end builds enough of it to bend the B-H curve.
        let flux = self.flux_lp.process(y);
        let y = y - 0.01 * flux * flux * flux;
        // Push-pull: symmetric rounding, a touch of imbalance for H2.
        let y = soft_clip(y + 0.002 * y * y, OUT_SAT, OUT_SAT * 1.03);
        self.dc_hp.process(y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalogMode {
    Off,
    Hz50,
    Hz60,
}

/// Power-supply hum and transistor hiss (PS2/49/1742 supply).
#[derive(Clone, Copy, Debug)]
pub struct NoiseSource {
    rng: u32,
    phase: f64,
    pink: [f64; 3],
}

impl NoiseSource {
    pub fn new(seed: u32) -> Self {
        Self { rng: seed | 1, phase: 0.0, pink: [0.0; 3] }
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.pink = [0.0; 3];
    }

    #[inline(always)]
    fn white(&mut self) -> f64 {
        // xorshift32, uniform in [-1, 1): flat spectrum, ~-4.8 dB RMS.
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f64 / 2_147_483_648.0) - 1.0
    }

    /// Hiss with a little 1/f flicker at the low end, unit ~RMS.
    #[inline(always)]
    pub fn hiss(&mut self) -> f64 {
        let w = self.white();
        // Three-pole approximation of a gentle low-frequency rise.
        self.pink[0] = 0.997 * self.pink[0] + 0.029 * w;
        self.pink[1] = 0.985 * self.pink[1] + 0.032 * w;
        self.pink[2] = 0.950 * self.pink[2] + 0.048 * w;
        1.7 * w + self.pink[0] + self.pink[1] + self.pink[2]
    }

    /// Mains hum at the output, dBFS-linear. A full-wave rectified supply
    /// leaves mostly 2x mains ripple; the fundamental leaks magnetically
    /// from the transformer.
    #[inline(always)]
    pub fn hum(&mut self, mode: AnalogMode, fs: f64) -> f64 {
        let (f0, amps) = match mode {
            AnalogMode::Off => return 0.0,
            // 50 Hz British mains: -92 / -86 / -98 / -94 dBFS.
            AnalogMode::Hz50 => (50.0, [2.5e-5, 5.0e-5, 1.26e-5, 2.0e-5]),
            // 60 Hz: the same supply runs a little quieter.
            AnalogMode::Hz60 => (60.0, [1.8e-5, 4.0e-5, 1.0e-5, 1.4e-5]),
        };
        self.phase += std::f64::consts::TAU * f0 / fs;
        if self.phase >= std::f64::consts::TAU {
            self.phase -= std::f64::consts::TAU;
        }
        // Single sin_cos + Chebyshev angle-addition recurrence for 2p, 3p, 4p.
        let (s1, c1) = self.phase.sin_cos();
        let s2 = 2.0 * s1 * c1;
        let c2 = c1 * c1 - s1 * s1;
        let s3 = s2 * c1 + c2 * s1;
        let c3 = c2 * c1 - s2 * s1;
        let s4 = 2.0 * s2 * c2;
        let c4 = c2 * c2 - s2 * s2;

        // Precomputed cos/sin of phase offsets (0.6, 1.1, 2.3 rad).
        const C06: f64 = 0.825_335_614_909_678_3;
        const S06: f64 = 0.564_642_473_395_035_4;
        const C11: f64 = 0.453_596_121_425_577_3;
        const S11: f64 = 0.891_207_360_061_435_4;
        const C23: f64 = -0.666_276_021_279_824_1;
        const S23: f64 = 0.745_705_212_176_720_3;

        amps[0] * s1
            + amps[1] * (s2 * C06 + c2 * S06)
            + amps[2] * (s3 * C11 + c3 * S11)
            + amps[3] * (s4 * C23 + c4 * S23)
    }
}
