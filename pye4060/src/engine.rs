//! Two-channel engine: routing, oversampling, control-voltage linking.
//!
//! Linking follows the hardware. On the dual frame the LINK switch joins
//! PL1 pins 6 and 10 of both units ("for parallel operation of units link
//! all pins 6 and all pins 10"), putting the two C23 timing capacitors and
//! their decay resistors in parallel: both channels then share one control
//! voltage, each detector charging half of the doubled capacitance. Stereo
//! mode is one detector driving both programme chains.

use crate::oversampler::Oversampler;
use crate::pwm_chopper::{ControlCell, ControlCoeffs, detector_target};
use crate::pye_channel::{PyeChannel, TickParams};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Routing {
    DualMono,
    Stereo,
    MidSide,
}

/// Per-frame settings for both channels.
#[derive(Clone, Copy, Debug)]
pub struct FrameParams {
    pub routing: Routing,
    pub link: bool,
    pub tick: [TickParams; 2],
    pub coeffs: [ControlCoeffs; 2],
    /// Output trim, linear.
    pub output: [f64; 2],
    /// Wet fraction, 0..1.
    pub mix: f64,
}

const MAX_OS: usize = Oversampler::MAX_FACTOR;

#[derive(Clone, Copy, Debug)]
pub struct PyeEngine {
    pub channels: [PyeChannel; 2],
    cells: [ControlCell; 2],
    os: [Oversampler; 2],
    dry_os: [Oversampler; 2],
    factor: usize,
    /// Lowest chopper gain seen since the last meter read.
    gain_min: [f64; 2],
}

impl Default for PyeEngine {
    fn default() -> Self {
        Self::new(48_000.0)
    }
}

impl PyeEngine {
    pub fn new(sample_rate: f64) -> Self {
        let factor = Oversampler::factor_for(sample_rate);
        let fs = sample_rate * factor as f64;
        Self {
            channels: [PyeChannel::new(fs, 0x9E37_79B9), PyeChannel::new(fs, 0x85EB_CA6B)],
            cells: [ControlCell::new(fs), ControlCell::new(fs)],
            os: [Oversampler::new(factor); 2],
            dry_os: [Oversampler::new(factor); 2],
            factor,
            gain_min: [1.0; 2],
        }
    }

    pub fn internal_rate(sample_rate: f64) -> f64 {
        sample_rate * Oversampler::factor_for(sample_rate) as f64
    }

    pub fn reset(&mut self) {
        for c in &mut self.channels {
            c.reset();
        }
        for c in &mut self.cells {
            c.reset();
        }
        for o in self.os.iter_mut().chain(self.dry_os.iter_mut()) {
            o.reset();
        }
        self.gain_min = [1.0; 2];
    }

    /// Process one stereo frame (base rate).
    #[inline]
    pub fn process_frame(&mut self, l: f64, r: f64, p: &FrameParams) -> (f64, f64) {
        let (a, b) = match p.routing {
            Routing::MidSide => ((l + r) * std::f64::consts::FRAC_1_SQRT_2, (l - r) * std::f64::consts::FRAC_1_SQRT_2),
            _ => (l, r),
        };

        let mut up = [[0.0; MAX_OS]; 2];
        self.os[0].up(a, &mut up[0]);
        self.os[1].up(b, &mut up[1]);
        let mut wet = [[0.0; MAX_OS]; 2];

        let shared = p.routing == Routing::Stereo || p.link;
        for k in 0..self.factor {
            let g = if shared { [self.cells[0].gain(); 2] } else { [self.cells[0].gain(), self.cells[1].gain()] };
            let (y0, l0) = self.channels[0].tick(up[0][k], g[0], p.coeffs[0].h2_drive, &p.tick[0]);
            let (y1, l1) = self.channels[1].tick(up[1][k], g[1], p.coeffs[1].h2_drive, &p.tick[1]);
            wet[0][k] = y0;
            wet[1][k] = y1;

            let t0 = detector_target(l0, &p.coeffs[0]);
            let t1 = detector_target(l1, &p.coeffs[1]);
            match (p.routing, p.link) {
                (Routing::Stereo, _) => {
                    // One detector fed by the louder channel.
                    let c = self.cells[0].charge(t0.max(t1), &p.coeffs[0]);
                    self.cells[0].step(c, p.coeffs[0].decay_k);
                }
                (_, true) => {
                    // Paralleled C23s: each detector charges 2C; the decay
                    // resistors discharge in parallel.
                    let c = 0.5 * (self.cells[0].charge(t0, &p.coeffs[0]) + self.cells[0].charge(t1, &p.coeffs[1]));
                    self.cells[0].step(c, 0.5 * (p.coeffs[0].decay_k + p.coeffs[1].decay_k));
                }
                (_, false) => {
                    let c0 = self.cells[0].charge(t0, &p.coeffs[0]);
                    let c1 = self.cells[1].charge(t1, &p.coeffs[1]);
                    self.cells[0].step(c0, p.coeffs[0].decay_k);
                    self.cells[1].step(c1, p.coeffs[1].decay_k);
                }
            }
            self.gain_min[0] = self.gain_min[0].min(g[0]);
            self.gain_min[1] = self.gain_min[1].min(g[1]);
        }
        if shared {
            // Keep the idle cell in step so unlinking does not jump.
            self.cells[1] = self.cells[0];
        }

        let ya = self.os[0].down(&wet[0]) * p.output[0];
        let yb = self.os[1].down(&wet[1]) * p.output[1];
        let (mut yl, mut yr) = match p.routing {
            Routing::MidSide => ((ya + yb) * std::f64::consts::FRAC_1_SQRT_2, (ya - yb) * std::f64::consts::FRAC_1_SQRT_2),
            _ => (ya, yb),
        };

        // Dry path through identical resampling filters keeps the blend phase
        // coherent with the wet path; skip when 100% wet.
        if p.mix < 1.0 {
            let mut d = [[0.0; MAX_OS]; 2];
            self.dry_os[0].up(l, &mut d[0]);
            self.dry_os[1].up(r, &mut d[1]);
            let dl = self.dry_os[0].down(&d[0]);
            let dr = self.dry_os[1].down(&d[1]);
            yl = dl + (yl - dl) * p.mix;
            yr = dr + (yr - dr) * p.mix;
        }
        (yl, yr)
    }

    /// Mono frame: channel A only.
    #[inline]
    pub fn process_mono(&mut self, x: f64, p: &FrameParams) -> f64 {
        let mut up = [0.0; MAX_OS];
        self.os[0].up(x, &mut up);
        let mut wet = [0.0; MAX_OS];
        for k in 0..self.factor {
            let g = self.cells[0].gain();
            let (y, lvl) = self.channels[0].tick(up[k], g, p.coeffs[0].h2_drive, &p.tick[0]);
            wet[k] = y;
            let c = self.cells[0].charge(detector_target(lvl, &p.coeffs[0]), &p.coeffs[0]);
            self.cells[0].step(c, p.coeffs[0].decay_k);
            self.gain_min[0] = self.gain_min[0].min(g);
        }
        let y = self.os[0].down(&wet) * p.output[0];
        if p.mix < 1.0 {
            let mut d = [0.0; MAX_OS];
            self.dry_os[0].up(x, &mut d);
            let dry = self.dry_os[0].down(&d);
            dry + (y - dry) * p.mix
        } else {
            y
        }
    }

    /// Peak attenuation per channel since the previous call, dB.
    pub fn take_gr_peak(&mut self) -> [f64; 2] {
        let v = self.gain_min.map(|g| -20.0 * g.log10());
        self.gain_min = [1.0; 2];
        v
    }

    pub fn set_sc_filter(&mut self, f: [crate::pye_channel::ScFilter; 2]) {
        self.channels[0].set_sc_filter(f[0]);
        self.channels[1].set_sc_filter(f[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pwm_chopper::{PyeDecay, PyeRatio};
    use crate::transformer::AnalogMode;
    use std::f64::consts::TAU;

    const SR: f64 = 48_000.0;

    fn params(routing: Routing, link: bool, ratio: PyeRatio, threshold_db: f64) -> FrameParams {
        let fs = PyeEngine::internal_rate(SR);
        let tick = TickParams {
            turnover: 10f64.powf((threshold_db + crate::pye_channel::TURNOVER_REF_DB) / 20.0),
            gate_threshold: 10f64.powf(-30.0 / 20.0),
            gate_atten: 1.0,
            analog: AnalogMode::Off,
        };
        FrameParams {
            routing,
            link,
            tick: [tick; 2],
            coeffs: [ControlCoeffs::new(ratio, PyeDecay::Decay400, fs); 2],
            output: [1.0; 2],
            mix: 1.0,
        }
    }

    fn run(e: &mut PyeEngine, p: &FrameParams, l: impl Fn(f64) -> f64, r: impl Fn(f64) -> f64, secs: f64) -> (Vec<f64>, Vec<f64>) {
        let n = (secs * SR) as usize;
        let (mut yl, mut yr) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let t = i as f64 / SR;
            let (a, b) = e.process_frame(l(t), r(t), p);
            yl.push(a);
            yr.push(b);
        }
        (yl, yr)
    }

    fn amp(y: &[f64], f: f64) -> f64 {
        crate::biquad::tests::fit_amplitude(y, f, SR, y.len() / 2)
    }

    #[test]
    fn unity_gain_below_threshold() {
        let mut e = PyeEngine::new(SR);
        let p = params(Routing::Stereo, true, PyeRatio::Ratio3_1, 0.0);
        let a = 10f64.powf(-40.0 / 20.0);
        let (yl, _) = run(&mut e, &p, |t| a * (TAU * 1000.0 * t).sin(), |t| a * (TAU * 1000.0 * t).sin(), 0.5);
        let g = 20.0 * (amp(&yl, 1000.0) / a).log10();
        assert!(g.abs() < 0.1, "gain {g} dB");
    }

    #[test]
    fn digital_silence_in_silence_out() {
        let mut e = PyeEngine::new(SR);
        let p = params(Routing::DualMono, false, PyeRatio::Limit, -24.0);
        let (yl, yr) = run(&mut e, &p, |_| 0.0, |_| 0.0, 0.2);
        assert!(yl.iter().chain(&yr).all(|v| *v == 0.0));
    }

    #[test]
    fn mid_side_keeps_mono_out_of_the_side() {
        let mut e = PyeEngine::new(SR);
        let p = params(Routing::MidSide, false, PyeRatio::Ratio5_1, -10.0);
        let s = |t: f64| 0.5 * (TAU * 220.0 * t).sin();
        let (yl, yr) = run(&mut e, &p, s, s, 0.5);
        let diff: f64 = yl.iter().zip(&yr).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        assert!(diff < 1e-9, "L-R residue {diff}");
    }

    #[test]
    fn linked_channels_share_one_gain() {
        let mut e = PyeEngine::new(SR);
        let p = params(Routing::DualMono, true, PyeRatio::Ratio3_1, 0.0);
        let loud = |t: f64| 0.8 * (TAU * 1000.0 * t).sin();
        let quiet = |t: f64| 0.01 * (TAU * 1000.0 * t).sin();
        let (_, yr) = run(&mut e, &p, loud, quiet, 0.8);
        // The quiet channel is pulled down by the loud channel's control voltage.
        let g = 20.0 * (amp(&yr, 1000.0) / 0.01).log10();
        assert!(g < -6.0, "linked gain on quiet channel {g} dB");

        let mut e = PyeEngine::new(SR);
        let p = params(Routing::DualMono, false, PyeRatio::Ratio3_1, 0.0);
        let (_, yr) = run(&mut e, &p, loud, quiet, 0.8);
        let g = 20.0 * (amp(&yr, 1000.0) / 0.01).log10();
        assert!(g.abs() < 0.1, "unlinked gain on quiet channel {g} dB");
    }

    #[test]
    fn noise_gate_follows_the_manual() {
        let mut e = PyeEngine::new(SR);
        let mut p = params(Routing::DualMono, false, PyeRatio::Ratio1_1, 0.0);
        p.tick[0].gate_atten = 10f64.powf(-6.0 / 20.0);
        // Turnover at -20 dBFS; gate threshold -30 dB below it => -50 dBFS.
        let low = 10f64.powf(-62.0 / 20.0);
        let (yl, _) = run(&mut e, &p, |t| low * (TAU * 1000.0 * t).sin(), |_| 0.0, 2.0);
        let at = |t0: f64, t1: f64| {
            let s = &yl[(t0 * SR) as usize..(t1 * SR) as usize];
            20.0 * (crate::biquad::tests::fit_amplitude(s, 1000.0, SR, 0) / low).log10()
        };
        assert!(at(0.10, 0.20) > -0.2, "gate closed before the 250 ms hold");
        assert!((at(1.80, 1.95) + 6.0).abs() < 0.5, "settled attenuation {}", at(1.8, 1.95));
    }
}
