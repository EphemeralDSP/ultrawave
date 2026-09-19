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


def tone_amplitude(samples, rate, frequency, frames=48000):
    samples = samples[:frames]
    cosine = sum(
        value * math.cos(2 * math.pi * frequency * frame / rate)
        for frame, value in enumerate(samples)
    )
    sine = sum(
        value * math.sin(2 * math.pi * frequency * frame / rate)
        for frame, value in enumerate(samples)
    )
    return 2 * math.hypot(cosine, sine) / len(samples)


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
reverse = metrics(out / "play-reverse.wav")
assert reverse["channels"] == 2 and reverse["sample_rate"] == 48000, reverse
# One captured bar at 120 BPM and 48 kHz is exactly 96000 frames. Compare
# every stereo frame in reverse order, not just frequency/energy or audibility.
forward_samples = read_wav(out / "play-r1.wav")[2]
reverse_samples = read_wav(out / "play-reverse.wav")[2]
expected_reverse = [
    sample
    for frame in reversed(range(96000))
    for sample in forward_samples[frame * 2 : frame * 2 + 2]
]
assert list(reverse_samples[:96000 * 2]) == expected_reverse, reverse
assert all(sample == 0.0 for sample in reverse_samples[96000 * 2:]), reverse
for name, interval, count in [
    ("play-retrig-120.wav", 6000, 2),  # sixteenth note at 120 BPM / 48 kHz
    ("play-retrig-60.wav", 12000, 2),
    ("play-retrig-infinite.wav", 750, None),  # 256 hits, beyond the old 127 cap
]:
    channels, rate, values, _ = read_wav(out / name)
    assert channels == 2 and rate == 48000, name
    # Eight beats must last eight seconds at 60 BPM, four seconds at 120.
    assert len(values) // 2 == (384000 if interval == 12000 else 192000), name
    for frame in range(len(values) // 2):
        hit = frame // interval
        if count is not None:
            hit = min(hit, count)
        source_frame = frame - hit * interval
        for channel in range(2):
            expected = forward_samples[source_frame * 2 + channel] if source_frame < 96000 else 0.0
            assert values[frame * 2 + channel] == expected, (name, frame, channel)
assert read_wav(out / "play-retrig-off.wav") == read_wav(out / "play-r1.wav")
side_dry = read_wav(out / "side-dry-660.wav")
side_wrong = metrics(out / "side-wrong-route-play.wav")
side_rejected = metrics(out / "side-rejected-balance-play.wav")
side_single = metrics(out / "side-single-play.wav")
side_sum_monitor = metrics(out / "side-sum-capture-monitor.wav")
side_sum = metrics(out / "side-sum-play.wav")
main_only = metrics(out / "main-only-play.wav")
assert 435 < main_only["estimated_frequency_hz"] < 445, main_only
main_samples = read_wav(out / "main-only-play.wav")[2][0::2]
for frame, actual in enumerate(main_samples):
    expected = (0.4 - 0.2 * frame / 96000) * math.sin(2 * math.pi * 440 * frame / 48000) if frame < 96000 else 0.0
    assert abs(actual - expected) <= 1 / 2048, (frame, actual, expected)
for name in ["side-single-capture.wav", "side-rejected-balance-capture.wav", "main-only-capture.wav"]:
    assert metrics(out / name)["peak"] < 1e-7, name
assert side_wrong["peak"] < 1e-7, side_wrong
assert side_rejected["peak"] < 1e-7, side_rejected
assert side_single["peak"] > 1e-4, side_single
assert 655 < side_single["estimated_frequency_hz"] < 665, side_single
assert side_single["stereo_delta_peak"] < 1e-7, side_single
dry_right = side_dry[2][1::2]
single_left = read_wav(out / "side-single-play.wav")[2][0::2]
assert len(dry_right) == len(single_left)
side_max_error = max(abs(actual - expected) for actual, expected in zip(single_left, dry_right))
assert side_max_error <= (1 / 2048), side_max_error
assert side_sum_monitor["peak"] > 1e-4, side_sum_monitor
assert side_sum["peak"] > 1e-4 and side_sum["stereo_delta_peak"] < 1e-7, side_sum
sum_left = read_wav(out / "side-sum-play.wav")[2][0::2]
amp_660 = tone_amplitude(sum_left, 48000, 660)
amp_1100 = tone_amplitude(sum_left, 48000, 1100)
assert 2.4 < amp_660 / amp_1100 < 2.6, (amp_660, amp_1100)
monitor_left = read_wav(out / "side-sum-capture-monitor.wav")[2][0::2]
monitor_max_error = max(abs(a - b) for a, b in zip(monitor_left, sum_left))
assert monitor_max_error <= (1 / 2048), monitor_max_error
host_result = (out / "host-result.txt").read_text()
assert "sidechain_receiver=5/1" in host_result, host_result
reopened = metrics(out / "play-after-reopen.wav")
assert reopened["peak"] < 1e-7, reopened  # RAM must not survive a full shutdown
recaptured = metrics(out / "play-recaptured.wav")
assert recaptured["peak"] > 1e-4 and recaptured["stereo_delta_peak"] < 1e-7, recaptured
assert 435 < recaptured["estimated_frequency_hz"] < 445, recaptured
assert 90000 < recaptured["active_extent_frames"] < 100000, recaptured
assert metrics(out / "recapture-after-reopen.wav")["peak"] < 1e-7
assert "project_controls_restored=true" in host_result
summary = {
    "capture_pass": capture,
    "r1": r1,
    "r2": r2,
    "second_input": second_input,
    "protected_take": protected,
    "reverse": reverse,
    "after_reopen": reopened,
    "recaptured": recaptured,
    "sidechain": {
        "dry": metrics(out / "side-dry-660.wav"),
        "wrong_route": side_wrong,
        "rejected_balance": side_rejected,
        "single": side_single,
        "sum": side_sum,
        "single_max_error_vs_dry_right": side_max_error,
        "sum_monitor_max_error_vs_play": monitor_max_error,
        "sum_660_amplitude": amp_660,
        "sum_1100_amplitude": amp_1100,
    },
    "assertions": {
        "capture_monitor_silent": True,
        "captured_r1_nonzero": True,
        "captured_content_is_440hz_two_seconds": True,
        "play_is_dual_mono": True,
        "r2_did_not_receive_r1": True,
        "one_shot_ignored_repeated_880hz_capture": True,
        "reversed_markers_reverse_every_sample": True,
        "retrigger_count_tempo_infinite_and_off_sample_exact": True,
        "project_controls_restored": True,
        "ram_empty_after_host_restart": True,
        "one_shot_rearmed_after_host_restart": True,
        "renoise_sidechain_receiver_accepted": True,
        "wrong_route_and_rejected_balance_silent": True,
        "single_and_summed_sidechain_audio_verified": True,
        "capture_monitor_matches_play_within_one_lsb": True,
    },
}
(out / "audio-assertions.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))
