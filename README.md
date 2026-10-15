# Jardim Sonoro Pye 4060 — Classic PWM Compressor & Limiter

[![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Validation](https://img.shields.io/badge/pluginval-Strictness%20Level%205%20PASS-brightgreen.svg)]()
[![Format](https://img.shields.io/badge/formats-CLAP%20%7C%20VST3-blue.svg)]()
[![Architecture](https://img.shields.io/badge/architecture-Zero--Allocation%20DSP-orange.svg)]()
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

An ultra-faithful, component-level DSP reconstruction of the legendary **Pye TVT 84-4060 / 4060/01** solid-state dynamics processor. Developed originally in Cambridge, England by Pye TVT Limited for BBC broadcast transmission, the unit was immortalized in rock and roll history by legendary producer/engineer **Eddie Kramer** at London's **Olympic Sound Studios**, shaping the explosive drum and vocal sounds of **Jimi Hendrix**, **Led Zeppelin**, and **The Rolling Stones**.

Built in Rust using the modern [Truce Audio Framework](https://github.com/truce-audio/truce), this plugin combines deep forensic circuit analysis, original schematic extraction, and empirical measurement against vintage reference plugins to deliver an analog emulation with zero audio thread memory allocations and strict compliance with Steinberg's `pluginval` test harness.

---

## 📸 Interface Preview

![Pye 4060 UI](screenshot.png)

*Ergonomic, strictly height-clamped 19" rack console layout featuring Dual Mono, Stereo, and Mid/Side (M/S) processing, authentic moving-coil Gain Reduction & VU meters, switched RC decay network, and the integrated broadcast Noise Gate (Type 845751/01).*

---

## 🏛️ Engineering Rigor & Forensic Hardware Modeling

Unlike modern compressors based on VCAs (THAT/dbx), optical photo-resistors (LA-2A), FETs (1176), or variable-mu tubes (Fairchild 660), the Pye TVT 84-4060 achieves dynamic gain reduction via **Pulse Width Modulation (PWM) Chopper Switching**.

### 1. The PWM Chopper Attenuation Engine
- **Carrier Oscillator:** A discrete blocking oscillator (`VT13 2N4062`, `VT14 2N4058`) generates ultra-high-frequency pulses at ~250 kHz (period $4\ \mu\text{s}$).
- **Chopper Switch (`VT2 V405`):** Modulates the mark-to-space (duty cycle) ratio according to the rectified control voltage.
- **Physical Attenuation Law:**
  $$\text{Gain}(t) = \frac{T_{\text{open}}}{T_{\text{period}}} = D(t) = 10^{-\text{Atten}_{\text{dB}}(t) / 20}$$
  - **At Threshold:** Switch is 100% open $\to$ 0 dB attenuation.
  - **+6 dB above Threshold:** Switch is 50% open, 50% closed $\to$ removes 50% energy $\to$ 6 dB attenuation.
  - **+12 dB above Threshold:** Switch is 25% open, 75% closed $\to$ removes 75% energy $\to$ 12 dB attenuation.
- **Ultra-Fast Attack:** Because the chopper operates at 250 kHz, gain changes occur in sub-millisecond times (~0.8 ms), cleanly clamping transients and sibilants without transient click artifacts.

### 2. Germanium Bridge Rectifier & The Automatic +8 dB LIMIT Offset
- **Detector Stage:** Sidechain transformer `T4 772201` drives a full-wave Germanium diode bridge (`OA47`). The low forward turn-on voltage (~0.25V) and smooth exponential conduction knee create musical, gradual threshold transitions.
- **Automatic +8 dB Limit Turnover Shift:** In accordance with the original Pye manual (Section 2, Chapter 2):
  > *"The turnover point in the limiting mode is automatically set 8 dB above the level indicated by the threshold control, so that limiting takes place correctly at the 100% modulation values, when the control is set to the incoming programme level (40% modulation)."*
  In **Limit** mode, the ratio is locked at 20:1 and the threshold turnover shifts +8 dB higher, reproducing the exact hardware behavior verified against Audio Precision test curves.

### 3. Switched RC Decay Network ($C_{23} = 22\ \mu\text{F}$)
The release timing is governed by an authentic 6-position switched resistor array discharging capacitor $C_{23}$ ($22\ \mu\text{F}$):
- `100 ms` ($R_{54} = 1.8\ \text{k}\Omega$)
- `200 ms` ($R_{53} = 4.7\ \text{k}\Omega$)
- `400 ms` ($R_{52} = 10\ \text{k}\Omega$ — Classic Olympic vocal/drum standard)
- `800 ms` ($R_{51} = 22\ \text{k}\Omega$)
- `1600 ms` ($R_{50} = 47\ \text{k}\Omega$)
- `3200 ms` ($R_{49} = 120\ \text{k}\Omega$)

### 4. Integrated Broadcast Noise Gate (Type 845751/01)
*Omitted in legacy digital emulations such as Waves Kramer PIE, but faithfully restored here from original Pye schematics:*
- **Operating Threshold:** Continuously variable from -20 dB to -50 dB below the compression threshold.
- **Preset Attenuation:** Switched steps of 0 dB (Off), 2 dB, 4 dB, 6 dB, 8 dB, and 10 dB.
- **Ballistics:** 250 ms hold/release into smooth downward expansion; rapid 1 ms attack when signal rises above threshold.

### 5. 5-Pole LC Anti-Carrier Filter (`L1, L2 52.5mH`, `C5-C7 0.0033uF`)
A discrete 5th-order LC low-pass filter strips the 250 kHz PWM carrier from the audio signal. Calibrated to factory tolerances (+0.5 to -1.0 dB from 30 Hz to 15 kHz) with progressive roll-off above 16 kHz (-6 dB at 18 kHz, -12 dB at 20 kHz).

### 6. Discrete Push-Pull Output Driver & Class-A $H_2$ Asymmetry
Driven by discrete bipolar transistors (`VT9 2N4062`, `VT10 2N3053`) into custom output transformer `T2 772246`:
- **Magnetic Flux Weighting ($1/f$):** According to Faraday's law ($\Phi \propto V / \omega$), low frequencies force greater flux density through the transformer core. Bass saturates earlier while high frequencies stay pristine.
- **Even-Harmonic ($H_2$) Asymmetry:** Discrete single-ended and push-pull bias points compress asymmetric half-cycles unequally:
  $$y = x - \alpha_{\text{even}} \frac{x^2}{1 + |x|} - \beta_{\text{odd}} x^3$$
  Producing authentic analog warmth with dominant 2nd harmonic ($H_2$) verified via stepped-sine analysis.
- **Subsonic DC Blocking:** 5 Hz high-pass filter eliminates any DC drift from even-order rectification.

---

## 🔄 Signal Processing Architecture

```
                    ┌────────────────────────────────────────────────────────┐
                    │               STEREO / M-S MATRIX ENCODER              │
                    └───────────────────────────┬────────────────────────────┘
                                                │
                     [Left / Mid]               │               [Right / Side]
                          │                     │                     │
                          ▼                     │                     ▼
              ┌───────────────────────┐         │         ┌───────────────────────┐
              │  Input Transformer T1 │         │         │  Input Transformer T1 │
              └───────────┬───────────┘         │         └───────────┬───────────┘
                          │                     │                     │
                          ▼                     │                     ▼
        ┌───────────────────────────────────┐   │   ┌───────────────────────────────────┐
        │        PWM CHOPPER ENGINE         │   │   │        PWM CHOPPER ENGINE         │
        │  • Sidechain Transformer T4       │   │   │  • Sidechain Transformer T4       │
        │  • Germanium OA47 Bridge Rectifier│◄──┼──►│  • Germanium OA47 Bridge Rectifier│
        │  • Switched RC Decay (100-3200ms) │   │   │  • Switched RC Decay (100-3200ms) │
        │  • +8 dB Limit Turnover Shift     │   │   │  • +8 dB Limit Turnover Shift     │
        │  • Broadcast Noise Gate (0-10 dB) │   │   │  • Broadcast Noise Gate (0-10 dB) │
        └─────────────────┬─────────────────┘   │   └─────────────────┬─────────────────┘
                          │                     │                     │
                          ▼                     │                     ▼
              ┌───────────────────────┐         │         ┌───────────────────────┐
              │ 5-Pole LC LP Filter   │         │         │ 5-Pole LC LP Filter   │
              │ (Stripped 250kHz PWM) │         │         │ (Stripped 250kHz PWM) │
              └───────────┬───────────┘         │         └───────────┬───────────┘
                          │                     │                     │
                          ▼                     │                     ▼
              ┌───────────────────────┐         │         ┌───────────────────────┐
              │ 2x Polyphase Halfband │         │         │ 2x Polyphase Halfband │
              │ Oversampling Engine   │         │         │ Oversampling Engine   │
              └───────────┬───────────┘         │         └───────────┬───────────┘
                          │                     │                     │
                          ▼                     │                     ▼
              ┌───────────────────────┐         │         ┌───────────────────────┐
              │ Output Transformer T2 │         │         │ Output Transformer T2 │
              │ • 1/f Flux Weighting  │         │         │ • 1/f Flux Weighting  │
              │ • Class-A H2 Saturation         │         │ • Class-A H2 Saturation
              └───────────┬───────────┘         │         └───────────┬───────────┘
                          │                     │                     │
                          ▼                     │                     ▼
              ┌───────────────────────┐         │         ┌───────────────────────┐
              │ Makeup Gain (-18/+18) │         │         │ Makeup Gain (-18/+18) │
              └───────────┬───────────┘         │         └───────────┬───────────┘
                          │                     │                     │
                          ▼                     │                     ▼
                    ┌───────────────────────────┴────────────────────────────┐
                    │               STEREO / M-S MATRIX DECODER              │
                    └───────────────────────────┬────────────────────────────┘
                                                │
                                                ▼
                                    Output [Left / Right]
```

---

## 🔬 Empirical Validation Suite

An automated, phase-independent stepped-sine measurement suite (`tools/test_full.py`) verifies our DSP model against Waves Kramer PIE Stereo and factory specifications across 8 critical tests:

| Test ID | Description | Benchmark / Spec | Result |
|:---:|:---|:---|:---:|
| **Test 1** | **Hard Bypass Passthrough** | Max error $< 10^{-6}$ | **0.00e+00** (PASS) |
| **Test 2** | **Frequency Response Sweep** | +0.5 to -1.0 dB (30 Hz - 15 kHz); 5-pole LC roll-off | **±0.02 dB at 1kHz** (PASS) |
| **Test 3** | **Static Compression Curves** | 1:1, 2:1, 3:1, 5:1, Limit across -40 to +2 dBFS | **Matched** (PASS) |
| **Test 4** | **+8 dB Limit Turnover Offset** | +8 dB turnover elevation in Limit mode | **Confirmed** (PASS) |
| **Test 5** | **Switched Decay Ballistics** | Monotonic recovery across 100ms - 3200ms | **Verified** (PASS) |
| **Test 6** | **Harmonic Spectrum Analysis** | Dominant 2nd harmonic ($H_2 > H_3$) Class-A warmth | **THD 0.20%, $H_2 \gg H_3$** (PASS) |
| **Test 7** | **Multi-Channel & M/S Routing** | Dual Mono isolation + M/S zero leakage | **0.00e+00 leakage** (PASS) |
| **Test 8** | **Integrated Noise Gate** | Exact 6.00 dB attenuation with 250ms hold/release | **6.00 dB steady state** (PASS) |

### Steinberg Pluginval Compliance
Validated under Steinberg's official test harness at **Strictness Level 5**:
```bash
cargo truce validate --pluginval
# pluginval (VST3)
#   Pye4060 ... PASS
# All validations passed.
```

---

## 📦 Installation & Quickstart

### Prerequisites
- [Rust & Cargo](https://rustup.rs/) (v1.85+ recommended)
- [Truce CLI](https://github.com/truce-audio/truce) (`cargo install cargo-truce`)

### Compile and Install to User Plugin Directory
```bash
# Clone the repository
git clone https://github.com/nagualcode/pye4060.git
cd pye4060/pye4060

# Build release bundles (CLAP + VST3)
cargo truce build

# Install directly to macOS user audio plug-in folders
# (~/Library/Audio/Plug-Ins/VST3/ and ~/Library/Audio/Plug-Ins/CLAP/)
cargo truce install
```

In your DAW (Ableton Live, Reaper, Logic Pro, Cubase, Studio One, Bitwig, Pro Tools), the plugin will appear under vendor **Jardim Sonoro** in the **Dynamics** category:
- **Display Name:** `Pye4060`
- **Vendor:** `Jardim Sonoro`
- **Formats:** VST3, CLAP, Standalone

---

## 🎛️ Control Reference

### Master Section
- **Processing Mode:**
  - `Dual Mono`: Completely independent Channel A (Left) and Channel B (Right) strips.
  - `Stereo`: Channel A controls both channels synchronously with linked sidechain detection.
  - `M/S`: Channel A processes Mid ($M = (L+R)/\sqrt{2}$), Channel B processes Side ($S = (L-R)/\sqrt{2}$).
- **Link A-B:** Links parameter adjustments and couples sidechains between channels.
- **Analog Hum:** Selectable `Off`, `50 Hz` (authentic British broadcast mains), or `60 Hz` (US/Olympic export mains) modeled at -86 dBFS.
- **Bypass:** Bit-clean, zero-latency wire bypass.

### Channel Strips (A & B)
- **Threshold:** Input dynamics reference point (-24 dB to +16 dB in 2 dB steps, continuous interpolating).
- **Ratio:** 5-position switch:
  - `1:1`: Linear calibration / passthrough.
  - `2:1`: Gentle bus leveling.
  - `3:1`: Classic Olympic rock drum and vocal compression.
  - `5:1`: Punchy limiting / bus compression.
  - `Limit`: Brickwall limiter with 20:1 ratio and automatic **+8 dB turnover offset**.
- **Decay Time:** Switched 6-position release time constants: `100 ms`, `200 ms`, `400 ms`, `800 ms`, `1600 ms`, `3200 ms`.
- **Noise Gate Threshold:** -50 dB to -20 dB relative to compression threshold.
- **Noise Gate Attenuation:** Switched preset attenuation: `0 dB (Off)`, `2 dB`, `4 dB`, `6 dB`, `8 dB`, `10 dB`.
- **Output Gain:** Linear makeup gain (-18 dB to +18 dB).
- **Meters:**
  - `GR`: Moving-coil meter indicating instantaneous and peak-smoothed Gain Reduction (0 to 20 dB).
  - `VU Out`: Calibrated output level meter (-20 dBVU to +6 dBVU, where 0 dBVU = -18 dBFS).

---

## 📋 Roadmap & Future Improvements (ToDo)
- [ ] **External Sidechain (Sidechain Input):** Expose auxiliary sidechain bus in VST3/CLAP for external ducking and sidechain keying.
- [ ] **Custom Skeuomorphic GUI Skin:** Vintage hammertone grey faceplate, Marconi-style knurled pointer knobs, and animated moving-coil meter needle.
- [ ] **High-Pass Sidechain Filter (HP SC):** Optional selectable 80 Hz / 120 Hz sidechain low-cut filter for modern low-end bass management on drum buses.
- [ ] **Mix / Wet-Dry Blend Knob:** Integrated parallel compression control for NY-style parallel smashing directly within the plugin.
- [ ] **AU v2 / AU v3 Apple Component Packaging:** Sign and notarize `.component` and `.appex` bundles for Logic Pro and iOS.

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
Audio DSP framework powered by [Truce Audio](https://github.com/truce-audio/truce).
Original Pye TVT schematics and BBC historical documentation referenced for educational and forensic circuit analysis purposes.
