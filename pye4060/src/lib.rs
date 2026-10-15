//! Pye TVT 84-4060 Classic PWM Compressor / Limiter Audio Plugin.
//!
//! Faithful component-level reconstruction of the legendary Olympic Studios
//! dynamics processor used by Eddie Kramer on Jimi Hendrix, Led Zeppelin, and The Rolling Stones.
//!
//! Features:
//! - High-frequency Pulse Width Modulation (PWM) chopper gain reduction (VT2 V405)
//! - Germanium diode full-wave bridge rectifier (OA47) with authentic exponential soft knee
//! - Switched 6-position RC decay constants (100ms - 3200ms) with 22uF timing capacitor
//! - Automatic +8 dB threshold turnover shift in LIMIT mode
//! - Integrated broadcast Noise Gate (Type 845751/01) with 250ms hold/decay and 1ms attack
//! - 5-pole LC anti-carrier low-pass filter (L1, L2 52.5mH, C5-C7 0.0033uF)
//! - Discrete push-pull output driver (VT9 2N4062, VT10 2N3053) & transformer T2 with H2 asymmetry
//! - Dual Mono, Stereo, and Mid/Side (M/S) processing topologies with Channel Linking
//! - Defeatable British (50 Hz) and US (60 Hz) analog power-rail hum modeling

mod biquad;
mod oversampler;
mod pwm_chopper;
mod pye_channel;
mod transformer;

use std::sync::Arc;
use truce::prelude::*;
use truce_gui::IntoLayoutEditor;
use truce_gui_types::layout::{dropdown, knob, meter, toggle, widgets, GridLayout};

use pye_channel::PyeChannel;
use pwm_chopper::{PyeDecay, PyeRatio};
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
    pub fn to_db(self) -> f32 {
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

#[derive(Params)]
pub struct Pye4060Params {
    // --- Master / Routing Controls ---
    #[param(name = "Processing Mode", short_name = "Mode")]
    pub mode: EnumParam<ProcessingMode>,

    #[param(name = "Link Channels", short_name = "Link", default = true)]
    pub link: BoolParam,

    #[param(name = "Analog Hum", short_name = "Analog")]
    pub analog: EnumParam<AnalogModeEnum>,

    #[param(name = "Bypass", short_name = "Bypass", default = false)]
    pub bypass: BoolParam,

    // --- Channel A (Left / Mid) Controls ---
    #[param(
        name = "Threshold A",
        short_name = "Thresh A",
        range = "linear(-24, 16)",
        default = 16.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub threshold_a: FloatParam,

    #[param(name = "Ratio A", short_name = "Ratio A")]
    pub ratio_a: EnumParam<RatioEnum>,

    #[param(name = "Decay A", short_name = "Decay A")]
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
        default = 16.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub threshold_b: FloatParam,

    #[param(name = "Ratio B", short_name = "Ratio B")]
    pub ratio_b: EnumParam<RatioEnum>,

    #[param(name = "Decay B", short_name = "Decay B")]
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
    #[meter]
    pub gr_meter_a: MeterSlot,

    #[meter]
    pub gr_meter_b: MeterSlot,

    #[meter]
    pub out_meter_a: MeterSlot,

    #[meter]
    pub out_meter_b: MeterSlot,
}

use Pye4060ParamsParamId as P;

pub struct Pye4060;

pub struct Pye4060Dsp {
    pub channels: [PyeChannel; 2],
    pub sample_rate: f32,
}

impl Default for Pye4060Dsp {
    fn default() -> Self {
        Self {
            channels: [PyeChannel::new(48000.0), PyeChannel::new(48000.0)],
            sample_rate: 48000.0,
        }
    }
}

impl PluginLogic for Pye4060 {
    type Params = Pye4060Params;
    type DspState = Pye4060Dsp;

    fn reset(state: &mut Self::DspState, _params: &Self::Params, config: &AudioConfig) {
        state.sample_rate = config.sample_rate as f32;
        for ch in &mut state.channels {
            ch.set_sample_rate(config.sample_rate as f32);
            ch.reset();
        }
    }

    fn process(
        state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        let num_channels = buffer.channels();

        // Hard bypass path
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
            return ProcessStatus::Normal;
        }

        let mode = params.mode.value();
        let link = params.link.value();
        let analog = AnalogMode::from(params.analog.value());

        // Channel A settings
        let thresh_a = params.threshold_a.value();
        let ratio_a = PyeRatio::from(params.ratio_a.value());
        let decay_a = PyeDecay::from(params.decay_a.value());
        let gate_th_a = params.gate_thresh_a.value();
        let gate_at_a = params.gate_atten_a.value().to_db();
        let out_a = params.output_a.value();

        // Channel B settings (or linked to A in Stereo)
        let (thresh_b, ratio_b, decay_b, gate_th_b, gate_at_b, out_b) = match mode {
            ProcessingMode::Stereo => (thresh_a, ratio_a, decay_a, gate_th_a, gate_at_a, out_a),
            ProcessingMode::DualMono | ProcessingMode::MidSide => {
                if link {
                    (
                        thresh_a,
                        ratio_a,
                        decay_a,
                        gate_th_a,
                        gate_at_a,
                        params.output_b.value(),
                    )
                } else {
                    (
                        params.threshold_b.value(),
                        PyeRatio::from(params.ratio_b.value()),
                        PyeDecay::from(params.decay_b.value()),
                        params.gate_thresh_b.value(),
                        params.gate_atten_b.value().to_db(),
                        params.output_b.value(),
                    )
                }
            }
        };

        const INV_SQRT_2: f32 = 0.70710678;

        if num_channels >= 2 {
            buffer.for_each_frame_io::<2, 2, _>(|frame_in, frame_out| {
                let l_in = frame_in[0];
                let r_in = frame_in[1];

                match mode {
                    ProcessingMode::DualMono => {
                        let (sc_l, sc_r) = if link {
                            let max_sc = l_in.abs().max(r_in.abs());
                            (max_sc, max_sc)
                        } else {
                            (l_in, r_in)
                        };

                        frame_out[0] = state.channels[0].process_sample(
                            l_in, sc_l, thresh_a, ratio_a, decay_a, gate_th_a, gate_at_a, out_a, analog,
                        );
                        frame_out[1] = state.channels[1].process_sample(
                            r_in, sc_r, thresh_b, ratio_b, decay_b, gate_th_b, gate_at_b, out_b, analog,
                        );
                    }
                    ProcessingMode::Stereo => {
                        let max_sc = l_in.abs().max(r_in.abs());

                        frame_out[0] = state.channels[0].process_sample(
                            l_in, max_sc, thresh_a, ratio_a, decay_a, gate_th_a, gate_at_a, out_a, analog,
                        );
                        frame_out[1] = state.channels[1].process_sample(
                            r_in, max_sc, thresh_a, ratio_a, decay_a, gate_th_a, gate_at_a, out_a, analog,
                        );
                    }
                    ProcessingMode::MidSide => {
                        // M/S Encode
                        let mid = (l_in + r_in) * INV_SQRT_2;
                        let side = (l_in - r_in) * INV_SQRT_2;

                        let (sc_m, sc_s) = if link {
                            let max_sc = mid.abs().max(side.abs());
                            (max_sc, max_sc)
                        } else {
                            (mid, side)
                        };

                        let mid_out = state.channels[0].process_sample(
                            mid, sc_m, thresh_a, ratio_a, decay_a, gate_th_a, gate_at_a, out_a, analog,
                        );
                        let side_out = state.channels[1].process_sample(
                            side, sc_s, thresh_b, ratio_b, decay_b, gate_th_b, gate_at_b, out_b, analog,
                        );

                        // M/S Decode
                        frame_out[0] = (mid_out + side_out) * INV_SQRT_2;
                        frame_out[1] = (mid_out - side_out) * INV_SQRT_2;
                    }
                }
            });
        } else if num_channels == 1 {
            buffer.for_each_frame_io::<1, 1, _>(|frame_in, frame_out| {
                let x = frame_in[0];
                frame_out[0] = state.channels[0].process_sample(
                    x, x, thresh_a, ratio_a, decay_a, gate_th_a, gate_at_a, out_a, analog,
                );
            });
        }

        // Update meters
        context.set_meter(P::GrMeterA, state.channels[0].meter_gr());
        context.set_meter(P::OutMeterA, state.channels[0].meter_out());
        context.set_meter(P::GrMeterB, state.channels[1].meter_gr());
        context.set_meter(P::OutMeterB, state.channels[1].meter_out());

        ProcessStatus::Normal
    }

    fn editor(params: Arc<Pye4060Params>) -> Box<dyn Editor> {
        // Ergonomic 19" rack-style side-by-side layout:
        // Left: Channel A (cols 0..3)
        // Center: Master & Routing (cols 4..5)
        // Right: Channel B (cols 6..9)
        // Exactly 2 rows high, with resizable(false) so the window height is strictly clamped.
        GridLayout::build(vec![widgets(vec![
            // --- Row 0: Channel A (Compression Dynamics) ---
            knob(P::ThresholdA, "Thresh A").at(0, 0),
            dropdown(P::RatioA, "Ratio A").at(1, 0),
            dropdown(P::DecayA, "Decay A").at(2, 0),
            meter(&[P::GrMeterA], "GR A").at(3, 0),

            // --- Row 0: Master Section ---
            dropdown(P::Mode, "Mode").at(4, 0),
            toggle(P::Bypass, "Bypass").at(5, 0),

            // --- Row 0: Channel B (Compression Dynamics) ---
            knob(P::ThresholdB, "Thresh B").at(6, 0),
            dropdown(P::RatioB, "Ratio B").at(7, 0),
            dropdown(P::DecayB, "Decay B").at(8, 0),
            meter(&[P::GrMeterB], "GR B").at(9, 0),

            // --- Row 1: Channel A (Noise Gate & Output) ---
            knob(P::GateThreshA, "Gate Th A").at(0, 1),
            dropdown(P::GateAttenA, "Gate Att A").at(1, 1),
            knob(P::OutputA, "Out A").at(2, 1),
            meter(&[P::OutMeterA], "VU Out A").at(3, 1),

            // --- Row 1: Master Section ---
            toggle(P::Link, "Link A-B").at(4, 1),
            dropdown(P::Analog, "Analog").at(5, 1),

            // --- Row 1: Channel B (Noise Gate & Output) ---
            knob(P::GateThreshB, "Gate Th B").at(6, 1),
            dropdown(P::GateAttenB, "Gate Att B").at(7, 1),
            knob(P::OutputB, "Out B").at(8, 1),
            meter(&[P::OutMeterB], "VU Out B").at(9, 1),
        ])])
        .with_title("PYE TVT 84-4060")
        .with_subtitle("PWM COMPRESSOR / LIMITER - DUAL MONO / STEREO / M-S")
        .resizable(false)
        .into_editor(&params)
    }
}

truce::plugin! {
    logic: Pye4060,
    params: Pye4060Params,
}

truce::enable_rt_paranoid!();
