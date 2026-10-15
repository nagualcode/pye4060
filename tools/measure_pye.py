#!/usr/bin/env python3
"""Empirical measurement suite for Pye 4060 compressor.

Measures:
1. Linear frequency response (transformer / discrete stage filtering).
2. Static compression curve across ratios (1:1, 2:1, 3:1, 5:1, Lim) and thresholds.
3. Attack and decay ballistics across all 6 decay settings.
4. Harmonic distortion spectrum (H1-H5) vs level.
"""

import sys
import numpy as np
import pedalboard

WAVES_SHELL = "/Library/Audio/Plug-Ins/VST3/WaveShell1-VST3 17.1.vst3"
PLUGIN_NAME = "Kramer PIE Stereo"
SR = 48000


def load_kramer_pie():
    return pedalboard.load_plugin(WAVES_SHELL, plugin_name=PLUGIN_NAME)


def set_params(p, threshold=16.0, ratio="1:1 ", decay="4 x100ms", output=0.0, analog="Off "):
    p.threshold = threshold
    p.ratio = ratio
    p.decay = decay
    p.output = output
    p.analog = analog


def measure_freq_response(p, freqs):
    print("--- Measuring Frequency Response (Linear, below threshold) ---")
    set_params(p, threshold=16.0, ratio="1:1 ", output=0.0, analog="Off ")
    gains = []
    amp = 10 ** (-30.0 / 20.0)  # -30 dBFS (well below threshold)
    duration = 0.25
    n = int(duration * SR)
    t = np.arange(n) / SR

    for f in freqs:
        x = (amp * np.sin(2 * np.pi * f * t)).astype(np.float32)
        x2 = np.stack([x, x])
        y = p.process(x2, SR, reset=True)[0]
        y_ss = y[n // 2:]
        rms_in = np.sqrt(np.mean(x[n // 2:] ** 2))
        rms_out = np.sqrt(np.mean(y_ss ** 2))
        gain_db = 20 * np.log10(max(rms_out, 1e-9) / rms_in)
        gains.append(gain_db)
        print(f"  {f:6.0f} Hz: {gain_db:+.2f} dB")
    return dict(zip(freqs, gains))


def measure_compression_curves(p):
    print("\n--- Measuring Compression Curves (Input Sweep -40 dBFS to 0 dBFS) ---")
    # Set threshold to 0.0 dB
    set_params(p, threshold=0.0, decay="4 x100ms", output=0.0, analog="Off ")
    in_levels = np.arange(-40, 4, 2)  # -40 dBFS to +2 dBFS
    duration = 0.4
    n = int(duration * SR)
    t = np.arange(n) / SR
    f = 1000.0
    results = {}

    ratios = ['1:1 ', '2:1 ', '3:1 ', '5:1 ', 'Limit ']
    for r in ratios:
        p.ratio = r
        out_levels = []
        for l_in in in_levels:
            amp = 10 ** (l_in / 20.0)
            x = (amp * np.sin(2 * np.pi * f * t)).astype(np.float32)
            x2 = np.stack([x, x])
            y = p.process(x2, SR, reset=True)[0]
            # Take last 100ms for steady state
            y_ss = y[-int(0.1 * SR):]
            rms_out = np.sqrt(np.mean(y_ss ** 2))
            l_out = 20 * np.log10(max(rms_out * np.sqrt(2), 1e-9))
            out_levels.append(l_out)
        results[r] = out_levels
        print(f"Ratio '{r}':")
        print("  In (dBFS): ", [f"{x:+3.0f}" for x in in_levels])
        print("  Out(dBFS): ", [f"{y:+5.1f}" for y in out_levels])
        gr = in_levels[-1] - out_levels[-1]
        print(f"  --> Max GR at +2 dBFS: {gr:.2f} dB")
    return results


def measure_threshold_offset(p):
    print("\n--- Measuring Threshold Turnover for Ratio 3:1 vs Limit ---")
    # Sweep threshold at fixed input level -10 dBFS
    in_level = -10.0
    amp = 10 ** (in_level / 20.0)
    duration = 0.3
    n = int(duration * SR)
    t = np.arange(n) / SR
    f = 1000.0
    x = (amp * np.sin(2 * np.pi * f * t)).astype(np.float32)
    x2 = np.stack([x, x])

    print("Checking gain reduction as threshold is lowered from +16 to -24 dB:")
    thresh_list = [16.0, 10.0, 4.0, 0.0, -4.0, -8.0, -12.0, -16.0, -20.0, -24.0]
    for mode in ["3:1 ", "Limit "]:
        p.ratio = mode
        gr_list = []
        for th in thresh_list:
            p.threshold = th
            y = p.process(x2, SR, reset=True)[0]
            y_ss = y[-int(0.1 * SR):]
            rms_out = np.sqrt(np.mean(y_ss ** 2))
            l_out = 20 * np.log10(max(rms_out * np.sqrt(2), 1e-9))
            gr = in_level - l_out
            gr_list.append(gr)
        print(f"  Mode {mode}:")
        print("    Threshold:", thresh_list)
        print("    GR (dB):  ", [f"{g:5.2f}" for g in gr_list])


def measure_decay_times(p):
    print("\n--- Measuring Decay Ballistics (Step down from 0 dBFS to -30 dBFS) ---")
    set_params(p, threshold=-10.0, ratio="3:1 ", output=0.0, analog="Off ")
    f = 1000.0
    t_burst = 0.5
    t_tail = 3.5
    n_burst = int(t_burst * SR)
    n_tail = int(t_tail * SR)
    t1 = np.arange(n_burst) / SR
    t2 = np.arange(n_tail) / SR
    amp_burst = 1.0
    amp_tail = 10 ** (-30.0 / 20.0)
    x = np.concatenate([
        amp_burst * np.sin(2 * np.pi * f * t1),
        amp_tail * np.sin(2 * np.pi * f * t2)
    ]).astype(np.float32)
    x2 = np.stack([x, x])

    decays = ['1 x100ms', '2 x100ms', '4 x100ms', '8 x100ms', '16 x100ms', '32 x100ms']
    for d in decays:
        p.decay = d
        y = p.process(x2, SR, reset=True)[0]
        tail_y = y[n_burst:]
        win = int(0.01 * SR)
        n_blocks = len(tail_y) // win
        env = []
        for b in range(n_blocks):
            blk = tail_y[b * win:(b + 1) * win]
            rms = np.sqrt(np.mean(blk ** 2))
            gain_rel = rms / (amp_tail / np.sqrt(2))
            gain_db = 20 * np.log10(max(gain_rel, 1e-6))
            env.append(gain_db)
        
        # find time when gain reaches -1.0 dB
        t_ms = [i * 10 for i, g in enumerate(env) if g >= -1.0]
        rec_time = t_ms[0] if len(t_ms) > 0 else ">3500"
        print(f"  Decay '{d}': Initial GR in tail: {env[0]:.1f} dB, recovers to -1dB in ~{rec_time} ms")


def measure_harmonics(p):
    print("\n--- Measuring Harmonics vs Level at 1 kHz (Ratio 3:1, Decay 400ms) ---")
    set_params(p, threshold=0.0, ratio="3:1 ", decay="4 x100ms", output=0.0, analog="Off ")
    f = 1000.0
    duration = 0.5
    n = int(duration * SR)
    t = np.arange(n) / SR

    levels = [-24, -18, -12, -6, 0, +3]
    for lev in levels:
        amp = 10 ** (lev / 20.0)
        x = (amp * np.sin(2 * np.pi * f * t)).astype(np.float32)
        x2 = np.stack([x, x])
        y = p.process(x2, SR, reset=True)[0]
        y_ss = y[-int(0.2 * SR):]
        t_fit = np.arange(len(y_ss)) / SR
        cols = []
        for h in range(1, 6):
            cols.append(np.sin(2 * np.pi * h * f * t_fit))
            cols.append(np.cos(2 * np.pi * h * f * t_fit))
        A = np.stack(cols, axis=1)
        coef, *_ = np.linalg.lstsq(A, y_ss, rcond=None)
        h_amps = [np.hypot(coef[2 * i], coef[2 * i + 1]) for i in range(5)]
        h1 = h_amps[0]
        h2 = 100 * h_amps[1] / max(h1, 1e-9)
        h3 = 100 * h_amps[2] / max(h1, 1e-9)
        h4 = 100 * h_amps[3] / max(h1, 1e-9)
        h5 = 100 * h_amps[4] / max(h1, 1e-9)
        thd = 100 * np.sqrt(sum(a**2 for a in h_amps[1:])) / max(h1, 1e-9)
        print(f"  Level {lev:+3.0f} dBFS: Fund={20*np.log10(h1*np.sqrt(2)):+.1f} dBFS | THD={thd:5.2f}% (H2={h2:5.2f}%, H3={h3:5.2f}%, H4={h4:5.2f}%, H5={h5:5.2f}%)")


if __name__ == "__main__":
    p = load_kramer_pie()
    freqs = [20, 50, 100, 500, 1000, 5000, 10000, 15000, 18000, 20000]
    measure_freq_response(p, freqs)
    measure_compression_curves(p)
    measure_threshold_offset(p)
    measure_decay_times(p)
    measure_harmonics(p)
