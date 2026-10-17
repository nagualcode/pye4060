//! Pye TVT 4060 compression amplifier (circuit 845751/01) - audio plugin.
//!
//! Component-level model of the PWM compressor/limiter Eddie Kramer used at
//! Olympic Studios on Jimi Hendrix, Led Zeppelin and Rolling Stones sessions:
//! - feedback side chain (T4, OA47 bridge, ratio amplifier) driving the C23
//!   timing capacitor, as in the 845751/01 schematic and manual Fig. 2.4
//! - 250 kHz chopper modelled by its mark:space average, hyperbolic gain law,
//!   ~21.5 dB maximum attenuation
//! - audio-rate control ripple: the program-dependent "scrunch"
//! - 5-pole LC anti-carrier filter, T1/T2 transformers, class-A amplifiers
//! - integrated noise gate (250 ms hold, 250 ms fade, < 1 ms opening)
//! - dual mono, stereo and M/S, with C23 control-voltage linking
//! - 4x oversampled analog path at 44.1/48 kHz (2x at 88.2/96 kHz)

mod biquad;
mod editor;
mod engine;
mod noise_gate;
mod oversampler;
mod pwm_chopper;
mod pye_channel;
mod transformer;

use std::sync::Arc;
use truce::prelude::*;

use engine::{FrameParams, PyeEngine, Routing};
use pwm_chopper::{ControlCoeffs, PyeDecay, PyeRatio};
use pye_channel::{ScFilter, TURNOVER_REF_DB, TickParams};
use transformer::AnalogMode;

#[derive(ParamEnum)]
pub enum ProcessingMode {
    #[name = "Dual Mono"]
    DualMono,
    #[name = "Stereo"]
    Stereo,
    #[name = "M/S"]
    MidSide,
}

#[derive(ParamEnum)]
pub enum RatioEnum {
    #[name = "1:1"]
    Ratio1_1,
    #[name = "2:1"]
    Ratio2_1,
    #[name = "3:1"]
    Ratio3_1,
    #[name = "5:1"]
    Ratio5_1,
    #[name = "Limit"]
    RatioLimit,
}

impl From<RatioEnum> for PyeRatio {
    fn from(r: RatioEnum) -> Self {
        match r {
            RatioEnum::Ratio1_1 => PyeRatio::Ratio1_1,
            RatioEnum::Ratio2_1 => PyeRatio::Ratio2_1,
            RatioEnum::Ratio3_1 => PyeRatio::Ratio3_1,
            RatioEnum::Ratio5_1 => PyeRatio::Ratio5_1,
            RatioEnum::RatioLimit => PyeRatio::Limit,
        }
    }
}

#[derive(ParamEnum)]
pub enum DecayEnum {
    #[name = "100 ms"]
    Decay100,
    #[name = "200 ms"]
    Decay200,
    #[name = "400 ms"]
    Decay400,
    #[name = "800 ms"]
    Decay800,
    #[name = "1600 ms"]
    Decay1600,
    #[name = "3200 ms"]
    Decay3200,
}

impl From<DecayEnum> for PyeDecay {
    fn from(d: DecayEnum) -> Self {
        match d {
            DecayEnum::Decay100 => PyeDecay::Decay100,
            DecayEnum::Decay200 => PyeDecay::Decay200,
            DecayEnum::Decay400 => PyeDecay::Decay400,
            DecayEnum::Decay800 => PyeDecay::Decay800,
            DecayEnum::Decay1600 => PyeDecay::Decay1600,
            DecayEnum::Decay3200 => PyeDecay::Decay3200,
        }
    }
}

#[derive(ParamEnum)]
pub enum GateAttenEnum {
    #[name = "0 dB (Off)"]
    Gate0dB,
    #[name = "2 dB"]
    Gate2dB,
    #[name = "4 dB"]
    Gate4dB,
    #[name = "6 dB"]
    Gate6dB,
    #[name = "8 dB"]
    Gate8dB,
    #[name = "10 dB"]
    Gate10dB,
}

impl GateAttenEnum {
    pub fn to_db(self) -> f64 {
        match self {
            GateAttenEnum::Gate0dB => 0.0,
            GateAttenEnum::Gate2dB => 2.0,
            GateAttenEnum::Gate4dB => 4.0,
            GateAttenEnum::Gate6dB => 6.0,
            GateAttenEnum::Gate8dB => 8.0,
            GateAttenEnum::Gate10dB => 10.0,
        }
    }
}

#[derive(ParamEnum)]
pub enum AnalogModeEnum {
    #[name = "Off"]
    Off,
    #[name = "50 Hz"]
    Hz50,
    #[name = "60 Hz"]
    Hz60,
}

impl From<AnalogModeEnum> for AnalogMode {
    fn from(a: AnalogModeEnum) -> Self {
        match a {
            AnalogModeEnum::Off => AnalogMode::Off,
            AnalogModeEnum::Hz50 => AnalogMode::Hz50,
            AnalogModeEnum::Hz60 => AnalogMode::Hz60,
        }
    }
}

#[derive(ParamEnum)]
pub enum ScFilterEnum {
    #[name = "Off"]
    Off,
    #[name = "60 Hz"]
    Hz60,
    #[name = "90 Hz"]
    Hz90,
    #[name = "150 Hz"]
    Hz150,
}

impl From<ScFilterEnum> for ScFilter {
    fn from(f: ScFilterEnum) -> Self {
        match f {
            ScFilterEnum::Off => ScFilter::Off,
            ScFilterEnum::Hz60 => ScFilter::Hz60,
            ScFilterEnum::Hz90 => ScFilter::Hz90,
            ScFilterEnum::Hz150 => ScFilter::Hz150,
        }
    }
}

#[derive(Params)]
pub struct Pye4060Params {
    // --- Master / Routing Controls ---
    #[param(name = "Processing Mode", short_name = "Mode", default = 1)]
    pub mode: EnumParam<ProcessingMode>,

    /// Joins the two C23 control voltages (hardware LINK switch).
    #[param(name = "Link Channels", short_name = "Link", default = true)]
    pub link: BoolParam,

    #[param(name = "Analog Hum", short_name = "Analog")]
    pub analog: EnumParam<AnalogModeEnum>,

    #[param(name = "Bypass", short_name = "Bypass", default = false)]
    pub bypass: BoolParam,

    #[param(
        name = "Mix",
        short_name = "Mix",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        smooth = "exp(20)"
    )]
    pub mix: FloatParam,

    #[param(name = "Sidechain HPF", short_name = "SC HPF")]
    pub sc_filter: EnumParam<ScFilterEnum>,

    // --- Channel A (Left / Mid) Controls ---
    #[param(
        name = "Threshold A",
        short_name = "Thresh A",
        range = "linear(-24, 16)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub threshold_a: FloatParam,

    #[param(name = "Ratio A", short_name = "Ratio A", default = 2)]
    pub ratio_a: EnumParam<RatioEnum>,

    #[param(name = "Decay A", short_name = "Decay A", default = 2)]
    pub decay_a: EnumParam<DecayEnum>,

    #[param(
        name = "Gate Thresh A",
        short_name = "Gate Th A",
        range = "linear(-50, -20)",
        default = -30.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub gate_thresh_a: FloatParam,

    #[param(name = "Gate Atten A", short_name = "Gate Att A")]
    pub gate_atten_a: EnumParam<GateAttenEnum>,

    #[param(
        name = "Output A",
        short_name = "Out A",
        range = "linear(-18, 18)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub output_a: FloatParam,

    // --- Channel B (Right / Side) Controls ---
    #[param(
        name = "Threshold B",
        short_name = "Thresh B",
        range = "linear(-24, 16)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub threshold_b: FloatParam,

    #[param(name = "Ratio B", short_name = "Ratio B", default = 2)]
    pub ratio_b: EnumParam<RatioEnum>,

    #[param(name = "Decay B", short_name = "Decay B", default = 2)]
    pub decay_b: EnumParam<DecayEnum>,

    #[param(
        name = "Gate Thresh B",
        short_name = "Gate Th B",
        range = "linear(-50, -20)",
        default = -30.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub gate_thresh_b: FloatParam,

    #[param(name = "Gate Atten B", short_name = "Gate Att B")]
    pub gate_atten_b: EnumParam<GateAttenEnum>,

    #[param(
        name = "Output B",
        short_name = "Out B",
        range = "linear(-18, 18)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub output_b: FloatParam,

    // --- Meters ---
    /// Gain reduction, dB.
    #[meter]
    pub gr_meter_a: MeterSlot,

    #[meter]
    pub gr_meter_b: MeterSlot,

    /// Output level with VU ballistics, linear (sine RMS-calibrated).
    #[meter]
    pub out_meter_a: MeterSlot,

    #[meter]
    pub out_meter_b: MeterSlot,

    /// Output sample peak since the previous block, linear.
    #[meter]
    pub peak_meter_a: MeterSlot,

    #[meter]
    pub peak_meter_b: MeterSlot,
}

use Pye4060ParamsParamId as P;

pub struct Pye4060;

/// VU ballistics: full-wave average through two poles (~300 ms to 99 %),
/// scaled to read sine RMS.
#[derive(Clone, Copy, Debug, Default)]
struct Vu {
    s1: f64,
    s2: f64,
    k: f64,
}

impl Vu {
    fn new(fs: f64) -> Self {
        Self { s1: 0.0, s2: 0.0, k: biquad::tau_coeff(0.065, fs) }
    }

    #[inline(always)]
    fn process(&mut self, x: f64) {
        self.s1 += (x.abs() - self.s1) * self.k;
        self.s2 += (self.s1 - self.s2) * self.k;
    }

    fn level(&self) -> f32 {
        (self.s2 * (std::f64::consts::PI / 2.0) * std::f64::consts::FRAC_1_SQRT_2) as f32
    }
}

pub struct Pye4060Dsp {
    engine: PyeEngine,
    sample_rate: f64,
    vu: [Vu; 2],
}

impl Default for Pye4060Dsp {
    fn default() -> Self {
        Self { engine: PyeEngine::new(48_000.0), sample_rate: 48_000.0, vu: [Vu::new(48_000.0); 2] }
    }
}

#[inline]
fn db_to_lin(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

impl PluginLogic for Pye4060 {
    type Params = Pye4060Params;
    type DspState = Pye4060Dsp;

    fn reset(state: &mut Self::DspState, _params: &Self::Params, config: &AudioConfig) {
        if (config.sample_rate - state.sample_rate).abs() < 0.5 {
            state.engine.reset();
        } else {
            state.engine = PyeEngine::new(config.sample_rate);
        }
        state.sample_rate = config.sample_rate;
        state.vu = [Vu::new(config.sample_rate); 2];
    }

    fn process(
        state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        let num_channels = buffer.channels();
        let n = buffer.num_samples();

        if params.bypass.value() {
            let n_in = buffer.num_input_channels();
            for ch in 0..num_channels {
                let (inp, out) = buffer.io(ch);
                if ch < n_in {
                    out.copy_from_slice(inp);
                } else {
                    out.fill(0.0);
                }
            }
            // Keep the smoothers in step so un-bypassing does not glide.
            for p in [&params.threshold_a, &params.threshold_b, &params.output_a, &params.output_b, &params.mix] {
                let _ = p.read_after(n);
            }
            context.set_meter(P::GrMeterA, 0.0);
            context.set_meter(P::GrMeterB, 0.0);
            return ProcessStatus::Normal;
        }

        let fs_int = PyeEngine::internal_rate(state.sample_rate);
        let routing = match params.mode.value() {
            ProcessingMode::DualMono => Routing::DualMono,
            ProcessingMode::Stereo => Routing::Stereo,
            ProcessingMode::MidSide => Routing::MidSide,
        };
        let stereo = routing == Routing::Stereo;
        let analog = AnalogMode::from(params.analog.value());

        // Switch settings: block rate. In stereo mode channel A's controls
        // drive both programme chains.
        let ratio_a = PyeRatio::from(params.ratio_a.value());
        let decay_a = PyeDecay::from(params.decay_a.value());
        let (ratio_b, decay_b) = if stereo {
            (ratio_a, decay_a)
        } else {
            (PyeRatio::from(params.ratio_b.value()), PyeDecay::from(params.decay_b.value()))
        };
        let coeffs = [ControlCoeffs::new(ratio_a, decay_a, fs_int), ControlCoeffs::new(ratio_b, decay_b, fs_int)];
        let gate_th_a = db_to_lin(f64::from(params.gate_thresh_a.read_after(n)));
        let gate_th_b = db_to_lin(f64::from(params.gate_thresh_b.read_after(n)));
        let gate_at_a = db_to_lin(-params.gate_atten_a.value().to_db());
        let gate_at_b = db_to_lin(-params.gate_atten_b.value().to_db());
        let (gate_th_b, gate_at_b) = if stereo { (gate_th_a, gate_at_a) } else { (gate_th_b, gate_at_b) };
        let sc = ScFilter::from(params.sc_filter.value());
        state.engine.set_sc_filter([sc, sc]);

        let tick = |turnover: f64, gate_threshold: f64, gate_atten: f64| TickParams {
            turnover,
            gate_threshold,
            gate_atten,
            analog,
        };
        let mut fp = FrameParams {
            routing,
            link: params.link.value(),
            tick: [tick(1.0, gate_th_a, gate_at_a), tick(1.0, gate_th_b, gate_at_b)],
            coeffs,
            output: [1.0, 1.0],
            mix: 1.0,
        };

        let mut peak = [0.0f64; 2];
        let engine = &mut state.engine;
        let vu = &mut state.vu;

        if num_channels >= 2 {
            buffer.for_each_frame_io::<2, 2, _>(|frame_in, frame_out| {
                let th_a = f64::from(params.threshold_a.read());
                let th_b = f64::from(params.threshold_b.read());
                let out_a = f64::from(params.output_a.read());
                let out_b = f64::from(params.output_b.read());
                let (th_b, out_b) = if stereo { (th_a, out_a) } else { (th_b, out_b) };
                fp.tick[0].turnover = db_to_lin(th_a + TURNOVER_REF_DB);
                fp.tick[1].turnover = db_to_lin(th_b + TURNOVER_REF_DB);
                fp.output = [db_to_lin(out_a), db_to_lin(out_b)];
                fp.mix = f64::from(params.mix.read());

                let (l, r) = engine.process_frame(f64::from(frame_in[0]), f64::from(frame_in[1]), &fp);
                vu[0].process(l);
                vu[1].process(r);
                peak[0] = peak[0].max(l.abs());
                peak[1] = peak[1].max(r.abs());
                frame_out[0] = l as f32;
                frame_out[1] = r as f32;
            });
        } else if num_channels == 1 {
            let _ = (params.threshold_b.read_after(n), params.output_b.read_after(n));
            buffer.for_each_frame_io::<1, 1, _>(|frame_in, frame_out| {
                fp.tick[0].turnover = db_to_lin(f64::from(params.threshold_a.read()) + TURNOVER_REF_DB);
                fp.output[0] = db_to_lin(f64::from(params.output_a.read()));
                fp.mix = f64::from(params.mix.read());
                let y = engine.process_mono(f64::from(frame_in[0]), &fp);
                vu[0].process(y);
                peak[0] = peak[0].max(y.abs());
                frame_out[0] = y as f32;
            });
        }

        let gr = state.engine.take_gr_peak();
        context.set_meter(P::GrMeterA, gr[0] as f32);
        context.set_meter(P::GrMeterB, gr[1] as f32);
        context.set_meter(P::OutMeterA, state.vu[0].level());
        context.set_meter(P::OutMeterB, state.vu[1].level());
        context.set_meter(P::PeakMeterA, peak[0] as f32);
        context.set_meter(P::PeakMeterB, peak[1] as f32);

        ProcessStatus::Normal
    }

    fn editor(params: Arc<Pye4060Params>) -> Box<dyn Editor> {
        editor::create(params)
    }
}

truce::plugin! {
    logic: Pye4060,
    params: Pye4060Params,
}

truce::enable_rt_paranoid!();
