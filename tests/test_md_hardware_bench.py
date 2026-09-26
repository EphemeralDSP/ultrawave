"""Offline checks only: no MIDI, mixer access, or audio playback."""

import importlib.util
import json
import sys
import tempfile
import unittest
import wave
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import MagicMock, patch

import numpy as np

spec = importlib.util.spec_from_file_location(
    "bench", Path(__file__).parents[1] / "scripts/md-hardware-bench.py"
)
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)


class BenchTests(unittest.TestCase):
    def test_channel_routing_never_touches_monitor_outputs(self):
        signal = np.array([0.125, -0.5, 0.0, 0.75])
        for side, channels in (("left", (2,)), ("right", (3,)), ("both", (2, 3))):
            with self.subTest(side=side):
                audio = bench.routed_stimulus(signal, side)
                self.assertEqual(audio.shape, (4, 6))
                for channel in range(6):
                    np.testing.assert_array_equal(
                        audio[:, channel],
                        signal if channel in channels else np.zeros(4),
                    )

    def test_mixer_readback_uses_selected_item_not_first_matching_name(self):
        response = (
            "Item #1 'Analogue 1'\nItem #11 'Mix C'\nItem #19 'PCM 3'\n : values=19\n"
        )
        with patch.object(bench, "run", return_value=response):
            self.assertEqual(bench.mixer_value("irrelevant"), (19, "PCM 3"))

    def test_capture_failure_restores_both_routes_and_verifies_kit(self):
        md = MagicMock()
        md.status.return_value = 0
        md.global_dump.return_value = SimpleNamespace(
            raw=b"global",
            track_routes_raw=[2],
            key_map_raw=[0] * 128,
            base_channel_raw=0,
            ctrl_in=True,
            clock_in=False,
            tempo_raw=3600,
        )
        md.kit_dump.return_value = SimpleNamespace(
            raw=b"kit",
            models=[32],
            params=[[64] * 24],
            levels=[100],
        )
        session = MagicMock()
        session.return_value.__enter__.return_value = md
        modules = {
            "mido": SimpleNamespace(
                get_output_names=lambda: ["U2MIDI Pro:U2MIDI Pro MIDI 1 36:0"]
            ),
            "machinedrum_oracle.sysex": SimpleNamespace(MachinedrumSession=session),
        }
        routes = {
            bench.ROUTES[0]: (11, "Mix C"),
            bench.ROUTES[1]: (12, "Mix D"),
            "PCM 03 Capture Enum": (3, "Analogue 3"),
        }
        original = dict(routes)

        def set_route(name, value):
            routes[name] = {
                "PCM 3": (19, "PCM 3"),
                "PCM 4": (20, "PCM 4"),
                11: (11, "Mix C"),
                12: (12, "Mix D"),
            }[value]

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with (
                patch.dict(sys.modules, modules),
                patch.object(Path, "resolve", return_value=Path("/proc/asound/card3")),
                patch.object(Path, "read_text", return_value="1235:8213"),
                patch.object(bench, "run", return_value="mixer snapshot"),
                patch.object(bench, "mixer_value", side_effect=routes.__getitem__),
                patch.object(bench, "set_mixer", side_effect=set_route),
                patch.object(
                    bench.subprocess, "run", return_value=SimpleNamespace(returncode=1)
                ),
                patch.object(
                    bench, "record_return", side_effect=RuntimeError("capture failed")
                ),
                self.assertRaisesRegex(RuntimeError, "capture failed"),
            ):
                bench.preflight(root, Path("unused"), True)
            self.assertEqual(routes, original)
            self.assertTrue(
                all(json.loads((root / "restoration.json").read_text()).values())
            )
            self.assertEqual((root / "original-current-kit.syx").read_bytes(), b"kit")
            # The failure path queries again rather than trusting the initial dump.
            self.assertEqual(md.kit_dump.call_count, 6)

    def test_stimulus_wavs_have_declared_levels_padding_and_channel_order(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bench.stimuli(root)
            receipt = json.loads((root / "stimuli.json").read_text())
            self.assertFalse(receipt["played_to_hardware"])
            self.assertEqual(len(receipt["cases"]), 10)
            for case in receipt["cases"]:
                with self.subTest(file=case["file"]):
                    with wave.open(str(root / case["file"])) as wav:
                        self.assertEqual(
                            (
                                wav.getnchannels(),
                                wav.getsampwidth(),
                                wav.getframerate(),
                            ),
                            (6, 4, 48000),
                        )
                        self.assertEqual(wav.getnframes(), 120000)
                        audio = (
                            np.frombuffer(wav.readframes(120000), "<i4").reshape(-1, 6)
                            / 2**31
                        )
                    channel = 2 if case["side"] == "left" else 3
                    self.assertEqual(
                        np.count_nonzero(np.delete(audio, channel, axis=1)), 0
                    )
                    self.assertEqual(np.count_nonzero(audio[:12000]), 0)
                    self.assertEqual(np.count_nonzero(audio[-12000:]), 0)
                    expected_peak = 10 ** (case["peak_dbfs"] / 20)
                    self.assertAlmostEqual(
                        float(np.max(np.abs(audio))), expected_peak, places=7
                    )
                    if case["file"].startswith("tone"):
                        # One second entirely within the flat envelope; bin 997 must dominate.
                        spectrum = np.abs(np.fft.rfft(audio[24000:72000, channel]))
                        self.assertEqual(int(np.argmax(spectrum)), 997)


if __name__ == "__main__":
    unittest.main()
