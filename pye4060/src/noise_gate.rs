//! Noise gate of the 845751/01 (noise gate printed circuit board).
//!
//! R101 taps the programme after the input amplifier, so the gate threshold
//! tracks the compression threshold ("continuously variable between -20 dB
//! and -50 dB, relative to the compression threshold"). TR101-TR103 amplify,
//! L101/C106 strip the high end, D101/D102 rectify into C108, and the
//! trigger drives the TR107 2N3819 FET that switches the SWC pad
//! (R119-R122, 2-10 dB) into the output attenuator.
//!
//! Manual ch. 1: attenuation is introduced "whenever the signal level falls
//! below the noise threshold level for a time in excess of 250 mS ...
//! smoothly, with a time-constant of 250 mS but, when the input signal level
//! rises above the noise threshold level, the attenuation is switched out of
//! circuit in not more than 1 mS."

use crate::biquad::{Biquad, OnePole, tau_coeff};

const HOLD_SECONDS: f64 = 0.250;
const CLOSE_TAU: f64 = 0.250;
const OPEN_TAU: f64 = 0.25e-3;
/// C108 detector: fast charge through D101/D102 OA47 (~0.2 ms), ~10 ms discharge.
const DET_ATTACK: f64 = 0.2e-3;
const DET_RELEASE: f64 = 0.010;
/// Trigger hysteresis (the TR104/TR105 Schmitt trigger is regenerative).
const HYSTERESIS: f64 = 1.26; // 2 dB

#[derive(Clone, Copy, Debug)]
pub struct NoiseGate {
    hpf: OnePole,
    lpf: Biquad,
    env: f64,
    det_att_k: f64,
    det_rel_k: f64,
    open_k: f64,
    close_k: f64,
    hold_samples: u32,
    below: u32,
    open: bool,
    gain: f64,
}

impl Default for NoiseGate {
    fn default() -> Self {
        Self::new(192_000.0)
    }
}

impl NoiseGate {
    pub fn new(fs: f64) -> Self {
        Self {
            // C101/C102 coupling strips mains hum; L101 (52.5 mH) + C106 (3.3 nF)
            // form a 2-pole LC low-pass at f0 = 1 / (2*pi*sqrt(L*C)) = 12,091 Hz.
            hpf: OnePole::high_pass(35.0, fs),
            lpf: Biquad::low_pass_matched(12_091.0, std::f64::consts::FRAC_1_SQRT_2, fs),
            env: 0.0,
            det_att_k: tau_coeff(DET_ATTACK, fs),
            det_rel_k: tau_coeff(DET_RELEASE, fs),
            open_k: tau_coeff(OPEN_TAU, fs),
            close_k: tau_coeff(CLOSE_TAU, fs),
            hold_samples: (HOLD_SECONDS * fs) as u32,
            below: 0,
            open: true,
            gain: 1.0,
        }
    }

    pub fn reset(&mut self) {
        self.hpf.reset();
        self.lpf.reset();
        self.env = 0.0;
        self.below = 0;
        self.open = true;
        self.gain = 1.0;
    }

    /// `u`: programme after the input amplifier (internal units).
    /// `threshold`: gate threshold, linear relative to the turnover.
    /// `atten`: SWC pad gain (1.0 = 0 dB position, gate inactive).
    #[inline(always)]
    pub fn process(&mut self, u: f64, threshold: f64, atten: f64) -> f64 {
        if atten >= 0.9999 && self.gain >= 0.999_999 {
            self.open = true;
            self.below = 0;
            self.gain = 1.0;
            return 1.0;
        }

        let r = self.lpf.process(self.hpf.process(u)).abs();
        let det_k = if r > self.env { self.det_att_k } else { self.det_rel_k };
        self.env += (r - self.env) * det_k;

        if atten >= 0.9999 {
            self.open = true;
            self.below = 0;
        } else if self.open {
            if self.env < threshold {
                self.below += 1;
                if self.below >= self.hold_samples {
                    self.open = false;
                }
            } else {
                self.below = 0;
            }
        } else if self.env > threshold * HYSTERESIS {
            self.open = true;
            self.below = 0;
        }

        let (target, k) = if self.open { (1.0, self.open_k) } else { (atten, self.close_k) };
        self.gain += (target - self.gain) * k;
        self.gain
    }
}
