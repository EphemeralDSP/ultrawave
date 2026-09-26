#!/usr/bin/env python3
"""Local MD/Scarlett bench: read-only preflight, bounded return probe, stimuli.

Requires numpy and, for MIDI, the existing machinedrum-oracle Python environment.
No machine assignments, Kit/Global writes, transport commands, or RAM captures.
See docs/testing/machinedrum-measurement.md before connecting audio.
"""

import argparse
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import time
import wave
from pathlib import Path

import numpy as np

RATE = 48000
CARD = "USB"
ROUTES = ("Analogue Output 03 Playback Enum", "Analogue Output 04 Playback Enum")


def run(*args):
    return subprocess.run(
        args, check=True, capture_output=True, text=True, timeout=10
    ).stdout


def save_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def mixer_value(name):
    text = run("amixer", "-c", CARD, "cget", f"name={name}")
    index = int(re.search(r": values=(\d+)\s*$", text).group(1))
    label = re.search(rf"Item #{index} '([^']+)'", text).group(1)
    return index, label


def set_mixer(name, value):
    run("amixer", "-c", CARD, "cset", f"name={name}", str(value))


def write_wav(path, audio):
    pcm = np.rint(audio * (2**31 - 1)).astype("<i4")
    with wave.open(str(path), "wb") as wav:
        wav.setparams((audio.shape[1], 4, RATE, 0, "NONE", "not compressed"))
        wav.writeframes(pcm.tobytes())


def routed_stimulus(mono, side):
    """ALSA playback slots 3/4 only; headphone/main slots 1/2 remain silent."""
    audio = np.zeros((len(mono), 6))
    for channel in {"left": (2,), "right": (3,), "both": (2, 3)}[side]:
        audio[:, channel] = mono
    return audio


def stimuli(root):
    t = np.arange(2 * RATE) / RATE
    fade = np.ones(len(t))
    edge = np.sin(np.linspace(0, np.pi / 2, RATE // 100)) ** 2
    fade[: len(edge)] = edge
    fade[-len(edge) :] = edge[::-1]
    sweep = np.sin(2 * np.pi * 20 * 2 / np.log(1000) * np.expm1(np.log(1000) * t / 2))
    tone = np.sin(2 * np.pi * 997 * t)
    cases = [(f"route-{side}", tone, -36, side) for side in ("left", "right")]
    cases += [(f"sweep-{abs(db)}dbfs", sweep, db, "left") for db in (-36, -24, -12)]
    cases += [
        (f"tone-{abs(db)}dbfs", tone, db, "left") for db in (-48, -36, -24, -12, -6)
    ]
    receipts = []
    for name, source, db, side in cases:
        mono = np.pad(source * fade * 10 ** (db / 20), (RATE // 4, RATE // 4))
        path = root / f"{name}.wav"
        write_wav(path, routed_stimulus(mono, side))
        receipts.append(
            {
                "file": path.name,
                "peak_dbfs": db,
                "side": side,
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            }
        )
    save_json(
        root / "stimuli.json",
        {
            "rate": RATE,
            "channels": 6,
            "duration_seconds": 2.5,
            "playback": "PCM3=L, PCM4=R; all other slots silent",
            "played_to_hardware": False,
            "cases": receipts,
        },
    )


def record_return(root, md):
    raw = root / "return-10ch.s32le"
    with (root / "arecord.log").open("w+") as log:
        process = subprocess.Popen(
            [
                "arecord",
                "-q",
                "-D",
                "hw:CARD=USB,DEV=0",
                "-t",
                "raw",
                "-f",
                "S32_LE",
                "-r",
                str(RATE),
                "-c",
                "10",
                "--samples=120000",
                str(raw),
            ],
            stderr=log,
        )
        triggered = False
        try:
            deadline = time.monotonic() + 5
            while not raw.exists() or raw.stat().st_size < 24000 * 40:
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError(
                        "Capture failed before the 0.5s baseline; no note sent"
                    )
                time.sleep(0.005)
            baseline = np.fromfile(raw, dtype="<i4", count=24000 * 10) / 2**31
            if np.max(np.abs(baseline)) > 0.05:
                raise RuntimeError("Active/loud baseline; no note sent")
            triggered = True
            md.note((0x90, 36, 64))
            time.sleep(0.08)
            md.note((0x80, 36, 0))
            if process.wait(timeout=6) != 0:
                raise RuntimeError("arecord failed")
        finally:
            # Stop only this child, never another host/capture process.
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=5)
            if triggered:
                md.note((0x80, 36, 0))
        log.seek(0)
        if re.search(r"overrun|xrun|error", log.read(), re.IGNORECASE):
            raise RuntimeError("Capture log reports an error; reject timing evidence")
    audio = np.fromfile(raw, dtype="<i4").reshape(-1, 10) / 2**31
    if len(audio) != 120000 or np.max(np.abs(audio)) >= 0.99:
        raise RuntimeError("Truncated or clipped capture")
    write_wav(root / "return-10ch.wav", audio)
    baseline_rms = np.sqrt(np.mean(audio[:24000] ** 2, axis=0))
    after_rms = np.sqrt(np.mean(audio[24000:] ** 2, axis=0))
    return {
        "baseline_rms": baseline_rms.tolist(),
        "after_rms": after_rms.tolist(),
        "peak": np.max(np.abs(audio), axis=0).tolist(),
        "strongest_capture_slot_1based": int(np.argmax(after_rms)) + 1,
        "input3_signal_over_baseline_db": float(
            20 * np.log10(max(after_rms[2], 1e-15) / max(baseline_rms[2], 1e-15))
        ),
    }


def preflight(root, oracle_source, probe):
    sys.path.insert(0, str(oracle_source))
    import mido
    from machinedrum_oracle.sysex import MachinedrumSession

    usb = Path("/proc/asound/USB").resolve()
    if (usb / "usbid").read_text().strip() != "1235:8213":
        raise RuntimeError("Expected Scarlett 8i6 USB identity not present")
    (root / "mixer-before.txt").write_text(run("amixer", "-c", CARD, "contents"))
    original_routes = [mixer_value(name) for name in ROUTES]
    save_json(root / "mixer-routes-before.json", dict(zip(ROUTES, original_routes)))
    names = [
        n
        for n in mido.get_output_names()
        if n.startswith("U2MIDI Pro:U2MIDI Pro MIDI 1 ")
    ]
    if len(names) != 1:
        raise RuntimeError("Expected exactly one U2MIDI Pro endpoint")
    journal = []
    restoration = {}
    try:
        with MachinedrumSession(
            output_name=names[0],
            client_name="U2MIDI Pro",
            port_name="U2MIDI Pro MIDI 1",
            usb_id="7104:1400",
            journal=lambda event, **data: journal.append({"event": event, **data}),
        ) as md:
            slots = {
                "global": md.status(1),
                "kit": md.status(2),
                "track": md.status(0x22),
            }
            g, k, stored = (
                md.global_dump(slots["global"]),
                md.kit_dump(127),
                md.kit_dump(slots["kit"]),
            )
            originals = {
                "global": g.raw,
                "current-kit": k.raw,
                "stored-kit": stored.raw,
            }
            for name, data in originals.items():
                (root / f"original-{name}.syx").write_bytes(data)

            def verify_state():
                return {
                    "global": md.global_dump(slots["global"]).raw == g.raw,
                    "current-kit": md.kit_dump(127).raw == k.raw,
                    "stored-kit": md.kit_dump(slots["kit"]).raw == stored.raw,
                    "global-slot": md.status(1) == slots["global"],
                    "kit-slot": md.status(2) == slots["kit"],
                    "selected-track": md.status(0x22) == slots["track"],
                }

            if not all(verify_state().values()):
                raise RuntimeError("Unstable state readback; refusing probe")
            save_json(
                root / "preflight.json",
                {
                    "slots": slots,
                    "track1_route": g.track_routes_raw[0],
                    "track1_model": k.models[0],
                    "track1_params": k.params[0],
                    "track1_level": k.levels[0],
                    "note36_track": g.key_map_raw[36],
                    "base_channel": g.base_channel_raw,
                    "tempo_raw": g.tempo_raw,
                    "ctrl_in": g.ctrl_in,
                    "clock_in": g.clock_in,
                    "midi_endpoint": names[0],
                },
            )
            if not probe:
                return
            if (
                g.track_routes_raw[0],
                g.key_map_raw[36],
                g.base_channel_raw,
                k.models[0],
            ) != (2, 0, 0, 32):
                raise RuntimeError(
                    "Probe requires existing EFM-BD track1, C route, note36/channel1; no state modified"
                )
            if not g.ctrl_in or mixer_value("PCM 03 Capture Enum")[1] != "Analogue 3":
                raise RuntimeError(
                    "MIDI receive or capture slot3 mapping differs; no state modified"
                )
            # Refuse to redirect audio another client might already be sending.
            busy = subprocess.run(
                ["fuser", f"/dev/snd/pcmC{usb.name[4:]}D0p"],
                capture_output=True,
                check=False,
            )
            if busy.returncode != 1:
                raise RuntimeError(
                    "Playback device busy or idle check failed; close its client first"
                )
            try:
                for name, source in zip(ROUTES, ("PCM 3", "PCM 4")):
                    set_mixer(name, source)
                    if mixer_value(name)[1] != source:
                        raise RuntimeError("Output isolation readback failed")
                analysis = record_return(root, md)
                save_json(root / "return-analysis.json", analysis)
                if (
                    analysis["strongest_capture_slot_1based"] != 3
                    or analysis["input3_signal_over_baseline_db"] < 30
                ):
                    raise RuntimeError(
                        "Return on input3 not established; inspect capture"
                    )
            finally:
                # Attempt both restorations even if one control fails.
                for name, (index, label) in zip(ROUTES, original_routes):
                    try:
                        set_mixer(name, index)
                        restoration[name] = mixer_value(name) == (index, label)
                    except Exception as error:  # noqa: BLE001 -- always attempt the other route
                        restoration[name] = str(error)
                restoration.update(verify_state())
                save_json(root / "restoration.json", restoration)
                if not all(value is True for value in restoration.values()):
                    raise RuntimeError(f"Restoration check failed: {restoration}")
    finally:
        save_json(root / "midi-journal.json", journal)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("preflight", "return-probe", "stimuli"))
    parser.add_argument(
        "output", type=Path, help="New private directory; never reused or overwritten"
    )
    parser.add_argument(
        "--oracle-source",
        type=Path,
        default=Path.home() / "Work/machinedrum-oracle/src",
    )
    args = parser.parse_args()
    os.umask(0o077)
    args.output.mkdir(parents=True, exist_ok=False)

    # SIGINT already raises KeyboardInterrupt. Give SIGTERM the same cleanup path.
    def interrupted(_signum, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupted)
    if args.action == "stimuli":
        stimuli(args.output)
    else:
        preflight(args.output, args.oracle_source, args.action == "return-probe")
    print(args.output)


if __name__ == "__main__":
    main()
