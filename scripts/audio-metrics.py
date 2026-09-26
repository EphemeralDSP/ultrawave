#!/usr/bin/env python3
"""Dependency-free checks for deterministic audition PCM WAVs."""

import json, math, pathlib, struct, sys, wave

root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "target/audition")
names = ["recorded-12bit", "play-12bit", "pitch-down", "brr", "srr", "driven"]


def read(name):
    path = root / (name + ".wav")
    try:
        with wave.open(str(path), "rb") as w:
            assert w.getcomptype() == "NONE" and w.getsampwidth() in (2, 3, 4), (
                "PCM WAV required"
            )
            meta = (
                w.getnchannels(),
                w.getframerate(),
                w.getnframes(),
                w.getsampwidth(),
            )
            raw = w.readframes(w.getnframes())
            floating = False
    except wave.Error:
        blob = path.read_bytes()
        assert blob[:4] == b"RIFF" and blob[8:12] == b"WAVE", "WAV required"
        pos = 12
        fmt = raw = None
        while pos + 8 <= len(blob):
            tag, size = blob[pos : pos + 4], struct.unpack_from("<I", blob, pos + 4)[0]
            chunk = blob[pos + 8 : pos + 8 + size]
            pos += 8 + size + (size & 1)
            if tag == b"fmt ":
                fmt = chunk
            elif tag == b"data":
                raw = chunk
        code, channels, rate, _, _, bits = struct.unpack_from("<HHIIHH", fmt)
        floating = code == 3 or (code == 0xFFFE and fmt[24:26] == b"\x03\x00")
        assert floating and bits == 32 and raw is not None, (
            "only float32 extended WAV supported"
        )
        meta = (channels, rate, len(raw) // (channels * 4), 4)
    width = meta[3]
    if floating:
        vals = list(struct.unpack("<" + "f" * (len(raw) // 4), raw))
    elif width == 3:
        vals = [
            int.from_bytes(raw[i : i + 3], "little", signed=True) / 8388608
            for i in range(0, len(raw), 3)
        ]
    else:
        code = "h" if width == 2 else "i"
        vals = [
            v / float(1 << (8 * width - 1))
            for v in struct.unpack("<" + code * (len(raw) // width), raw)
        ]
    return meta, vals


result, data, errors = {}, {}, []
for name in names:
    try:
        meta, samples = read(name)
    except Exception as e:
        errors.append(f"{name}: {e}")
        continue
    data[name] = (meta, samples)
    peak = max(map(abs, samples), default=0.0)
    rms = math.sqrt(sum(x * x for x in samples) / max(1, len(samples)))
    result[name] = {
        "frames": meta[2],
        "rate": meta[1],
        "channels": meta[0],
        "peak": round(peak, 6),
        "rms": round(rms, 6),
    }
    if not samples or not math.isfinite(peak + rms) or rms < 1e-6:
        errors.append(f"{name}: empty/silent/non-finite")
    clipped = sum(abs(x) >= 0.999999 for x in samples)
    # Float playback has headroom: removing DC can legitimately overshoot unity.
    # Only the integer captured WAV has a hard full-scale rail. The normalized
    # high-pass's impulse-response absolute sum is below two, so a unit-bounded
    # raw voice must still stay inside +/-2 without any output clipping stage.
    result[name]["over_unity_samples"] = sum(abs(x) > 1.0 for x in samples)
    if name == "recorded-12bit" and clipped > max(4, len(samples) // 1000):
        errors.append(f"{name}: clipping ({clipped} samples)")
    if peak > 2.0:
        errors.append(f"{name}: exceeded wet filter headroom bound ({peak})")
if len(data) == len(names):
    base_meta, base = data["recorded-12bit"]
    neutral_meta, neutral = data["play-12bit"]
    if base_meta[:2] != neutral_meta[:2] or len(neutral) < len(base):
        errors.append("neutral capture/play format mismatch or truncated output")
    else:
        # Independent double-precision weighted-mean reference for the fixed
        # 17Hz wet low cut, including its idle tail. Do not compare filtered
        # playback directly to the unfiltered RAM or hide a placement error.
        pole = math.exp(-2 * math.pi * 17 / base_meta[1])
        mean, expected = 0.0, []
        for i in range(len(neutral)):
            sample = base[i] if i < len(base) else 0.0
            expected.append((1 + pole) * 0.5 * (sample - mean))
            mean = (1 - pole) * sample + pole * mean
        delta = math.sqrt(
            sum((a - b) ** 2 for a, b in zip(expected, neutral)) / len(neutral)
        )
        result["neutral_delta_rms"] = round(delta, 8)
        if delta > 2e-6:
            errors.append(f"neutral capture/play mismatch ({delta:.8f})")
    for name in names[2:]:
        variant = data[name][1]
        n = min(len(base), len(variant))
        delta = math.sqrt(
            sum((base[i] - variant[i]) ** 2 for i in range(n)) / max(1, n)
        )
        result[name]["delta_rms"] = round(delta, 6)
        if delta < 0.001:
            errors.append(f"{name}: not meaningfully distinct ({delta:.6f})")
result["ok"] = not errors
result["errors"] = errors
print(json.dumps(result, sort_keys=True, separators=(",", ":")))
sys.exit(0 if not errors else 1)
