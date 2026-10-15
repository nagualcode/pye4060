//! Monolithic Channel Strip for Pye TVT 84-4060 Compressor.
//!
//! Complete signal path:
//! Input -> Input Transformer T1 -> PWM Chopper Gain Reduction (with Noise Gate)
//!       -> 5-Pole LC Anti-Carrier Filter -> 2x Oversampled Output Saturation & Transformer T2
//!       -> Makeup Gain -> Analog Hum -> Output

use crate::oversampler::Oversampler2x;
use crate::pwm_chopper::{PwmChopper, PyeDecay, PyeRatio};
use crate::transformer::{AnalogMode, TransformerStage};

#[derive(Clone, Copy, Debug)]
pub struct PyeChannel {
    pub chopper: PwmChopper,
    pub transformer: TransformerStage,
    pub oversampler: Oversampler2x,
    pub sample_rate: f32,

    // Metering ballistics
    gr_peak: f32,
    out_peak: f32,
}

impl Default for PyeChannel {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl PyeChannel {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            chopper: PwmChopper::new(sample_rate),
            transformer: TransformerStage::new(sample_rate),
            oversampler: Oversampler2x::new(),
            sample_rate,
            gr_peak: 0.0,
            out_peak: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.chopper.reset();
        self.transformer.reset();
        self.oversampler.reset();
        self.gr_peak = 0.0;
        self.out_peak = 0.0;
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(44100.0);
        self.chopper.update_sample_rate(self.sample_rate);
        self.transformer.update_sample_rate(self.sample_rate, self.sample_rate * 2.0);
    }

    /// Process a single audio sample through the complete Pye 4060 channel path.
    ///
    /// - `input_sample`: the audio sample to process
    /// - `sidechain_sample`: the sidechain sample to feed the Germanium detector (can be linked)
    /// - `threshold_db`: -24 to +16 dB
    /// - `ratio`: 1:1, 2:1, 3:1, 5:1, Limit
    /// - `decay`: 100ms, 200ms, 400ms, 800ms, 1600ms, 3200ms
    /// - `gate_thresh_rel_db`: -50 to -20 dB
    /// - `gate_atten_db`: 0 to 10 dB
    /// - `output_db`: -18 to +18 dB
    /// - `analog`: Off, 50Hz, 60Hz
    #[inline(always)]
    pub fn process_sample(
        &mut self,
        input_sample: f32,
        sidechain_sample: f32,
        threshold_db: f32,
        ratio: PyeRatio,
        decay: PyeDecay,
        gate_thresh_rel_db: f32,
        gate_atten_db: f32,
        output_db: f32,
        analog: AnalogMode,
    ) -> f32 {
        self.chopper.set_decay(decay);

        // 1. Input Transformer T1 & Sidechain Transformer T4 conditioning
        let x_t1 = self.transformer.process_input_transformer(input_sample);
        let sc_t1 = self.transformer.process_sidechain_transformer(sidechain_sample);

        // 2. PWM Chopper Attenuation (with Germanium detector and Noise Gate)
        let (x_chopped, gr_db) = self.chopper.process(
            x_t1,
            sc_t1,
            threshold_db,
            ratio,
            gate_thresh_rel_db,
            gate_atten_db,
        );

        // Track peak Gain Reduction for metering
        if gr_db > self.gr_peak {
            self.gr_peak = gr_db;
        } else {
            self.gr_peak += 0.0005 * (gr_db - self.gr_peak);
        }

        // 3. 5-Pole LC Anti-Carrier Filter (strips carrier above 18-20 kHz)
        let x_lc = self.transformer.process_lc_filter(x_chopped);

        // 4. Discrete Output Driver & Transformer T2 with 2x Oversampling
        let (up0, up1) = self.oversampler.upsample2(x_lc);
        let sat0 = self.transformer.process_output_saturation(up0);
        let sat1 = self.transformer.process_output_saturation(up1);
        let x_out = self.oversampler.downsample2(sat0, sat1);

        // 5. Output Makeup Gain
        let makeup_lin = 10.0f32.powf(output_db / 20.0);
        let mut y = x_out * makeup_lin;

        // 6. Analog Power Supply Hum
        let hum = self.transformer.generate_analog_hum(analog);
        y += hum;

        // Track peak output level for metering
        let y_abs = y.abs();
        if y_abs > self.out_peak {
            self.out_peak = y_abs;
        } else {
            self.out_peak += 0.0003 * (y_abs - self.out_peak);
        }

        y
    }

    /// Read normalized Gain Reduction meter [0.0, 1.0], representing 0 to 20 dB GR
    #[inline(always)]
    pub fn meter_gr(&self) -> f32 {
        (self.gr_peak / 20.0).clamp(0.0, 1.0)
    }

    /// Read normalized Output Level meter [0.0, 1.0] calibrated for VU (-20 to +6 dBVU)
    #[inline(always)]
    pub fn meter_out(&self) -> f32 {
        let dbfs = if self.out_peak > 1e-5 {
            20.0 * self.out_peak.log10()
        } else {
            -100.0
        };
        // 0 dBVU = -18 dBFS (+4 dBu). Range -20 dBVU (-38 dBFS) to +6 dBVU (-12 dBFS)
        let dbvu = dbfs + 18.0;
        ((dbvu + 20.0) / 26.0).clamp(0.0, 1.0)
    }
}
