//! One 4060 channel at the internal (oversampled) rate.
//!
//! ```text
//! input --> T1 --> VT1 --+--> chopper VT2 --> LC filter --> VT3-VT5 --+--> output atten. --> VT6-VT9/T2 --> out
//!                        |         ^                                  |    (+ gate pad)
//!                        |         | mark:space                       |
//!                        |     C23 / blocking osc. <-- ratio amp <-- bridge <-- T4
//!                        +--> noise gate detector (pad on output attenuator)
//! ```

use crate::biquad::{Biquad, OnePole};
use crate::noise_gate::NoiseGate;
use crate::transformer::{AnalogMode, NoiseSource, OutputStage, ProgrammeChain};

/// Compression turnover (3:1, sine peak) sits 20 dB below the threshold
/// dial reading in dBFS: dial 0 -> turnover at -20 dBFS peak.
pub const TURNOVER_REF_DB: f64 = -20.0;
/// Input-referred hiss of the programme chain, relative to the turnover.
const HISS_PRE: f64 = 6.0e-4;
/// Output stage hiss, dBFS-linear (~ -94 dBFS).
const HISS_OUT: f64 = 2.0e-5;

/// Modern addition: side-chain high-pass ahead of the bridge rectifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScFilter {
    Off,
    Hz60,
    Hz90,
    Hz150,
}

impl ScFilter {
    fn corner(self) -> Option<f64> {
        match self {
            ScFilter::Off => None,
            ScFilter::Hz60 => Some(60.0),
            ScFilter::Hz90 => Some(90.0),
            ScFilter::Hz150 => Some(150.0),
        }
    }
}

/// Settings for one internal sample.
#[derive(Clone, Copy, Debug)]
pub struct TickParams {
    /// Linear gain of the output attenuator (= turnover level, dBFS-linear).
    pub turnover: f64,
    pub gate_threshold: f64,
    pub gate_atten: f64,
    pub analog: AnalogMode,
}

#[derive(Clone, Copy, Debug)]
pub struct PyeChannel {
    fs: f64,
    chain: ProgrammeChain,
    output: OutputStage,
    gate: NoiseGate,
    t4_hp: OnePole,
    sc_filter: ScFilter,
    sc_hp: Biquad,
    noise: NoiseSource,
}

impl PyeChannel {
    pub fn new(fs: f64, seed: u32) -> Self {
        Self {
            fs,
            chain: ProgrammeChain::new(fs),
            output: OutputStage::new(fs),
            gate: NoiseGate::new(fs),
            // T4 772201 side-chain transformer: small core, early LF roll-off.
            t4_hp: OnePole::high_pass(18.0, fs),
            sc_filter: ScFilter::Off,
            sc_hp: Biquad::identity(),
            noise: NoiseSource::new(seed),
        }
    }

    pub fn reset(&mut self) {
        self.chain.reset();
        self.output.reset();
        self.gate.reset();
        self.t4_hp.reset();
        self.sc_hp.reset();
        self.noise.reset();
    }

    pub fn set_sc_filter(&mut self, f: ScFilter) {
        if f != self.sc_filter {
            self.sc_filter = f;
            self.sc_hp = match f.corner() {
                Some(fc) => Biquad::high_pass(fc, std::f64::consts::FRAC_1_SQRT_2, self.fs),
                None => Biquad::identity(),
            };
        }
    }

    /// Process one internal sample. `x` is the input in dBFS-linear units,
    /// `gain` the chopper mark fraction. Returns (output, side-chain level).
    #[inline(always)]
    pub fn tick(&mut self, x: f64, gain: f64, p: &TickParams) -> (f64, f64) {
        let analog = p.analog != AnalogMode::Off;
        let u = self.chain.input(x / p.turnover);
        let gate = self.gate.process(u, p.gate_threshold, p.gate_atten);
        let hiss = if analog { HISS_PRE * self.noise.hiss() } else { 0.0 };
        let s = self.chain.chop(u, gain, hiss);

        let sc = self.sc_hp.process(self.t4_hp.process(s));

        let mut y = self.output.process(s * p.turnover * gate);
        if analog {
            y += HISS_OUT * self.noise.hiss() + self.noise.hum(p.analog, self.fs);
        }
        (y, sc.abs())
    }
}
