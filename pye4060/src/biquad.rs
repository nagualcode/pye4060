//! Filter primitives for the analog model (f64 state, TDF-II).
//!
//! Everything runs at the internal oversampled rate (>= 176.4 kHz), where
//! poles down at ~10 Hz sit very close to the unit circle; f64 state keeps
//! them stable and noise-free. All designs are bilinear transforms of the
//! analog prototypes, pre-warped at the section's own corner frequency, so
//! each pole/pair lands exactly where the analog fit put it.

use std::f64::consts::PI;

/// Second-order section, transposed direct form II.
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    s1: f64,
    s2: f64,
}

impl Default for Biquad {
    fn default() -> Self {
        Self::identity()
    }
}

impl Biquad {
    pub const fn identity() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, s1: 0.0, s2: 0.0 }
    }

    fn from_analog(b: [f64; 3], a: [f64; 3], f0: f64, fs: f64) -> Self {
        // H(s) = (b0 s^2 + b1 s + b2) / (a0 s^2 + a1 s + a2), s normalised to
        // w0, bilinear with pre-warp at f0.
        let f0 = f0.clamp(1.0, fs * 0.49);
        let k = 1.0 / (PI * f0 / fs).tan();
        let k2 = k * k;
        let a0 = a[0] * k2 + a[1] * k + a[2];
        let inv = 1.0 / a0;
        Self {
            b0: (b[0] * k2 + b[1] * k + b[2]) * inv,
            b1: (2.0 * b[2] - 2.0 * b[0] * k2) * inv,
            b2: (b[0] * k2 - b[1] * k + b[2]) * inv,
            a1: (2.0 * a[2] - 2.0 * a[0] * k2) * inv,
            a2: (a[0] * k2 - a[1] * k + a[2]) * inv,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// Analog all-pole pair w0^2 / (s^2 + s w0/Q + w0^2).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn low_pass(f0: f64, q: f64, fs: f64) -> Self {
        Self::from_analog([0.0, 0.0, 1.0], [1.0, 1.0 / q, 1.0], f0, fs)
    }

    /// Magnitude-matched version of [`Self::low_pass`] (M. Vicanek, "Matched
    /// Second Order Digital Filters", 2016): impulse-invariant poles, zeros
    /// chosen to match the analog magnitude at DC, f0 and Nyquist. Keeps a
    /// high-Q pair close to the band edge on its analog shape, where the
    /// bilinear transform would distort its bandwidth.
    pub fn low_pass_matched(f0: f64, q: f64, fs: f64) -> Self {
        let w0 = 2.0 * PI * f0.clamp(1.0, fs * 0.49) / fs;
        let zeta = 0.5 / q;
        let r = (-zeta * w0).exp();
        let a1 = if zeta <= 1.0 {
            -2.0 * r * ((1.0 - zeta * zeta).sqrt() * w0).cos()
        } else {
            -2.0 * r * ((zeta * zeta - 1.0).sqrt() * w0).cosh()
        };
        let a2 = (-2.0 * zeta * w0).exp();
        let big_a0 = (1.0 + a1 + a2).powi(2);
        let big_a1 = (1.0 - a1 + a2).powi(2);
        let big_a2 = -4.0 * a2;
        let p1 = (0.5 * w0).sin().powi(2);
        let p0 = 1.0 - p1;
        let p2 = 4.0 * p0 * p1;
        let r1 = (big_a0 * p0 + big_a1 * p1 + big_a2 * p2) * q * q;
        let big_b1 = ((r1 - big_a0 * p0) / p1).max(0.0);
        let b0 = 0.5 * (big_a0.sqrt() + big_b1.sqrt());
        Self { b0, b1: big_a0.sqrt() - b0, b2: 0.0, a1, a2, s1: 0.0, s2: 0.0 }
    }

    /// Analog s^2 / (s^2 + s w0/Q + w0^2).
    pub fn high_pass(f0: f64, q: f64, fs: f64) -> Self {
        Self::from_analog([1.0, 0.0, 0.0], [1.0, 1.0 / q, 1.0], f0, fs)
    }

    /// Analog peaking section, `gain_db` at f0.
    pub fn peaking(f0: f64, gain_db: f64, q: f64, fs: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        Self::from_analog([1.0, a / q, 1.0], [1.0, 1.0 / (a * q), 1.0], f0, fs)
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// First-order section (bilinear, pre-warped).
#[derive(Clone, Copy, Debug, Default)]
pub struct OnePole {
    b0: f64,
    b1: f64,
    a1: f64,
    s: f64,
}

impl OnePole {
    /// w0 / (s + w0)
    pub fn low_pass(f0: f64, fs: f64) -> Self {
        let k = 1.0 / (PI * f0.clamp(0.1, fs * 0.49) / fs).tan();
        let inv = 1.0 / (k + 1.0);
        Self { b0: inv, b1: inv, a1: (1.0 - k) * inv, s: 0.0 }
    }

    /// [`Self::low_pass`] with an exact (impulse-invariant) pole and the
    /// zero placed to match the analog magnitude at DC and Nyquist.
    pub fn low_pass_matched(f0: f64, fs: f64) -> Self {
        let f0 = f0.clamp(0.1, fs * 0.49);
        let a1 = -(-2.0 * PI * f0 / fs).exp();
        let nyq = 1.0 / (1.0 + (0.5 * fs / f0).powi(2)).sqrt();
        let b0 = 0.5 * ((1.0 + a1) + nyq * (1.0 - a1));
        let b1 = 0.5 * ((1.0 + a1) - nyq * (1.0 - a1));
        Self { b0, b1, a1, s: 0.0 }
    }

    /// s / (s + w0)
    pub fn high_pass(f0: f64, fs: f64) -> Self {
        let k = 1.0 / (PI * f0.clamp(0.1, fs * 0.49) / fs).tan();
        let inv = 1.0 / (k + 1.0);
        Self { b0: k * inv, b1: -k * inv, a1: (1.0 - k) * inv, s: 0.0 }
    }

    pub fn reset(&mut self) {
        self.s = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.s;
        self.s = self.b1 * x - self.a1 * y;
        y
    }
}

/// One-pole smoothing coefficient for a time constant `tau` (seconds).
#[inline]
pub fn tau_coeff(tau: f64, fs: f64) -> f64 {
    if tau <= 0.0 { 1.0 } else { 1.0 - (-1.0 / (tau * fs)).exp() }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Least-squares sine fit of the settled half of the response.
    pub(crate) fn fit_amplitude(y: &[f64], freq: f64, fs: f64, start: usize) -> f64 {
        let (mut ss, mut cc, mut sc, mut ys, mut yc) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (i, v) in y.iter().enumerate().skip(start) {
            let ph = 2.0 * PI * freq * i as f64 / fs;
            let (s, c) = ph.sin_cos();
            ss += s * s;
            cc += c * c;
            sc += s * c;
            ys += v * s;
            yc += v * c;
        }
        let det = ss * cc - sc * sc;
        let a = (ys * cc - yc * sc) / det;
        let b = (yc * ss - ys * sc) / det;
        (a * a + b * b).sqrt()
    }

    fn gain_db(mut f: impl FnMut(f64) -> f64, freq: f64, fs: f64) -> f64 {
        let n = (fs * 2.0) as usize;
        let y: Vec<f64> = (0..n).map(|i| f((2.0 * PI * freq * i as f64 / fs).sin())).collect();
        20.0 * fit_amplitude(&y, freq, fs, n / 2).log10()
    }

    #[test]
    fn prewarped_pair_hits_its_q_at_f0() {
        let fs = 192_000.0;
        let mut lp = Biquad::low_pass(15_890.0, 2.796, fs);
        let g = gain_db(|x| lp.process(x), 15_890.0, fs);
        assert!((g - 20.0 * 2.796f64.log10()).abs() < 0.05, "got {g}");
    }

    #[test]
    fn matched_pair_keeps_analog_shape_near_band_edge() {
        // Analog 15.9 kHz / Q 3.07 pair at 18 kHz: |H| = 1/sqrt((1-r^2)^2 + (r/Q)^2).
        let (f0, q, f) = (15_897.0, 3.074, 18_000.0);
        let r: f64 = f / f0;
        let analog = -10.0 * ((1.0 - r * r).powi(2) + (r / q).powi(2)).log10();
        for fs in [176_400.0, 192_000.0] {
            let mut lp = Biquad::low_pass_matched(f0, q, fs);
            let g = gain_db(|x| lp.process(x), f, fs);
            assert!((g - analog).abs() < 0.1, "fs {fs}: {g} vs {analog}");
        }
    }

    #[test]
    fn one_pole_corners_are_minus_3db() {
        let fs = 176_400.0;
        let mut hp = OnePole::high_pass(11.8, fs);
        let mut lp = OnePole::low_pass(22_115.0, fs);
        assert!((gain_db(|x| hp.process(x), 11.8, fs) + 3.01).abs() < 0.05);
        assert!((gain_db(|x| lp.process(x), 22_115.0, fs) + 3.01).abs() < 0.05);
    }
}
