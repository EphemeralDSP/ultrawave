# Pinned framework fixes

`nice-plug` and `nice-plug-core` are the published 0.4.2 crates, ISC licensed.
Their `.cargo_vcs_info.json` records upstream commit
`91ba75d20c43ca0cd21afa802d0ae8c0511f48ed`. LICENSE files are copied from that
revision. Upstream source is otherwise preserved except for the changes below.

## Why the local patch exists

Renoise 3.5.4 can call `setParamNormalized()` then `getState()` before processing
the queued parameter changes. Upstream ignores controller updates while active,
so its saved component state contains old values. Reproduced with Play Buffer
R2 → R1 and Level 0 → −3 dB, saving in the same Lua callback. This affects the
controlled immediate-save case; it does not establish that every ordinary save
is affected.

The VST3 wrapper now publishes controller/GUI edits as revision-tagged atomic
normalized values. Saving snapshots these before reading DSP state, then overlays
typed plain values (including stable enum IDs) without modifying DSP or smoothers.
Processing still uses the host's sample offsets. At block entry it snapshots
tokens into preallocated storage; after successfully applying a parameter point
it retires only that entry revision with one compare/exchange. Edits made during
the block survive, including A → B → A. Empty/unrelated blocks and stop do not
discard edits. Loading state invalidates them.

VST3 provides no edit IDs: a queued point cannot be proven to acknowledge a
particular controller edit. The policy is that a later block containing a point
for that parameter regains authority, regardless of value. New automation must
not remain masked by a pending edit. A delayed controller notification after a
block remains pending until a later point or state load. This is a protocol
ordering limitation, not an exact acknowledgement mechanism.

## Patch surface and tests

- Wrapper: `src/wrapper/state.rs`, `vst3.rs`, `vst3/{context,inner,wrapper,pending}.rs`.
- Core: `EnumParamInner::preview_plain_id()` for non-mutating serialization.
- Tests: `vst3/tests.rs`, plus revision/invalidation tests in `pending.rs`.
- Packaging: standalone vendor workspace; core path dependency points to its
  patched sibling. The root lockfile is authoritative. Test-only locks are ignored.

Run `./scripts/test-vst3-state.sh --release` for wrapper regressions and
`./scripts/renoise-test.sh` for the actual host save/reopen loop. Both dev-loop
modes run the wrapper tests. CLAP uses its unchanged wrapper/state path.

When upgrading, reproduce the immediate save against upstream first. Remove
these patches and the two local crates when upstream satisfies the regression
tests.

## baseview 0.3.4: join the X11 thread on editor close

This is the published MIT/Apache-2.0 crate, upstream revision
`bc987b96c22cbdb36507196dbbcad00c768e135b`, not the old NIH-plug/Vizia fork.
Only `src/platform/x11/window_thread.rs` is patched.

Upstream Drop stops the event loop, then calls `run_until_closed()`, which first
sends a synchronous Show request. If the loop exits without replying, `?` returns
before joining and logs `Error while closing window`. Unloading plugin code while
its detached thread finishes is unsafe. Drop now calls the extracted join-only
helper, with no request after stop; public run-until-closed still shows first.

The warning was reproduced under pluginval Editor Automation/lifecycle stress.
`scripts/test-editor-lifecycle.sh`, `validate-vst3.sh` and the Renoise fixture
reject it, even when the host exits zero. Re-run ten repetitions for both plugins
when upgrading this dependency. macOS code is unchanged from the published crate
and still requires testing on macOS.
