//! Polyphase IIR half-band resampling (elliptic allpass pairs, hiir topology).
//!
//! The analog model runs at >= 176.4 kHz: the chopper multiplies the audio by
//! a gain that ripples at audio rate, and the amplifier/transformer stages
//! clip, so their products must be computed far above the audio band and
//! band-limited before decimation.
//!
//! Stage 1 (1x <-> 2x): 8 coefficients, transition 0.0416 -> flat to
//! 20.2 kHz at 44.1 kHz with ~100 dB image/alias rejection.
//! Stage 2 (2x <-> 4x): 4 coefficients, transition 0.25 -> ~117 dB.
//! Coefficients from Laurent de Soras' elliptic half-band design
//! (PolyphaseIir2Designer::compute_coefs_spec_order_tbw).

const STAGE1: [f64; 8] = [
    0.039_766_276_498,
    0.147_561_814_372,
    0.295_664_048_183,
    0.454_393_312_402,
    0.603_052_666_074,
    0.733_010_476_550,
    0.845_497_193_283,
    0.948_366_169_252,
];

const STAGE2: [f64; 4] = [0.042_454_709_865, 0.170_739_850_498, 0.393_319_893_190, 0.745_713_588_720];

/// Two interleaved allpass chains: even coefficients form path 0, odd
/// coefficients path 1. Each section is (a + z^-1) / (1 + a z^-1) at the
/// low rate.
#[derive(Clone, Copy, Debug)]
struct AllpassPair<const N: usize> {
    coef: [f64; N],
    x: [f64; N],
    y: [f64; N],
}

impl<const N: usize> AllpassPair<N> {
    const fn new(coef: [f64; N]) -> Self {
        Self { coef, x: [0.0; N], y: [0.0; N] }
    }

    fn reset(&mut self) {
        self.x = [0.0; N];
        self.y = [0.0; N];
    }

    #[inline(always)]
    fn process(&mut self, mut s0: f64, mut s1: f64) -> (f64, f64) {
        let mut i = 0;
        while i + 1 < N {
            let t0 = (s0 - self.y[i]) * self.coef[i] + self.x[i];
            self.x[i] = s0;
            self.y[i] = t0;
            s0 = t0;
            let t1 = (s1 - self.y[i + 1]) * self.coef[i + 1] + self.x[i + 1];
            self.x[i + 1] = s1;
            self.y[i + 1] = t1;
            s1 = t1;
            i += 2;
        }
        (s0, s1)
    }

    /// One low-rate input -> two high-rate outputs (in time order).
    #[inline(always)]
    fn up(&mut self, x: f64) -> (f64, f64) {
        self.process(x, x)
    }

    /// Two high-rate inputs (in time order) -> one low-rate output. The
    /// later sample feeds path 0, the earlier one path 1.
    #[inline(always)]
    fn down(&mut self, first: f64, second: f64) -> f64 {
        let (a, b) = self.process(second, first);
        0.5 * (a + b)
    }
}

/// Up/down converter for one audio channel, factor 1, 2 or 4.
#[derive(Clone, Copy, Debug)]
pub struct Oversampler {
    factor: usize,
    up1: AllpassPair<8>,
    up2: AllpassPair<4>,
    dn1: AllpassPair<8>,
    dn2: AllpassPair<4>,
}

impl Default for Oversampler {
    fn default() -> Self {
        Self::new(4)
    }
}

impl Oversampler {
    pub const MAX_FACTOR: usize = 4;

    pub const fn new(factor: usize) -> Self {
        Self {
            factor,
            up1: AllpassPair::new(STAGE1),
            up2: AllpassPair::new(STAGE2),
            dn1: AllpassPair::new(STAGE1),
            dn2: AllpassPair::new(STAGE2),
        }
    }

    /// Smallest factor that brings `sample_rate` to at least 176.4 kHz.
    pub fn factor_for(sample_rate: f64) -> usize {
        if sample_rate < 60_000.0 {
            4
        } else if sample_rate < 120_000.0 {
            2
        } else {
            1
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn factor(&self) -> usize {
        self.factor
    }

    pub fn reset(&mut self) {
        self.up1.reset();
        self.up2.reset();
        self.dn1.reset();
        self.dn2.reset();
    }

    /// Interpolate one base-rate sample into `factor` samples in `out`.
    #[inline(always)]
    pub fn up(&mut self, x: f64, out: &mut [f64; Self::MAX_FACTOR]) {
        match self.factor {
            4 => {
                let (a, b) = self.up1.up(x);
                let (a0, a1) = self.up2.up(a);
                let (b0, b1) = self.up2.up(b);
                *out = [a0, a1, b0, b1];
            }
            2 => {
                let (a, b) = self.up1.up(x);
                out[0] = a;
                out[1] = b;
            }
            _ => out[0] = x,
        }
    }

    /// Decimate `factor` samples from `inp` into one base-rate sample.
    #[inline(always)]
    pub fn down(&mut self, inp: &[f64; Self::MAX_FACTOR]) -> f64 {
        match self.factor {
            4 => {
                let a = self.dn2.down(inp[0], inp[1]);
                let b = self.dn2.down(inp[2], inp[3]);
                self.dn1.down(a, b)
            }
            2 => self.dn1.down(inp[0], inp[1]),
            _ => inp[0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn tone_level(os: &mut Oversampler, f: f64, fs: f64, nonlinear: bool) -> (f64, Vec<f64>) {
        let n = 1 << 15;
        let mut buf = [0.0; 4];
        let mut y = Vec::with_capacity(n);
        for i in 0..n {
            let x = 0.5 * (2.0 * PI * f * i as f64 / fs).sin();
            os.up(x, &mut buf);
            if nonlinear {
                for s in buf.iter_mut().take(os.factor()) {
                    *s = s.signum() * s.abs().min(0.25);
                }
            }
            y.push(os.down(&buf));
        }
        let a = crate::biquad::tests::fit_amplitude(&y, f, fs, n / 2);
        (a, y[n / 2..].to_vec())
    }

    #[test]
    fn passband_is_flat() {
        for factor in [2, 4] {
            for f in [100.0, 1_000.0, 10_000.0, 19_000.0] {
                let mut os = Oversampler::new(factor);
                let (a, _) = tone_level(&mut os, f, 44_100.0, false);
                let db = 20.0 * (a / 0.5).log10();
                assert!(db.abs() < 0.01, "factor {factor} {f} Hz: {db} dB");
            }
        }
    }

    #[test]
    fn clipping_products_do_not_alias() {
        // A clipped 7 kHz tone at 44.1 kHz: odd harmonics above Nyquist must be
        // removed before decimation instead of folding back.
        let fs = 44_100.0;
        let mut os = Oversampler::new(4);
        let (_, y) = tone_level(&mut os, 7_000.0, fs, true);
        let w: Vec<f64> = (0..y.len()).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / y.len() as f64).cos()).collect();
        // Alias of H5 (35 kHz) lands at 9.1 kHz; H7 (49 kHz) at 4.9 kHz.
        for (alias, label) in [(9_100.0, "H5"), (4_900.0, "H7")] {
            let (mut s, mut c, mut s0, mut c0) = (0.0, 0.0, 0.0, 0.0);
            for (k, v) in y.iter().enumerate() {
                let ph = 2.0 * PI * alias * k as f64 / fs;
                let p0 = 2.0 * PI * 7_000.0 * k as f64 / fs;
                s += v * w[k] * ph.sin();
                c += v * w[k] * ph.cos();
                s0 += v * w[k] * p0.sin();
                c0 += v * w[k] * p0.cos();
            }
            let rel = 20.0 * ((s * s + c * c).sqrt() / (s0 * s0 + c0 * c0).sqrt()).log10();
            assert!(rel < -75.0, "{label} alias at {rel:.1} dB");
        }
    }
}
