# Ultrawave editor layout

One 920×650 pane. One instance owns one take. Capture and Play are actions,
not modes; both stay visible. This design supersedes the two-page hardware replica.

| Region | Controls and feedback |
|---|---|
| Header | Title, explicit Input + sample / Sample only choices, small MIDI popover |
| Waveform | Shared sample, draggable S/E handles, dimmed excluded region, playback cursor |
| Trim row | Editable Start/End values, Forward/Reverse/Empty region, duration and source rate |
| Record card | Capture button beside duration dropdown, Record once, Re-arm, recording/protected/ready status |
| Capture sources | Track input, Sidechain, Gain; input meter |
| Capture details popover | Track/side balances, capture rate, rate crush and drive |
| Playback card | Play sample button, ready/playing feedback; disabled when empty |
| Pitch & envelope | Pitch, Hold, Decay |
| Repeat | Repeats and tempo-synced Interval |
| Character/output | Rate crush, Bit crush, Drive, Level and playback meter |
| Footer | New captures replace the take; project saving does not preserve audio |

Use readable typography, restrained warm capture/cool playback accents and clear
grouping. MIDI assignments are dropdowns showing both number and note name, with
an octave-naming disclaimer and an equal-note priority warning. Opening a nested
dropdown or editing capture details must keep the parent popover open. Scrolling
the note list must work from 0 through 127, without snapping back to the selection.

Track input means the host main input, not an implicit master tap. Sidechain is
optional host-routed audio. Both sources mix to mono before capture. Output
choices do not affect capture; Sample only mutes stereo dry-through.

An accepted capture replaces the sample and stops its old voice/repeat train.
Record once protects the first take: show SAMPLE PROTECTED, disable Capture,
enable Re-arm after capture completes. Re-arm retains the take until the next
trigger. New instances start empty and unprotected, even after project restore.

Run `./scripts/screenshot.sh` and inspect empty and loaded images immediately
after GUI changes. Also exercise and inspect open MIDI/note dropdowns, capture
details, reverse interior markers, sample-only output and protected/re-arm states.
The isolated Renoise fixture verifies actual audio, FX-alias routing and state.
