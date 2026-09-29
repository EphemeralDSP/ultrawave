# Ultrawave editor layout

One 920×650 pane. One instance owns one take. Record and Play are actions,
not modes; both stay visible. This design supersedes the two-page hardware replica.

| Region | Controls and feedback |
|---|---|
| Header | Title, compact Resample / Sample only selector, and small MIDI popover |
| Waveform | Shared sample, draggable S/E handles, dimmed excluded region, playback cursor; normal Start and End knobs sit beside it and edit the same parameters |
| Trim row | Forward/Reverse/Empty region, duration and source rate (Start/End values live with the knobs, not here) |
| Record card | Record button beside duration dropdown, Record once, contextual Re-arm, Ready/Recording/Protected, actual progress and elapsed/total time, one Gain knob; input meter |
| Inputs popover | Main + sample (Resample) / Main input (Sample only), Sidechain, Main balance and Side balance |
| Record FX popover | Sample rate in actual kHz, record Rate crush and Drive |
| Play card | Play button, playing/empty feedback; disabled when empty |
| Pitch & envelope | Pitch, Hold, Decay |
| Repeat | Hits (total, not additional repeats) and Interval in actual milliseconds at current host tempo; the minimum interval is not off |
| Character/output | Rate crush, Bit crush, Drive, Level and playback meter |

Use readable typography, restrained warm capture/cool playback accents and clear
grouping. Do not require tutorial prose on the faceplate. MIDI assignments are
dropdowns labeled Record and Play, showing both number and note name. Opening a
nested dropdown or editing a popover must keep it open. Scrolling
the note list must work from 0 through 127, without snapping back to the selection.

Resample is the default. It passes dry stereo main input plus the played mono
sample to the outputs. Recording combines the configured input mix with processed
sample feedback after playback effects, MIDI velocity and sample level. Inputs
popover levels and balances affect recording, never audible dry output. While
recording into staging, the old take can keep playing; the completed non-empty
take then replaces it and stops its old voice/repeat train. An empty/no-sample
recording leaves the previous take intact.

Sample only retains the original routing: Main input means the host main input,
not an implicit master tap; Sidechain is optional host-routed audio; sources mix
to mono before input-only capture; and output is sample-only, silent until
playback is triggered. An accepted capture replaces the sample and stops its old
voice/repeat train at capture start.

Record once protects the first take: show Protected, disable Record, and show
Re-arm only after the protected take finishes. Re-arm retains the take until the
next trigger. New instances start empty and unprotected, even after project restore.

Duration choices are beat-based, but show a 10-second cap when a choice would
exceed it and provide effective seconds feedback. Actual duration and routing
strategy are latched at record start. Record FX rate crush and Drive are retained
and printed into new recordings. Sample rate is displayed and edited as actual
kHz (Hz input is also accepted); generic host Capture Rate text is the divisor
÷1…÷16 because the host has no editor context. Source, drive and rate-crush
readouts are percentage amounts; Bit crush shows effective bits.

Hits values 0 and 1 both display `1 hit`; 127 remains `INF`. The editor's Interval
readout and numeric editing use actual milliseconds at current host tempo, while
generic host parameter text remains the MD raw code because it lacks tempo
context. Start and End retain numeric entry, knob/marker dragging, and crossing
for reverse playback.

Run `./scripts/screenshot.sh` and inspect empty and loaded images immediately
after GUI changes. Also exercise and inspect open MIDI/note dropdowns, Inputs and
Record FX, reverse interior markers, sample-only output and protected/re-arm states.
The isolated Renoise fixture verifies actual audio, FX-alias routing and state.
