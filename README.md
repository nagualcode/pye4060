# Jardim Sonoro Pye 4060 — PWM Compressor / Limiter

[![Validation](https://img.shields.io/badge/pluginval-PASS-brightgreen.svg)]()
[![Format](https://img.shields.io/badge/formats-CLAP%20%7C%20VST3-blue.svg)]()
[![Architecture](https://img.shields.io/badge/DSP-zero--allocation-orange.svg)]()
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A component-level model of the **Pye TVT 4060 compression amplifier** (circuit 845751/01), the pulse-width-modulation compressor/limiter built in Cambridge for BBC and ITV studio desks and used by **Eddie Kramer** at Olympic Studios on Jimi Hendrix, Led Zeppelin and Rolling Stones sessions.

Written in Rust on the [Truce](https://github.com/truce-audio/truce) framework. The model is derived from the original 845751/01 schematic, the Pye instructions and maintenance manual (types 4060/01 and 4060/03) and the BBC Operations Manual pages, and is calibrated against black-box measurements of the Waves Kramer PIE (itself modelled on a unit at -18 dBFS = +4 dBu).

---

## 📸 Interface

![Pye 4060 UI](screenshot.png)

Two channel modules laid out like the dual 4060 frame: the attenuation meter on top (Pye 0–24 dB scale, moving-coil ballistics), **NOISEGATE THRESHOLD** with the attenuation switch mounted concentrically inside it as on the hardware, the detented **THRESHOLD** dial, then **DECAY TIME ×100 ms** and **COMPRESSION RATIO**. The centre strip carries the dual frame's **LINK** toggle plus two modern additions, **MIX** and **SIDECHAIN HPF**. Flat dark styling, Pye-orange accents.

Drag to adjust (Shift = fine, and frees the threshold from its 2 dB detents), double-click for default, mouse wheel steps, click a scale legend to jump to that switch position.

---

## 🎸 Sound lineage

The 4060 was never a "studio compressor" product in the modern sense. It came out of a broadcast console programme, and its sound travelled from television studios into rock records.

1. **Pye TVT, Cambridge (c. 1960).** Pye's audio department under Bob Barrass designed a modular transistor mixing desk built from plug-in "cassettes". The 4060 compression amplifier was one of those cassettes. It is the part of that programme that outlived everything else.
2. **ITV and BBC television (1960s).** ATV's four 34-channel desks at Elstree (from 1960–61) each carried four 4060s, and Tyne Tees had one in its 24-channel desk. The BBC specified the unit for the Pye desks in Television Centre studio TC1 (opened 1964) and the Television Theatre. Later versions (Pye 842678/00, BBC LF104) went into BBC DK4 and EMI desks, and the BBC Type D desk in TC8 used the original 4060 with the threshold set about 8 dB higher. BBC sound supervisor Mike McCarthy remembered the TC1 limiters for how they "scrunched up" audience applause: "very harsh sound, which was very popular at the time".
3. **Olympic Studios, London (late 1960s).** Eddie Kramer has said almost everything he recorded there passed through Pye compressors: Jimi Hendrix, Led Zeppelin, the Rolling Stones. The units were popular enough that Neve built a replacement compressor in the same form factor.
4. **Digital and boutique descendants.** Waves modelled a unit with Kramer as the Kramer PIE. Modern hardware such as the 500-series "Pynamic" PWM limiter (photos in `reference/pye_clone/`) keeps the PWM topology and adds mix, high-pass and attack controls.
5. **This plugin** goes back to the circuit itself, the 845751/01 drawing (Issue 1, 12/6/68), and uses the Waves model only as a measuring stick.

A pair of original units was recently listed for about £14,000.

---

## 🔎 Research process

Every behaviour in the plugin traces back to one of these sources (all in `reference/`):

| Source | What it settled |
|:---|:---|
| **Circuit 845751/01 schematic** (Pye TVT, Issue 1, 12/6/68) | Topology: side chain taken *after* the chopper (feedback); component values for the LC filter (L1/L2 52.5 mH, C5–C7 3.3 nF), the C23 22 µF timing capacitor and its SWA decay ladder (R49–R54), the SWB2/SWB3 ratio resistor sets, the OA47 germanium bridge, the noise-gate board (FET pad R119–R122), and the LINK wiring (PL1 pins 6 and 10). |
| **Instructions and Maintenance Manual**, types 4060/01 & 4060/03 | Compression and limiting curves (Fig. 2.2), the +8 dB limit turnover, the mark:space principle (Fig. 2.3), the block diagram (Fig. 2.4), waveform distortion with short decay times (Fig. 2.5), noise-gate timing, and the spec: +0.5/−1 dB 30 Hz–15 kHz, ±24 dBm. |
| **BBC Operations Manual** pages (John Eden-Eadon, revised 1971, in `history.pdf`) | 250 kHz chopper with a 4 µs period; threshold as "a pair of opposed attenuators"; attack under 0.5 ms (compression) and 1 ms ± 0.5 (limit); 2 dB threshold steps over 40 dB. |
| **Photos of a dual frame** with PS2/49/1742 supply | Front-panel layout, the concentric noise-gate controls, the meter's non-linear 0–24 dB scale, and the LINK toggle between the modules. |
| **Waves Kramer PIE manual** and plugin | Calibration level (−18 dBFS = +4 dBu), hum/noise behaviour, and measured data (next section). |

Process:
1. **Read the circuit.** We traced the schematic stage by stage and found that the side chain is a feedback loop, not the feedforward detector of v0.1.0. That changed the whole DSP architecture.
2. **Measure the reference.** We characterized the Waves model as a black box (below) to get numbers that documents don't give: time constants, curve shapes, distortion spectra.
3. **Find the mechanism that explains the data.** Candidate physical models were prototyped in Python and fitted, and a model was kept only when it explained several measurements at once. Example: the hyperbolic chopper gain law `g = 1/(1+V)` fits every recovery curve with one time constant per decay position, at any depth.
4. **Port to Rust and re-measure** the built plugin with the same harness, side by side with the reference, until the curves agree or there is a documented reason to differ.
5. **Fill gaps from comparable vintage gear**, only where neither documents nor measurements decide: the class-A amplifier's H2 balance and the output transformer's core flux.

---

## 🧪 Reverse engineering the reference

The Waves Kramer PIE was analysed as a **black box**, through measurement only. It was not disassembled or decompiled. `tools/compare_waves.py` loads it with [pedalboard](https://github.com/spotify/pedalboard) and runs the same stimuli through both plugins:

- **Frequency response** with stepped sines at 44.1 and 96 kHz. The roll-off is identical at both rates (−33 dB at 30 kHz, −43 dB at 40 kHz, about 30 dB/octave), so it is a modelled 5-pole analog filter, not an anti-alias artefact. A symmetric ideal LC ladder could not reproduce it; a fitted 5-pole response plus a T2 resonance does (±0.13 dB).
- **Static curves** in 1 dB steps for each ratio and threshold. They show the linear threshold map (turnover = dial − 20 dBFS peak), the 8.2 dB limit offset, a hard knee, about 25:1 in limit, and attenuation saturating near 21.4 dB.
- **Attack.** A step was analysed per half-cycle. It is fast at first and then creeps toward the final value, with overshoot in limit, which is typical of a feedback loop.
- **Release.** Recovery curves were taken at two depths for each decay position. The hyperbolic law fits all of them at 0.07 dB RMS. They yield the fitted C23 time constants of 53, 96, 155, 327, 785 and 1685 ms.
- **Distortion vs. decay, frequency and level.** The H3-dominant, decay-dependent distortion identified control-voltage ripple as the main harmonic mechanism. A 1:1 drive sweep located the clipping after the chopper, about 21 dB over the turnover.
- **Detector and noise.** Stereo detection is max-linked. Hum and noise spectra were checked per mains setting.

The reference informed calibration. The architecture comes from the schematic. Where the two disagree, the hardware documents win; the departures are listed under *Measured against the reference*.

---

## 🏛️ How the model works

### 1. A feedback compressor
The schematic and the manual's block diagram (Fig. 2.4) put the side chain **after** the chopper, LC filter and amplifier: T4 (772201) → OA47 bridge (MR4–7) → ratio amplifier (VT15/VT16, SWB2/SWB3) → C23 22 µF (charged by VT13/VT14, discharged by the SWA decay resistors) → blocking oscillator T3 → VT2 V405 shunt switch. The plugin runs that loop sample by sample at the oversampled rate, so the compressor reacts to its own output, as the hardware does.

### 2. The PWM chopper and its gain law
The 250 kHz chopper is modelled by its mark:space average (the carrier itself is ~120 dB down after the 5-pole filter and inaudible). The gain law is hyperbolic in the C23 voltage, `g = 1 / (1 + V)`. With an exponential RC discharge this reproduces the measured recovery shape — near-linear in dB while deep, an exponential tail near unity — to **0.07 dB RMS at every depth with a single time constant**; a dB-domain law does not.

The ratio amplifier is a power law of the rectified output, `V = (|out| / turnover)^(ratio−1) − 1`, which inside the loop yields exactly the constant ratios of manual Fig. 2.2. Attenuation tops out softly near 21.5 dB (minimum pulse width).

### 3. Program-dependent distortion — the "scrunch"
Because the gain is applied inside the loop, C23 ripples between waveform peaks and modulates the audio at audio rate (manual Fig. 2.5). That gives the 4060 its odd-order signature, rising at low frequencies and with short decay settings — the sound BBC engineers described as "squeench". This emerges from the circuit model; it is not a distortion curve added on top.

### 4. Threshold as opposed attenuators
"The threshold control operating as a pair of opposed attenuators, one preceding the input amplifier and the other controlling the amount of feedback over the output amplifier" (BBC Ops Manual). Gain stays unity, but the amplifiers work relative to the turnover: low thresholds drive the post-chopper amplifier harder (H2, then soft clipping about 21 dB over the turnover), and input-referred hiss rises with the threshold setting and breathes with the gain reduction. Dial 0 puts the turnover at −20 dBFS sine peak; **Limit** moves it 8 dB up ("so that limiting takes place correctly at the 100 % modulation values").

### 5. Time constants
- **Attack** (fixed, as on the unit): C23 charges through a current-limited stage. Compression reaches 63 % in < 0.5 ms and limiting in ≈ 1 ms, per the BBC spec sheet ("Compression less than 0.5 m.secs. Limit 1 m.sec. ±0.5"). Fast transients overshoot before the loop catches them, as in Fig. 2.5.
- **Decay**: the panel's 1–32 ×100 ms are nominal recovery times. The C23 discharge constants are fitted to the reference unit's recovery curves: 53, 96, 155, 327, 785 and 1685 ms.

### 6. Filters and transformers
The LC anti-carrier filter (L1/L2 52.5 mH, C5–C7 3.3 nF) together with T1/T2 is a fitted 5-pole response plus the T2 leakage resonance: +0.45 dB at 10 kHz, −0.3 dB at 15 kHz, −6.7 dB at 18 kHz, −12 dB at 20 kHz, ~30 dB/octave beyond, and an 11.5 Hz two-pole low end — inside the manual's +0.5/−1 dB (30 Hz–15 kHz). The high-Q sections use magnitude-matched designs (Vicanek) so the response holds at every sample rate. The output stage (VT6–VT9 into T2) adds low-frequency core flux and push-pull rounding at high output levels.

### 7. Noise gate (845751/01 gate board)
Senses the programme after the input amplifier, so its −20…−50 dB threshold is relative to the compression threshold. Below threshold for more than 250 ms it fades in the selected 2–10 dB pad with a 250 ms time constant; it opens again in under 1 ms (manual ch. 1).

### 8. Routing and LINK
**Dual Mono**, **Stereo** (one detector, channel A's controls drive both) and **M/S**. **LINK** behaves like the dual frame's switch, which joins PL1 pins 6 and 10 of both units: the two C23 control voltages are paralleled, so both channels share one gain while keeping their own knobs.

### 9. Analog
Off (digital silence in, silence out), or 50/60 Hz: rectifier ripple and transformer hum plus transistor hiss.

### Signal flow (per channel, at 176.4–192 kHz)
```
in ─▶ T1 ─▶ VT1 input amp ─┬─▶ chopper VT2 ─▶ LC filter + T2 ─▶ VT3–VT5 amp ─┬─▶ output atten. ─▶ VT6–VT9 / T2 ─▶ out
   (threshold attenuator)  │        ▲  mark:space                           │   (+ gate pad)
                           │        └── C23 / blocking osc ◀── ratio amp ◀── OA47 bridge ◀── T4 (+ SC HPF)
                           └─▶ noise gate detector
```
Oversampling is 4× at 44.1/48 kHz and 2× at 88.2/96 kHz with polyphase elliptic half-band filters (≈100 dB alias rejection, flat to 20.2 kHz). No latency is reported or needed. CPU: about 1 % of one core for a stereo instance at 48 kHz.

---

## 🔬 Measured against the reference

`tools/compare_waves.py` drives this plugin and the Waves Kramer PIE (Mono) with identical stimuli. Representative results (1 kHz sine, threshold 0):

| Measurement | Pye4060 | Waves PIE | Notes |
|:---|:---:|:---:|:---|
| Frequency response, 30 Hz – 20 kHz | 15k −0.25, 18k −6.7, 20k −12.5 dB | 15k −0.30, 18k −6.7, 20k −12.4 dB | within ±0.3 dB of the reference throughout |
| GR, 3:1, +6 dBFS in | 16.9 dB | 16.9 dB | 5:1 and Limit within ~0.3 dB from turnover to +6 dBFS |
| GR at −10 dBFS, threshold −16 / −8 / 0 / +8 | 16.9 / 11.8 / 6.5 / 1.2 dB | 16.9 / 11.9 / 6.8 / 1.3 dB | threshold mapping |
| Recovery, 3:1, 400 ms position, 100/200/400 ms after release | 11.6 / 7.9 / 3.0 dB | 11.0 / 7.4 / 2.8 dB | same shape at every decay position |
| H3, 3:1, 15 dB GR, 100 ms decay — 1 kHz / 100 Hz / 50 Hz | −56 / −37 / −33 dB | −56 / −37 / −34 dB | loop-ripple distortion |
| H3, 3:1, 15 dB GR, 3.2 s decay, 1 kHz | −86 dB | −84 dB | |

Where the model deliberately departs from the reference:
- **2:1 turnover**: the reference measures its 2:1 knee ~2.6 dB above 3:1. The model follows the manual's Fig. 2.2, where all ratios share the threshold point.
- **Attack**: faster than the reference, to meet the BBC attack specification.
- **H2**: one physically placed class-A nonlinearity for every mode. The reference has almost no H2 under compression (≈ −65 dB) but plenty in 1:1; the model sits at ≈ −52 dB under heavy compression and somewhat cleaner than the reference in 1:1.

### Tests
- `cargo test --release --lib`: 14 unit tests — static ratios, +8 dB limit turnover, BBC attack spec, maximum attenuation, filter accuracy, oversampler flatness and alias rejection, unity gain below threshold, digital silence, M/S isolation, LINK behaviour, noise-gate timing.
- `tools/test_full.py`: end-to-end checks on the installed VST3 (bypass, curves, limit offset, decay ordering, harmonics, routing, gate).
- `cargo truce validate --pluginval`: VST3 passes.

---

## 📦 Installation & Quickstart

### Download (macOS, Apple Silicon)
Grab `Pye4060-v0.2.0-macOS-arm64.zip` from [Releases](https://github.com/nagualcode/pye4060/releases), then:
```bash
unzip Pye4060-v0.2.0-macOS-arm64.zip
cd Pye4060-v0.2.0-macOS-arm64
cp -R Pye4060.vst3 ~/Library/Audio/Plug-Ins/VST3/
cp -R Pye4060.clap ~/Library/Audio/Plug-Ins/CLAP/
# The bundles are ad-hoc signed, not notarised: clear the download quarantine.
xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/Pye4060.vst3 ~/Library/Audio/Plug-Ins/CLAP/Pye4060.clap
```
Intel Macs and other platforms: build from source.

### Prerequisites
- [Rust & Cargo](https://rustup.rs/) (1.92+)
- [Truce CLI](https://github.com/truce-audio/truce) (`cargo install cargo-truce`)

### Build and install
```bash
git clone https://github.com/nagualcode/pye4060.git
cd pye4060/pye4060
cargo truce build      # CLAP + VST3 (+ standalone)
cargo truce install    # ~/Library/Audio/Plug-Ins/{VST3,CLAP}
```
The plugin appears under vendor **Jardim Sonoro**, category **Dynamics**, as `Pye4060`.

---

## 🎛️ Control Reference

### Master
- **Mode**: `Dual Mono` (independent A = left, B = right), `Stereo` (one detector; channel A's controls drive both), `M/S` (A = mid, B = side; M = (L+R)/√2, S = (L−R)/√2).
- **Link**: joins the two C23 control voltages (Dual Mono / M/S). Always shared in Stereo.
- **Analog**: `Off`, `50 Hz`, `60 Hz` — hum and hiss.
- **Bypass**: bit-exact wire.
- **Mix** *(modern)*: wet/dry blend for parallel compression; the dry path runs through matching resampling filters so the blend stays phase-coherent.
- **Sidechain HPF** *(modern)*: Off / 60 / 90 / 150 Hz high-pass ahead of the bridge rectifier, to keep low end from driving the gain.

### Channel (A and B)
- **Threshold**: −24 … +16 in 2 dB detents (Shift-drag for in-between values). 0 = the unit's normal operating position.
- **Ratio**: `1:1` (side chain off, for line-up), `2:1`, `3:1`, `5:1`, `Limit` (≈25:1, turnover +8 dB).
- **Decay**: 1, 2, 4, 8, 16, 32 ×100 ms.
- **Noisegate threshold**: −50 … −20 dB relative to the compression threshold. **Attenuation**: 0 (off), 2, 4, 6, 8, 10 dB.
- **Output** *(plugin trim)*: −18 … +18 dB, after the hardware chain.
- **Meters**: attenuation (Pye 0–24 dB scale, needle with peak marker) and output level (VU ballistics, sample-peak tick, clip LED; green to −18 dBFS = 0 VU).

Defaults follow the hardware's normal position: Stereo, threshold 0, 3:1, decay 4 (×100 ms).

---

## 🎚️ Factory Preset Library (21 Presets)

The plugin includes an authentic factory preset library organized by instrument and session type, including all classic **Eddie Kramer / Olympic Studios** settings extracted from the original hardware model, alongside broadcast and modern M/S mastering presets:

| Category | Preset Name | Ratio | Decay | Setting / Sonic Purpose |
|:---|:---|:---:|:---:|:---|
| **Vocal** | `Lead Vocal Comp` | 3:1 | 100 ms | Fast attack transparent vocal leveling from classic Olympic sessions |
| **Drums** | `Kick Drum Comp` | 3:1 | 100 ms | Tight, punchy kick transient control |
| **Drums** | `Snare Drum Norm Comp` | 3:1 | 100 ms | Classic rock snare drum compression with fast snap |
| **Drums** | `Snare Drum SuperHype Comp` | Limit | 100 ms | Explosive brickwall limiter snare smash |
| **Drums** | `Toms Norm Comp` | 3:1 | 100 ms | Controlled tom sustain and stick impact |
| **Drums** | `Toms Super Smash` | Limit | 100 ms | Heavy tom pump and aggressive room sustain |
| **Drums** | `Boxes / Parallel Smash` | Limit | 100 ms | Parallel crushing preset for drum bus blending |
| **Drums** | `Hendrix Drum Smash (M-S)` | Limit / 3:1 | 100 ms / 200 ms | Aggressive brickwall limiting on center mono punch with dynamic stereo width |
| **Room** | `Medium Room Comp` | 2:1 | 100 ms | Ambient drum room enhancement with natural decay |
| **Room** | `Norm Close Room Comp` | 2:1 | 100 ms | Tight, controlled close room ambiance |
| **Room** | `Norm Room Comp` | 2:1 | 100 ms | Balanced room compression for cohesive space |
| **Room** | `Super Smash Close Rooms` | Limit | 100 ms | Massive explosive room energy (Bonham/Kramer style) |
| **Room** | `Super Smash Room` | Limit | 100 ms | All-out room compression and brickwall distortion |
| **Acoustic** | `AGTR Norm Comp` | 2:1 | 100 ms | Smooth acoustic guitar strumming leveling |
| **Acoustic** | `AGTR SMASH Comp` | 2:1 | 3200 ms | Slow release acoustic guitar sustain boost |
| **Bass** | `Bass Comp` | 3:1 | 200 ms | Warm, solid electric bass leveling with punchy note definition |
| **Bass** | `Bass Comp 2` | Limit | 200 ms | Limiter mode bass control for high-energy rock grooves |
| **Init** | `Default 3-1` | 3:1 | 400 ms | Factory default Pye TVT 84-4060 linear setup state |
| **Broadcast** | `BBC Broadcast Limiter` | Limit | 100 ms | 100% modulation brickwall limiter with British 50Hz hum and 4dB Noise Gate |
| **Mastering** | `Olympic Master Bus Glue (M-S)`| 2:1 | 400 ms / 800 ms | Subtle Mid/Side cohesion with decoupled timing |
| **Tracking** | `Dual Mono Tracking (Vocal & Bass)`| 3:1 | 200 ms / 400 ms | Ch A dialed for vocal presence and Ch B for bass foundation |

---

## 📋 Roadmap
- [ ] **External sidechain input** (VST3/CLAP auxiliary bus) for keying and ducking.
- [ ] **AU v2 / AU v3** packaging, signed and notarised.
- [ ] **Measure a real 4060**: the decay constants, H2 balance and 2:1 knee are calibrated against the Waves model; impulse, stepped-sine and recovery captures of a physical unit would settle them.
- [ ] **CLAP validation** with clap-validator in CI.

## 📄 License

This project is licensed under the [MIT License](LICENSE).
Audio DSP framework powered by [Truce Audio](https://github.com/truce-audio/truce).
Original Pye TVT schematics and BBC historical documentation referenced for educational and forensic circuit analysis purposes.
