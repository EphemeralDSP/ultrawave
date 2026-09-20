# Ultrawave editor layout

One 760×460 monochrome editor, two pages. Preserve the sharp rectangles, ticked
encoders, black/white palette and compact typography of
[the hardware reference](../reference/gui_reference.png).

| Region | Record page | Play page |
|---|---|---|
| Header | Title, One Shot, Re-arm, Capture | Title, Audition |
| Sidebar | Capture LEV, SRR, DIST and input meter | Playback LEV, SRR, DIST and playback meter |
| Knob row 1 | MAIN, BAL, SIDE, BAL | PTCH, DEC, HOLD, BRR |
| Knob row 2 | REC NOTE, PLAY NOTE, LEN, RATE | STRT, END, RTRG, RTIM |
| Waveform | Selected buffer | Selected buffer with draggable start/end and cursor |
| Status | Empty / Armed / Recording / Locked / Ready, duration, source rate | Empty / Playing / Ready, duration, source rate |
| Footer | Rn indicator, Record/Play page buttons, output routing, [R1]–[R4] | Same controls |

MAIN and SIDE mean the host's main input and optional sidechain, not an implicit
master tap. Dry + Playback preserves stereo input; Playback Only mutes dry.
These replace the former CUE controls. The buffer selector is shared by both
pages; capture/play triggers use its current selection, while running takes and
voices retain their latched buffer.

Run `./scripts/screenshot.sh` and inspect **both** images immediately after GUI
changes. Also exercise changed interactive states (One Shot, output mode,
buffer selection, reverse markers) in the rendered editor. Images are in
`screenshots/`; the Renoise fixture separately exercises actual host audio/state.
