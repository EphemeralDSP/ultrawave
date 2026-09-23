#!/usr/bin/env python3
"""Verify the isolated Renoise renders from the unified Ultrawave effect."""

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
        name = data[offset : offset + 4]
        size = struct.unpack_from("<I", data, offset + 4)[0]
        chunks[name] = data[offset + 8 : offset + 8 + size]
        offset += 8 + size + (size & 1)
    fmt, raw = chunks[b"fmt "], chunks[b"data"]
    audio_format, channels, rate = struct.unpack_from("<HHI", fmt)
    bits = struct.unpack_from("<H", fmt, 14)[0]
    if audio_format == 0xFFFE:  # WAVE_FORMAT_EXTENSIBLE
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
        values = [value / float(1 << (bits - 1)) for value in ints]
        encoding = "int%d" % bits
    else:
        raise AssertionError("unsupported WAV format=%d bits=%d" % (audio_format, bits))
    return channels, rate, values, encoding


def metrics(path):
    channels, rate, values, encoding = read_wav(path)
    left, right = values[0::channels], values[1::channels]
    active = [index for index, value in enumerate(left) if abs(value) > 1e-5]
    crossings = [index for index in range(1, len(left)) if left[index - 1] <= 0 < left[index]]
    frequency = None
    if len(crossings) > 2:
        frequency = rate * (len(crossings) - 1) / (crossings[-1] - crossings[0])
    return {
        "channels": channels,
        "sample_rate": rate,
        "frames": len(values) // channels,
        "encoding": encoding,
        "peak": max(map(abs, values)),
        "rms": math.sqrt(sum(value * value for value in values) / len(values)),
        "stereo_delta_peak": max(abs(a - b) for a, b in zip(left, right)),
        "active_extent_frames": (active[-1] + 1) if active else 0,
        "estimated_frequency_hz": frequency,
    }


def channels(path):
    channel_count, rate, values, _ = read_wav(path)
    assert channel_count == 2 and rate == 48000, path
    return values[0::2], values[1::2]


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


def assert_source_tone(samples, frequency, source_step=1):
    for frame, actual in enumerate(samples):
        source_frame = frame * source_step
        expected = (
            (0.4 - 0.2 * source_frame / 96000)
            * math.sin(2 * math.pi * frequency * frame / 48000)
            if source_frame < 96000
            else 0.0
        )
        assert abs(actual - expected) <= 1 / 2048, (frame, actual, expected)


out = pathlib.Path(sys.argv[1])
capture = metrics(out / "capture-pass.wav")
original = metrics(out / "play-original.wav")

# A bypassed reference proves the source reaches the device. The active effect
# is permanently sample-only and remains exactly silent during capture.
capture_reference_left, capture_reference_right = channels(out / "capture-source-reference.wav")
assert_source_tone(capture_reference_left, 440)
assert max(map(abs, capture_reference_right)) < 1e-7
capture_left, capture_right = channels(out / "capture-pass.wav")
assert max(map(abs, capture_left)) < 1e-7
assert max(map(abs, capture_right)) < 1e-7

# MIDI Play Note through the FX alias emits the private mono take to both sides.
assert original["peak"] > 1e-4 and original["rms"] > 1e-5, original
assert original["stereo_delta_peak"] < 1e-7, original
assert 435 < original["estimated_frequency_hz"] < 445, original
assert 90000 < original["active_extent_frames"] < 100000, original
original_left, original_right = channels(out / "play-original.wav")
assert len(original_left) == len(capture_reference_left)
# Capture rounds the 32-bit source to signed 12-bit codes. Allow half a code
# plus float/WAV rounding, rather than requiring unquantized passthrough.
capture_error = max(abs(actual - source) for actual, source in zip(original_left, capture_reference_left))
assert capture_error <= 1 / 4096 + 1e-7, capture_error
assert original_right == original_left

# Nonzero 880 Hz host input is overwritten: playback remains sample-exact.
playback_with_input = metrics(out / "playback-with-host-input.wav")
assert read_wav(out / "playback-with-host-input.wav") == read_wav(out / "play-original.wav")

# Independently prove the denied capture's 880 Hz source is present, then require
# exact silence from the active effect. Record Once keeps the sample unchanged.
second_reference_left, second_reference_right = channels(out / "second-capture-source-reference.wav")
assert_source_tone(second_reference_left, 880, source_step=2)
assert max(map(abs, second_reference_right)) < 1e-7
second_input = metrics(out / "second-capture.wav")
assert second_input["peak"] < 1e-7, second_input
protected = metrics(out / "play-record-once.wav")
assert read_wav(out / "play-original.wav") == read_wav(out / "play-record-once.wav"), protected
assert read_wav(out / "play-original.wav") == read_wav(out / "playback-control.wav")

reverse = metrics(out / "play-reverse.wav")
forward_samples = read_wav(out / "play-original.wav")[2]
reverse_samples = read_wav(out / "play-reverse.wav")[2]
expected_reverse = [
    sample
    for frame in reversed(range(96000))
    for sample in forward_samples[frame * 2 : frame * 2 + 2]
]
assert list(reverse_samples[: 96000 * 2]) == expected_reverse, reverse
assert all(sample == 0.0 for sample in reverse_samples[96000 * 2 :]), reverse

for name, interval, count in [
    ("play-retrig-120.wav", 6000, 2),
    ("play-retrig-60.wav", 12000, 2),
    ("play-retrig-infinite.wav", 750, None),
]:
    channel_count, rate, values, _ = read_wav(out / name)
    assert channel_count == 2 and rate == 48000, name
    assert len(values) // 2 == (384000 if interval == 12000 else 192000), name
    for frame in range(len(values) // 2):
        hit = frame // interval
        if count is not None:
            hit = min(hit, count)
        source_frame = frame - hit * interval
        for channel in range(2):
            expected = forward_samples[source_frame * 2 + channel] if source_frame < 96000 else 0.0
            assert values[frame * 2 + channel] == expected, (name, frame, channel)
assert read_wav(out / "play-retrig-off.wav") == read_wav(out / "play-original.wav")

# Re-arm is another rising edge. It permits the denied 880 Hz take to replace the sample.
rearmed = metrics(out / "play-rearmed.wav")
assert rearmed["peak"] > 1e-4 and rearmed["stereo_delta_peak"] < 1e-7, rearmed
assert 875 < rearmed["estimated_frequency_hz"] < 885, rearmed

# One instance has one sample. Starting a new capture at frame 24000 must stop
# the currently playing 880 Hz take immediately, then replace it with 440 Hz.
replacement_left, replacement_right = channels(out / "replacement-stops-playback.wav")
rearmed_samples = read_wav(out / "play-rearmed.wav")[2][0::2]
assert replacement_right[:24000] == rearmed_samples[:24000]
assert max(map(abs, replacement_right[24000:])) < 1e-7
assert replacement_left[:24000] == rearmed_samples[:24000]
assert max(map(abs, replacement_left[24000:])) < 1e-7
replacement_play = metrics(out / "play-replacement.wav")
assert read_wav(out / "play-replacement.wav") == read_wav(out / "play-original.wav"), replacement_play

# Keep the substantive main/sidechain routing checks. Sidechain capture is mono;
# wrong effect routing and the rejected balance channel remain silent.
side_reference = read_wav(out / "side-source-reference-660.wav")
side_wrong = metrics(out / "side-wrong-route-play.wav")
side_rejected = metrics(out / "side-rejected-balance-play.wav")
side_single = metrics(out / "side-single-play.wav")
side_sum = metrics(out / "side-sum-play.wav")
main_only = metrics(out / "main-only-play.wav")
assert side_wrong["peak"] < 1e-7, side_wrong
assert side_rejected["peak"] < 1e-7, side_rejected
assert side_single["peak"] > 1e-4 and side_single["stereo_delta_peak"] < 1e-7, side_single
assert 655 < side_single["estimated_frequency_hz"] < 665, side_single
reference_right = side_reference[2][1::2]
single_left = read_wav(out / "side-single-play.wav")[2][0::2]
side_max_error = max(abs(actual - expected) for actual, expected in zip(single_left, reference_right))
assert side_max_error <= 1 / 2048, side_max_error
assert 435 < main_only["estimated_frequency_hz"] < 445, main_only
assert_source_tone(read_wav(out / "main-only-play.wav")[2][0::2], 440)
assert side_sum["peak"] > 1e-4 and side_sum["stereo_delta_peak"] < 1e-7, side_sum
sum_left = read_wav(out / "side-sum-play.wav")[2][0::2]
amp_660 = tone_amplitude(sum_left, 48000, 660)
amp_1100 = tone_amplitude(sum_left, 48000, 1100)
assert 2.4 < amp_660 / amp_1100 < 2.6, (amp_660, amp_1100)

# The state saved into the XRNS must be genuinely spent, not merely have the
# Record Once switched on. A final 440 Hz Capture Note is followed by a denied
# 880 Hz Capture Note, and playback must reproduce the original take
# sample-for-sample while leaving the latch locked for the screenshot/save.
save_reference_left, save_reference_right = channels(out / "save-denied-source-reference.wav")
assert_source_tone(save_reference_left, 880, source_step=2)
assert max(map(abs, save_reference_right)) < 1e-7
save_denied = metrics(out / "save-record-once-denied.wav")
assert save_denied["peak"] < 1e-7, save_denied
save_protected = metrics(out / "save-record-once-play.wav")
assert read_wav(out / "save-record-once-play.wav") == read_wav(out / "play-original.wav"), save_protected

host_result = (out / "host-result.txt").read_text()
assert "ultrawave_effect_matches=1" in host_result, host_result
assert "ultrawave_instrument_matches_before_alias=0" in host_result, host_result
assert "fx_alias_target=1/2" in host_result, host_result
assert "sidechain_receiver=5/1" in host_result, host_result
assert "project_controls_and_fx_alias_restored=true" in host_result, host_result

# Actual process destruction clears the sample and spent latch. The saved FX alias
# then captures and plays with its restored custom MIDI note assignments.
reopened = metrics(out / "play-after-reopen.wav")
assert reopened["peak"] < 1e-7, reopened
recapture = metrics(out / "recapture-after-reopen.wav")
assert recapture["peak"] < 1e-7, recapture
recaptured = metrics(out / "play-recaptured.wav")
assert recaptured["peak"] > 1e-4 and recaptured["stereo_delta_peak"] < 1e-7, recaptured
assert 435 < recaptured["estimated_frequency_hz"] < 445, recaptured

summary = {
    "capture_source_reference": metrics(out / "capture-source-reference.wav"),
    "capture_active_effect_silence": capture,
    "fx_alias_playback": original,
    "replacement_stops_old_playback": metrics(out / "replacement-stops-playback.wav"),
    "replacement_take": replacement_play,
    "playback_with_nonzero_host_input": playback_with_input,
    "record_once_source_reference": metrics(out / "second-capture-source-reference.wav"),
    "record_once_second_input": second_input,
    "record_once_protected_take": protected,
    "saved_record_once_source_reference": metrics(out / "save-denied-source-reference.wav"),
    "saved_record_once_denied_input": save_denied,
    "saved_record_once_protected_take": save_protected,
    "reverse": reverse,
    "rearmed_take": rearmed,
    "after_reopen": reopened,
    "recaptured_after_reopen": recaptured,
    "sidechain": {
        "source_reference": metrics(out / "side-source-reference-660.wav"),
        "wrong_route": side_wrong,
        "rejected_balance": side_rejected,
        "single": side_single,
        "sum": side_sum,
        "single_max_error_vs_source_right": side_max_error,
        "sum_660_amplitude": amp_660,
        "sum_1100_amplitude": amp_1100,
    },
    "assertions": {
        "single_effect_classification_and_parameter_surface": True,
        "record_and_play_notes_use_actual_fx_alias": True,
        "active_effect_output_is_permanently_sample_only": True,
        "capture_and_denied_capture_are_silent_with_independent_source_evidence": True,
        "playback_parameter_rising_edge": True,
        "record_once_survives_host_reset": True,
        "replacement_stops_old_take_and_installs_new_take": True,
        "saved_instance_record_once_is_spent_and_audio_protected": True,
        "rearm_rising_edge_releases_record_once": True,
        "reverse_and_retrigger_are_sample_exact": True,
        "main_and_sidechain_routing": True,
        "project_controls_and_fx_alias_restored": True,
        "fresh_instance_starts_without_sample_or_spent_latch": True,
    },
}
(out / "audio-assertions.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))
