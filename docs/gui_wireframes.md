# Ultrawave editor layout

One 920×650 pane. One instance owns one take. Capture and Play are actions,
not modes; both stay visible. This design supersedes the two-page hardware replica.

| Region | Controls and feedback |
|---|---|
| Header | Title, compact Master / Sample only selector, and small MIDI popover |
| Waveform | Shared sample, draggable S/E handles, dimmed excluded region, playback cursor; normal Start and End knobs sit beside it and edit the same parameters |
| Trim row | Forward/Reverse/Empty region, duration and source rate (Start/End values live with the knobs, not here) |
| Record card | Capture button beside duration dropdown, Record once, Re-arm, recording/protected/ready status |
| Capture sources | Track input, Sidechain, Gain; input meter |
| Capture details popover | Track/side balances, capture rate, rate crush and drive |
| Playback card | Play sample button, playing/empty feedback; disabled when empty |
| Pitch & envelope | Pitch, Hold, Decay |
| Repeat | Hits (total, not additional repeats) and tempo-synced Interval; MIN is the fastest interval, not off |
| Character/output | Rate crush, Bit crush, Drive, Level and playback meter |

Use readable typography, restrained warm capture/cool playback accents and clear
grouping. MIDI assignments are dropdowns showing both number and note name, with
an octave-naming disclaimer and an equal-note priority warning. Opening a nested
dropdown or editing capture details must keep the parent popover open. Scrolling
the note list must work from 0 through 127, without snapping back to the selection.

Master is the default. It passes stereo track input plus the played mono sample
to the outputs, and captures that combined signal after playback effects, MIDI
velocity and sample level. While recording into staging, the old take can keep
playing; the completed non-empty take then replaces it and stops its old
voice/repeat train. An empty/no-sample capture leaves the previous take intact.

Sample only retains the original routing: Track input means the host main input,
not an implicit master tap; Sidechain is optional host-routed audio; sources mix
to mono before input-only capture; and output is sample-only, silent until
playback is triggered. An accepted capture replaces the sample and stops its old
voice/repeat train at capture start.

Record once protects the first take: show SAMPLE PROTECTED, disable Capture,
enable Re-arm after capture completes. Re-arm retains the take until the next
trigger. New instances start empty and unprotected, even after project restore.

Run `./scripts/screenshot.sh` and inspect empty and loaded images immediately
after GUI changes. Also exercise and inspect open MIDI/note dropdowns, capture
details, reverse interior markers, sample-only output and protected/re-arm states.
The isolated Renoise fixture verifies actual audio, FX-alias routing and state.
