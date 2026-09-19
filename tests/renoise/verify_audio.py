#!/usr/bin/env python3
"""Verify Renoise renders, including correct PCM-vs-float WAV decoding."""

import json
import math
import pathlib
import struct
import sys


def read_wav(path):
    data = path.read_bytes()
    assert data[:4] == b"RIFF" and data[8:12] == b"WAVE", path
    chunks, offset = {}, 12
    while offset + 8 <= len(data):
        name, size = (
            data[offset : offset + 4],
            struct.unpack_from("<I", data, offset + 4)[0],
        )
        chunks[name] = data[offset + 8 : offset + 8 + size]
        offset += 8 + size + (size & 1)
    fmt, raw = chunks[b"fmt "], chunks[b"data"]
    audio_format, channels, rate = struct.unpack_from("<HHI", fmt)
    bits = struct.unpack_from("<H", fmt, 14)[0]
    if (
        audio_format == 0xFFFE
    ):  # WAVE_FORMAT_EXTENSIBLE: first GUID word is the real format.
        audio_format = struct.unpack_from("<H", fmt, 24)[0]
    width = bits // 8
    if audio_format == 3 and bits == 32:
        values = struct.unpack("<%df" % (len(raw) // 4), raw)
        encoding = "float32"
    elif audio_format == 1 and bits in (16, 24, 32):
        ints = [
            int.from_bytes(raw[i : i + width], "little", signed=True)
            for i in range(0, len(raw), width)
        ]
        values = [v / float(1 << (bits - 1)) for v in ints]
        encoding = "int%d" % bits
    else:
        raise AssertionError("unsupported WAV format=%d bits=%d" % (audio_format, bits))
    return channels, rate, values, encoding


def metrics(path):
    channels, rate, values, encoding = read_wav(path)
    left, right = values[0::channels], values[1::channels]
    active = [i for i, value in enumerate(left) if abs(value) > 1e-5]
    crossings = [i for i in range(1, len(left)) if left[i - 1] <= 0 < left[i]]
    frequency = None
    if len(crossings) > 2:
        frequency = rate * (len(crossings) - 1) / (crossings[-1] - crossings[0])
    return {
        "channels": channels,
        "sample_rate": rate,
        "frames": len(values) // channels,
        "encoding": encoding,
        "peak": max(map(abs, values)),
        "rms": math.sqrt(sum(v * v for v in values) / len(values)),
        "stereo_delta_peak": max(abs(a - b) for a, b in zip(left, right)),
        "active_extent_frames": (active[-1] + 1) if active else 0,
        "estimated_frequency_hz": frequency,
    }


out = pathlib.Path(sys.argv[1])
capture = metrics(out / "capture-pass.wav")
r1, r2 = metrics(out / "play-r1.wav"), metrics(out / "play-r2.wav")
assert capture["peak"] < 1e-7, capture  # monitor defaults are zero
assert r1["channels"] == 2 and r1["peak"] > 1e-4 and r1["rms"] > 1e-5, r1
assert r1["stereo_delta_peak"] < 1e-7, r1
assert 435 < r1["estimated_frequency_hz"] < 445, r1
assert 90000 < r1["active_extent_frames"] < 100000, r1
assert r2["peak"] < 1e-7, r2
second_input = metrics(out / "second-capture.wav")
assert 875 < second_input["estimated_frequency_hz"] < 885, second_input
protected = metrics(out / "play-one-shot.wav")
assert read_wav(out / "play-r1.wav") == read_wav(out / "play-one-shot.wav"), protected
settings = metrics(out / "play-settings.wav")
assert math.isclose(settings["rms"] / r1["rms"], 10 ** (-3 / 20), rel_tol=0.001), settings
reopened = metrics(out / "play-after-reopen.wav")
assert reopened["peak"] < 1e-7, reopened  # RAM must not survive a full shutdown
recaptured = metrics(out / "play-recaptured.wav")
assert recaptured["peak"] > 1e-4 and recaptured["stereo_delta_peak"] < 1e-7, recaptured
assert 435 < recaptured["estimated_frequency_hz"] < 445, recaptured
assert 90000 < recaptured["active_extent_frames"] < 100000, recaptured
assert metrics(out / "recapture-after-reopen.wav")["peak"] < 1e-7
assert "project_controls_restored=true" in (out / "host-result.txt").read_text()
summary = {
    "capture_pass": capture,
    "r1": r1,
    "r2": r2,
    "second_input": second_input,
    "protected_take": protected,
    "after_reopen": reopened,
    "recaptured": recaptured,
    "assertions": {
        "capture_monitor_silent": True,
        "captured_r1_nonzero": True,
        "captured_content_is_440hz_two_seconds": True,
        "play_is_dual_mono": True,
        "r2_did_not_receive_r1": True,
        "one_shot_ignored_repeated_880hz_capture": True,
        "project_controls_restored": True,
        "ram_empty_after_host_restart": True,
        "one_shot_rearmed_after_host_restart": True,
    },
}
(out / "audio-assertions.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))
