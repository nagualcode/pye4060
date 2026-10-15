#!/usr/bin/env python3
"""Generates factory presets for Jardim Sonoro Pye 4060.

Includes:
1. Authentic legacy presets extracted from Waves Kramer PIE (Eddie Kramer Olympic sessions).
2. Pye TVT BBC broadcast transmission presets.
3. Modern Mid/Side (M/S) mastering and Dual Mono tracking presets.
"""

import os
import xml.etree.ElementTree as ET

PRESET_DIR = "/Users/nagual/code/pyecompressor/pye4060/presets"
WAVES_XML = "/Applications/Waves/Plug-Ins V17/KramerPIE.bundle/Contents/Resources/Presets/1000.xml"

category_map = {
    "Lead Vocal Comp": ("vocal", "Fast attack transparent vocal leveling from classic Olympic sessions", ["vocal", "lead", "olympic"]),
    "Kick Drum Comp": ("drums", "Tight, punchy kick drum transient control", ["drums", "kick", "punch"]),
    "Snare Drum Norm Comp": ("drums", "Classic rock snare drum compression with fast snap", ["drums", "snare", "rock"]),
    "Snare Drum SuperHype Comp": ("drums", "Explosive brickwall limiter snare smash", ["drums", "snare", "crush", "limit"]),
    "Toms Norm Comp": ("drums", "Controlled tom sustain and stick impact", ["drums", "toms", "transient"]),
    "Toms Super Smash": ("drums", "Heavy tom pump and aggressive room sustain", ["drums", "toms", "smash"]),
    "Medium Room Comp": ("room", "Ambient drum room enhancement with natural decay", ["room", "drums", "reverb"]),
    "Norm Close Room Comp": ("room", "Tight, controlled close room ambiance", ["room", "ambiance"]),
    "Norm Room Comp": ("room", "Balanced room compression for cohesive space", ["room", "stereo"]),
    "Super Smash Close Rooms": ("room", "Massive explosive close room energy (Bonham/Kramer style)", ["room", "crush", "zeppelin"]),
    "Super Smash Room": ("room", "All-out room compression and brickwall distortion", ["room", "smash", "limit"]),
    "AGTR Norm Comp": ("acoustic", "Smooth acoustic guitar strumming leveling", ["acoustic", "guitar", "smooth"]),
    "AGTR SMASH Comp": ("acoustic", "Slow release acoustic guitar sustain boost", ["acoustic", "guitar", "sustain"]),
    "Bass Comp": ("bass", "Warm, solid electric bass leveling with punchy note definition", ["bass", "warmth", "punch"]),
    "Bass Comp 2": ("bass", "Limiter mode bass control for high-energy rock grooves", ["bass", "rock", "limit"]),
    "boxes": ("drums", "Parallel crushing preset for drum bus blending", ["drums", "parallel", "crush"]),
}


def sanitize_filename(name):
    return name.lower().replace(" ", "-").replace("/", "-") + ".preset"


def extract_waves_presets():
    tree = ET.parse(WAVES_XML)
    root = tree.getroot()
    extracted = []

    for p in root.findall("Preset"):
        name = p.get("Name")
        if name in ["Full Reset", "PA Default"]:
            continue
        data = p.find("PresetData/Parameters")
        if data is None or not data.text:
            continue
        tokens = data.text.split()
        ratio_idx = int(float(tokens[0]))
        decay_idx = int(float(tokens[1]))
        analog_val = int(float(tokens[14]))
        out_gain = float(tokens[27])
        thresh = float(tokens[33])

        # Analog map: 0: 50Hz, 1: 60Hz, 2: Off -> In our plugin: 0: Off, 1: 50Hz, 2: 60Hz
        our_analog = 2 if analog_val == 1 else (1 if analog_val == 0 else 0)

        cat, comment, tags = category_map.get(
            name, ("general", f"Classic Kramer PIE setting: {name}", ["classic", "pye"])
        )

        preset_data = {
            "name": name,
            "category": cat,
            "author": "Eddie Kramer / Olympic Studios",
            "comment": comment,
            "tags": tags,
            "mode": 1,  # Stereo
            "link": 1,
            "analog": our_analog,
            "bypass": 0,
            "thresh_a": thresh,
            "ratio_a": ratio_idx,
            "decay_a": decay_idx,
            "gate_thresh_a": -30.0,
            "gate_atten_a": 0,  # 0 dB (Off)
            "out_a": out_gain,
            "thresh_b": thresh,
            "ratio_b": ratio_idx,
            "decay_b": decay_idx,
            "gate_thresh_b": -30.0,
            "gate_atten_b": 0,
            "out_b": out_gain,
        }
        extracted.append(preset_data)
    return extracted


def get_additional_presets():
    return [
        {
            "name": "Default 3-1",
            "category": "init",
            "author": "Jardim Sonoro",
            "comment": "Default Pye TVT 84-4060 3:1 compression state",
            "tags": ["default", "init"],
            "mode": 1,
            "link": 1,
            "analog": 0,
            "bypass": 0,
            "thresh_a": 16.0,
            "ratio_a": 2,  # 3:1
            "decay_a": 2,  # 400 ms
            "gate_thresh_a": -30.0,
            "gate_atten_a": 0,
            "out_a": 0.0,
            "thresh_b": 16.0,
            "ratio_b": 2,
            "decay_b": 2,
            "gate_thresh_b": -30.0,
            "gate_atten_b": 0,
            "out_b": 0.0,
        },
        {
            "name": "BBC Broadcast Limiter",
            "category": "broadcast",
            "author": "Pye TVT Limited",
            "comment": "100% modulation brickwall limiter with British 50Hz hum and 4dB Noise Gate",
            "tags": ["broadcast", "bbc", "limit", "gate"],
            "mode": 1,
            "link": 1,
            "analog": 1,  # 50 Hz
            "bypass": 0,
            "thresh_a": 0.0,
            "ratio_a": 4,  # Limit
            "decay_a": 0,  # 100 ms
            "gate_thresh_a": -35.0,
            "gate_atten_a": 2,  # 4 dB
            "out_a": 0.0,
            "thresh_b": 0.0,
            "ratio_b": 4,
            "decay_b": 0,
            "gate_thresh_b": -35.0,
            "gate_atten_b": 2,
            "out_b": 0.0,
        },
        {
            "name": "Olympic Master Bus Glue (M-S)",
            "category": "mastering",
            "author": "Jardim Sonoro",
            "comment": "Subtle 2:1 Mid/Side cohesion with independent timing on Mid and Side channels",
            "tags": ["mastering", "glue", "m-s", "bus"],
            "mode": 2,  # M/S
            "link": 1,
            "analog": 0,  # Off
            "bypass": 0,
            "thresh_a": -4.0,
            "ratio_a": 1,  # 2:1
            "decay_a": 2,  # 400 ms
            "gate_thresh_a": -40.0,
            "gate_atten_a": 0,
            "out_a": 1.5,
            "thresh_b": 0.0,
            "ratio_b": 1,  # 2:1
            "decay_b": 3,  # 800 ms
            "gate_thresh_b": -40.0,
            "gate_atten_b": 0,
            "out_b": 1.0,
        },
        {
            "name": "Hendrix Drum Smash (M-S)",
            "category": "drums",
            "author": "Eddie Kramer Style",
            "comment": "Aggressive brickwall limiting on center mono punch (Kick/Snare) with dynamic stereo room width",
            "tags": ["drums", "hendrix", "m-s", "crush"],
            "mode": 2,  # M/S
            "link": 0,
            "analog": 2,  # 60 Hz
            "bypass": 0,
            "thresh_a": -10.0,
            "ratio_a": 4,  # Limit
            "decay_a": 0,  # 100 ms
            "gate_thresh_a": -30.0,
            "gate_atten_a": 0,
            "out_a": 8.0,
            "thresh_b": -4.0,
            "ratio_b": 2,  # 3:1
            "decay_b": 1,  # 200 ms
            "gate_thresh_b": -30.0,
            "gate_atten_b": 0,
            "out_b": 4.0,
        },
        {
            "name": "Dual Mono Tracking (Vocal & Bass)",
            "category": "tracking",
            "author": "Jardim Sonoro",
            "comment": "Channel A dialed for vocal presence (3:1, 200ms) and Channel B for bass foundation (3:1, 400ms)",
            "tags": ["tracking", "dual-mono", "vocal", "bass"],
            "mode": 0,  # Dual Mono
            "link": 0,
            "analog": 0,
            "bypass": 0,
            "thresh_a": 4.0,
            "ratio_a": 2,  # 3:1
            "decay_a": 1,  # 200 ms
            "gate_thresh_a": -30.0,
            "gate_atten_a": 2,  # 4 dB
            "out_a": 2.5,
            "thresh_b": 6.0,
            "ratio_b": 2,  # 3:1
            "decay_b": 2,  # 400 ms
            "gate_thresh_b": -30.0,
            "gate_atten_b": 0,
            "out_b": 1.0,
        },
    ]


def write_preset(p):
    cat_dir = os.path.join(PRESET_DIR, p["category"])
    os.makedirs(cat_dir, exist_ok=True)
    filename = sanitize_filename(p["name"])
    path = os.path.join(cat_dir, filename)

    tags_str = ", ".join(f'"{t}"' for t in p["tags"])
    content = f'''name = "{p["name"]}"
author = "{p["author"]}"
comment = "{p["comment"]}"
tags = [{tags_str}]

[params]
mode = {p["mode"]}
link = {p["link"]}
analog = {p["analog"]}
bypass = {p["bypass"]}

threshold_a = {p["thresh_a"]:.1f}
ratio_a = {p["ratio_a"]}
decay_a = {p["decay_a"]}
gate_thresh_a = {p["gate_thresh_a"]:.1f}
gate_atten_a = {p["gate_atten_a"]}
output_a = {p["out_a"]:.1f}

threshold_b = {p["thresh_b"]:.1f}
ratio_b = {p["ratio_b"]}
decay_b = {p["decay_b"]}
gate_thresh_b = {p["gate_thresh_b"]:.1f}
gate_atten_b = {p["gate_atten_b"]}
output_b = {p["out_b"]:.1f}
'''
    with open(path, "w") as f:
        f.write(content)
    print(f"Wrote {path}")


if __name__ == "__main__":
    presets = extract_waves_presets() + get_additional_presets()
    for p in presets:
        write_preset(p)
    print(f"\nGenerated {len(presets)} total presets successfully.")
