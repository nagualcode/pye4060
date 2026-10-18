# Jardim Sonoro Pye 4060 — PWM Compressor / Limiter

[![Validation](https://img.shields.io/badge/pluginval%20%7C%20auval-PASS-brightgreen.svg)]()
[![Format](https://img.shields.io/badge/formats-AU%20%7C%20VST3%20%7C%20CLAP-blue.svg)]()
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
3. **Find the mechanism that explains the data.** Candidate physical models were prototyped in Python and fitted, and a model was kept only when it explained several measurements at once. Example: one small-signal charging constant for C23 (5 ms) closes the static curves at every decay position and every ratio at once, where the reference needs a calibration table.
4. **Port to Rust and re-measure** the built plugin with the same harness, side by side with the reference, until the curves agree or there is a documented reason to differ.
5. **Fill gaps from comparable vintage gear**, only where neither documents nor measurements decide: the class-A amplifier's H2 balance and the output transformer's core flux.

---

## 🧪 Reverse engineering the reference (Black-Box & White-Box)

The Waves Kramer PIE was analysed through both **black-box empirical measurements** (`tools/compare_waves.py` and `tools/test_full.py` via [pedalboard](https://github.com/spotify/pedalboard)) and **white-box binary inspection** (`KramerPIE.bundle` resources `ParamXML/1000.xml`, `ProcessXML/1001.xml`, `cKramerPIEALG`, and `GenericMacArm.dylib` ARM64 DSP routines):

- **No oversampling in the legacy plugin:** White-box inspection confirmed that the Waves Kramer PIE runs at **1× native sample rate**, merely switching pre-fitted biquad tables at sample-rate thresholds (`<= 46 kHz`, `> 46 kHz`, `> 64 kHz`, `> 92 kHz`, `> 120 kHz`). Because the chopper multiplies the programme by a fast-rippling gain and applies hard/soft clipping at 1× rate, the legacy plugin suffers heavy aliasing on high-frequency transients and saturation (`-23.6 dBc` H4 alias on a driven 7.5 kHz tone at 48 kHz, vs. **`-65.5 dBc` to `-131.5 dBc`** in `Pye4060` with 4× elliptic oversampling).
- **Method:** the per-sample loop was disassembled from the exported routines, and the coefficient block the knobs actually program was captured at run time (debugger breakpoint on the process entry, one capture per switch position). A Python re-implementation from those coefficients matches the plugin's output to −56…−89 dB after the start-up transient, so the reading below is exact, not inferred.
- **Feedback sidechain tap:** the loop is tapped after the LC anti-carrier filter (`OutputFilter`), *before* the output bell (T2 resonance), the output drive, the clip and the distortion stage, with a one-sample delay. The side chain has a first-order **low-shelf boost** (+2.1 dB below 17 Hz), not a high-pass; the programme itself is high-passed (3rd order, 19 Hz) before the tap.
- **Detector and ratio:** the rectifier is **linear with a bias**, `w = k·|y| − b`, with a different gain and bias per ratio position; the ratio emerges from them against a fixed chopper law. Release is an exponential return of `w` to its rest value (0.76) with `0.0585, 0.1064, 0.1707, 0.3604, 0.8335, 1.8535 s`; the side-chain gain is trimmed per decay position (+0.2 … +1.5 dB) to hold the static curve.
- **Chopper gain law:** `g = exp(P₄(ln w))`, a 4th-order polynomial in the log domain, clamped to `[0.08404, 1]` (−21.5 dB).
- **Distortion:** a one-sided **hard** clip on the negative half-cycle (`Soft Thresh NEG = −7.9 dB` is `0.4027`; the +50 dB positive threshold never engages), driven `∝ 10^(−threshold/20)` (the opposed attenuators), plus a pure even-order term `−x²/(k + 2|x|)` weighted by `GR_THD_Curve = g⁵/(g⁵ + 0.00067)`, which removes it below ~12 dB of attenuation. Noise is a fixed filtered floor (it does not breathe with the gain); hum is a filtered sawtooth.
- **Attack:** a ~4.8 ms one-pole on the instantaneous rectified output, slower in compression than in limit — the opposite of the BBC spec, which `Pye4060` follows.
- **Complete `845751/01` features omitted by Waves:** The legacy plugin completely omitted the **Noise Gate Printed Circuit Board** (`TR101–TR107`, `L101/C106` LC filter, `D101/D102 OA47`, `TR107 2N3819` FET pad `R119–R122`) and the dual-frame **LINK** switch that parallels the two `C23` timing capacitors (`PL1` pins 6 and 10). Both are modelled in full in `Pye4060`.

---

## 🏛️ How the model works

### 1. A feedback compressor
The schematic and the manual's block diagram (Fig. 2.4) put the side chain **after** the chopper, LC filter and amplifier: T4 (772201) → OA47 bridge (MR4–7) → ratio amplifier (VT15/VT16, SWB2/SWB3) → C23 22 µF (charged by VT13/VT14, discharged by the SWA decay resistors) → blocking oscillator T3 → VT2 V405 shunt switch. The plugin runs that loop sample by sample at the oversampled rate, so the compressor reacts to its own output, as the hardware does.

### 2. The PWM chopper and its gain law
The 250 kHz chopper is modelled by its mark:space average (the carrier itself is ~120 dB down after the 5-pole filter and inaudible). At rest C23 sits at the VT12 bias (`w = 0.76`, normalised); the blocking oscillator turns `w` into a mark fraction through a smooth monotonic law, `g = exp(P(ln w))`, unity up to `w = 0.767` and bottoming out at the minimum pulse width, **−21.5 dB**. The law is the one read from the reference model (white-box, above).

The ratio amplifier is **linear with a bias**, `w_target = k·|s| − b`: SWB2/SWB3 swap resistor sets, so they change the amplifier's gain and the OA47/VT16 conduction threshold, not an exponent. The compression ratios emerge from slope and bias against the chopper law, and so do the effective turnovers: +2.6 dB in `2:1` (the higher knee of the lower `R62` shunt), ~0 dB in `3:1`/`5:1`, +8.2 dB in `Limit`.

### 3. Program-dependent distortion — the "scrunch"
Because the gain is applied inside the loop, C23 ripples between waveform peaks and modulates the audio at audio rate (manual Fig. 2.5). That gives the 4060 its odd-order signature, rising at low frequencies and with short decay settings — the sound BBC engineers described as "squeench". This emerges from the circuit model; it is not a distortion curve added on top.

### 4. Threshold as opposed attenuators & single-ended Class-A asymmetry
"The threshold control operating as a pair of opposed attenuators, one preceding the input amplifier and the other controlling the amount of feedback over the output amplifier" (BBC Ops Manual). Gain stays unity, but the amplifiers work relative to the turnover: low thresholds drive the single-ended `VT3–VT5` post-chopper amplifier harder (generating dominant Class-A H2, then asymmetric rail clipping ~21 dB over the turnover, AC-coupled through `C16a & b`). The H2 term has the reference unit's shape, `a·s²/(1 + c|s|)`, and fades out under compression as `g⁵/(g⁵ + k)`: gone below ~12 dB of attenuation, so it cannot come from ahead of the chopper, while input-referred hiss rises with the threshold setting and breathes with the gain reduction. Dial 0 puts the turnover at −20 dBFS sine peak; **Limit** moves it 8 dB up ("so that limiting takes place correctly at the 100 % modulation values").

### 5. Time constants
- **Attack** (fixed, as on the unit): C23 charges through the VT13/VT14 junction, so the charging current rises expansively with the overdrive and is finally current-limited. Compression reaches 63 % in < 0.5 ms and limiting in ≈ 0.9 ms (the limit resistor set passes less current), per the BBC spec sheet ("Compression less than 0.5 m.secs. Limit 1 m.sec. ±0.5"). Fast transients overshoot before the loop catches them, as in Fig. 2.5.
- **Ripple top-ups and the static curve**: the small charges that refill C23 on every waveform peak see the junction's small-signal constant (5 ms). How far C23 sags between peaks against the decay resistor sets the static curve at each decay position; this one constant matches the reference at every ratio and decay to ≤ 0.17 dB, on top of its measured per-decay side-chain trims.
- **Decay**: the panel's 1–32 ×100 ms are nominal recovery times. C23 returns to its rest bias with the reference unit's constants: 58.5, 106, 171, 360, 834 and 1854 ms.

### 6. Filters and transformers
The LC anti-carrier filter (L1/L2 52.5 mH, C5–C7 3.3 nF) together with T1/T2 is a fitted 5-pole response plus the T2 leakage resonance: +0.40 dB at 10 kHz, −0.25 dB at 15 kHz, −6.7 dB at 18 kHz, −12.5 dB at 20 kHz, ~30 dB/octave beyond, and an 11.5 Hz two-pole low end — inside the manual's +0.5/−1 dB (30 Hz–15 kHz). The high-Q sections use magnitude-matched designs (Vicanek) so the response holds at every sample rate. The side chain is tapped after the LC filter, ahead of the T2 resonance and the post-chopper amplifier, so neither the resonance nor the amplifier's distortion acts on the gain. The side-chain transformer `T4 (772201)` coupled through `C14a & b` is modelled as an underdamped 2-pole high-pass (`16.5 Hz, Q = 0.92`), preserving full low-frequency sidechain sensitivity at 50–100 Hz. The output stage (VT6–VT9 into T2) adds low-frequency core flux and push-pull rounding at high output levels.

### 7. Noise gate (845751/01 gate board)
Senses the programme after the input amplifier through the `C101/C102` high-pass and the `L101 (52.5 mH) / C106 (3.3 nF)` 2-pole LC low-pass filter (`f0 = 12.09 kHz`), rectifies into `C108 (200 µF)` via `D101/D102 (OA47)`, and drives the `TR104/TR105` regenerative Schmitt trigger with 2 dB hysteresis. Below threshold for more than 250 ms it fades in the selected 2–10 dB `SWC` FET pad (`TR107 2N3819`) with a 250 ms time constant; it opens again in under 1 ms (manual ch. 1).

### 8. Routing and LINK
**Dual Mono**, **Stereo** (one detector, channel A's controls drive both) and **M/S**. **LINK** behaves like the dual frame's switch, which joins PL1 pins 6 and 10 of both units: the two C23 control voltages are paralleled, so both channels share one gain while keeping their own knobs.

### 9. Analog
Off (digital silence in, silence out), or 50/60 Hz: rectifier ripple and 4-harmonic transformer hum (computed via a single `sin_cos` and Chebyshev recurrence) plus transistor flicker/hiss.

### Signal flow (per channel, at 176.4–192 kHz)
```
in ─▶ T1 ─▶ VT1 input amp ─┬─▶ chopper VT2 ─▶ LC filter ─┬─▶ T2 res. ─▶ VT3–VT5 amp ─▶ C16 ─▶ output atten. ─▶ VT6–VT9 / T2 ─▶ out
   (threshold attenuator)  │        ▲  mark:space        │                                      (+ gate pad)
                           │        └── C23 / blocking osc ◀── ratio amp ◀── OA47 bridge ◀── T4 (+ SC HPF)
                           └─▶ noise gate (C101 HPF ─▶ L101/C106 12.09 kHz LC ─▶ OA47 / C108 ─▶ TR107 FET)
```
Oversampling is 4× at 44.1/48 kHz and 2× at 88.2/96 kHz with polyphase elliptic half-band filters (≈100 dB alias rejection, flat to 20.2 kHz). No latency is reported or needed. CPU: **~1.5–2 % of one core** for a stereo instance at 48 kHz on Apple Silicon (offline pedalboard harness, 512-sample buffers; on par with v0.3.0 in the same harness).

---

## 🔬 Measured against the reference

`tools/compare_waves.py` and `tools/test_full.py` drive this plugin and the Waves Kramer PIE with identical stimuli. Representative results (48 kHz, threshold 0):

| Measurement | Pye4060 | Waves PIE | Notes |
|:---|:---:|:---:|:---|
| Frequency response, 30 Hz – 20 kHz | 15k −0.25, 18k −6.7, 20k −12.5 dB | 15k −0.30, 18k −6.7, 20k −12.4 dB | within ±0.25 dB of the reference throughout |
| GR, 2:1, −12 / −6 / 0 / +3 dBFS in | 2.7 / 5.9 / 9.2 / 10.8 dB | 2.7 / 5.9 / 9.1 / 10.8 dB | knee emerges from the linear-with-bias ratio amplifier |
| GR, 3:1, −12 / −6 / 0 / +6 dBFS in | 5.4 / 9.4 / 13.2 / 16.9 dB | 5.4 / 9.4 / 13.2 / 16.9 dB | 5:1 and Limit within 0.1 dB across full range |
| GR, 3:1, −6 dBFS, decay 100 ms / 400 ms / 3.2 s | 9.33 / 9.39 / 10.55 dB | 9.19 / 9.36 / 10.57 dB | all ratios × decays within 0.17 dB |
| GR at −10 dBFS, threshold −16 / −8 / 0 / +8 | 17.0 / 12.0 / 6.8 / 1.3 dB | 16.9 / 11.9 / 6.8 / 1.3 dB | linear opposed-attenuator threshold mapping |
| Recovery, 3:1, 400 ms position, 100/200/400 ms | 11.1 / 7.5 / 2.9 dB | 11.0 / 7.4 / 2.8 dB | C23 returns to its rest bias |
| GR & H3 at 50 Hz, 3:1, 100 ms / 400 ms decay | 16.1 dB (−31 dBc) / 16.2 dB (−39 dBc) | 15.8 dB (−34 dBc) / 16.0 dB (−42 dBc) | 2-pole resonant `T4 772201` sidechain transformer |
| H2, 3:1, 3.2 s, 1 kHz, at 0 / 5 / 13 / 17 dB GR | −63 / −53 / −56 / −65 dBc | −63 / −53 / −57 / −69 dBc | even-order term fades out under compression |
| H3, 3:1, 16.8 dB GR, 3.2 s decay, 1 kHz | −84 dBc | −84 dBc | smooth C23 voltage at long decay |
| Attack, 63 %, −30 → −2 dBFS at 4 kHz; BBC: < 0.5 ms (comp.) / 1 ± 0.5 ms (limit) | 0.3 / 0.9 ms | 1.3 / 0.3 ms | Waves inverts compression and limit |
| Aliasing rejection (7.5 kHz @ +4 dBFS, 3:1, 100 ms) | **H4 alias: −131.5 dBc, H5: −87.0 dBc** | H4 alias: −99.8 dBc, H5: −72.6 dBc | **14 to 32 dB cleaner** (4× OS vs. 1×) |
| Aliasing rejection (7.5 kHz @ −6 dBFS, 1:1 drive) | **H4 alias: −65.5 dBc, H6: −65.1 dBc** | H4 alias: −23.6 dBc, H6: −32.3 dBc | **33 to 42 dB cleaner** in saturation |

### Tests
- `cargo test --release --lib`: 15 unit tests — static ratios, +8 dB limit turnover, BBC attack spec (compression faster than limit), recovery to unity, maximum attenuation, filter accuracy, oversampler flatness and alias rejection, unity gain below threshold, digital silence, M/S isolation, LINK behaviour, noise-gate timing.
- `tools/test_full.py`: 8 end-to-end verification checks on the built VST3 bundle (`pye4060/target/bundles`) (bypass, frequency response, all 5 static compression curves, +8 dB limit offset, 6 decay positions, Class-A H2 dominance, Dual Mono & M/S isolation, 845751/01 noise gate).
- `cargo truce validate --pluginval`: VST3 passes strictness validation.
- `auval -v aufx Pye4 JSon`: AU v2 passes Apple's validation.

---

## 📦 Installation & Quickstart

### Download (macOS, Apple Silicon)
Grab `Pye4060-v0.4.0-macOS-arm64.zip` from [Releases](https://github.com/nagualcode/pye4060/releases). It contains the AU v2 (`.component`), VST3 and CLAP bundles; install the format(s) your DAW uses:
```bash
unzip Pye4060-v0.4.0-macOS-arm64.zip
cd Pye4060-v0.4.0-macOS-arm64
cp -R Pye4060.component ~/Library/Audio/Plug-Ins/Components/   # Logic, GarageBand, Ableton Live...
cp -R Pye4060.vst3 ~/Library/Audio/Plug-Ins/VST3/              # optional
cp -R Pye4060.clap ~/Library/Audio/Plug-Ins/CLAP/              # optional
# The bundles are ad-hoc signed, not notarised: clear the download quarantine.
xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/Components/Pye4060.component \
  ~/Library/Audio/Plug-Ins/VST3/Pye4060.vst3 ~/Library/Audio/Plug-Ins/CLAP/Pye4060.clap 2>/dev/null
```
If Logic does not list the plugin right away, run `killall -9 AudioComponentRegistrar` and rescan.
Intel Macs and other platforms: build from source.

### Prerequisites
- [Rust & Cargo](https://rustup.rs/) (1.92+)
- [Truce CLI](https://github.com/truce-audio/truce) (`cargo install cargo-truce`)

### Build and install
```bash
git clone https://github.com/nagualcode/pye4060.git
cd pye4060/pye4060
cargo truce build          # AU v2 + VST3 + CLAP (+ standalone) into target/bundles
cargo truce install --au2  # ~/Library/Audio/Plug-Ins/Components (or omit the flag for every format)
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
- [x] **AU v2** packaging (v0.4.0).
- [ ] **AU v3**, Developer ID signing and notarisation.
- [ ] **Measure a real 4060**: the decay constants, H2 balance and 2:1 knee are calibrated against the Waves model; impulse, stepped-sine and recovery captures of a physical unit would settle them.
- [ ] **CLAP validation** with clap-validator in CI.

## 📄 License

This project is licensed under the [MIT License](LICENSE).
Audio DSP framework powered by [Truce Audio](https://github.com/truce-audio/truce).
Original Pye TVT schematics and BBC historical documentation referenced for educational and forensic circuit analysis purposes.
