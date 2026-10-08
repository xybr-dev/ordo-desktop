# GPUI source migration and `gpui-base` re-qualification for T0.4a

## Decision

Move Ordo's GPUI from Zed's git repository to Longbridge's `gpui-pre`
crates.io snapshot, and move `gpui-base` from git revision
`7885c41663c7a6cc68ad0c99b1ba33550f807ff0` (0.5.2) to the crates.io release
built against that snapshot:

| Role | Before (T0.3, T0.4) | After (T0.4a) |
| --- | --- | --- |
| GPUI | `zed-industries/zed` git, revision `8b1497dbd22fb06f5838a7c0b84a1e54fafa71bc` | `gpui-pre =0.3.8`, a snapshot of Zed `279fe070bb389b79652e52065b2f001edcc0b11b` |
| Platform | the same Zed git source (`gpui_platform`) | `gpui-pre-platform =0.3.8` |
| Behavior layer | `gpui-base` 0.5.2, `longbridge/gpui-component` git | `gpui-base =0.7.1`, which itself requires `gpui-pre =0.3.8` |

The manifests rename each package back to its Zed name (`gpui`,
`gpui_platform`), so no Ordo source path changes. This is Route B of
`docs/research/gpui-fast-and-gpui-kit.md`, which records why the alternatives
were rejected. gpui-fast is out of scope (Route C2 is a later experiment behind
T0.6's harness).

This record supersedes the pin facts in `GPUI-BASE-T0.3.md`. Its behavior
selection, boundaries, and rejections remain in force unchanged. T0.4a drops
no adopted behavior. Its one addition is the operating-system reduced-motion
reading that `gpui_base::init` now performs, adopted as the closure of the
recorded reduced-motion pin gap (see "Pin gaps re-checked").

## GPUI supply

- **Source.** `gpui-pre` and its `gpui-pre-*` crates on crates.io, published by
  `huacnlee` (Longbridge), not Zed Industries. Each crate records the
  reproduced Zed commit in `[package.metadata.gpui-pre] zed-rev`, here
  `279fe070bb389b79652e52065b2f001edcc0b11b` (2026-10-04). The snapshot is
  built by GPUI Kit's `script/bump-gpui.ts`.
- **Divergence from Zed.** Four files in three crates carry a
  `Modified for gpui-pre` notice:
  `gpui-pre/src/action.rs` (crate-relative `actions!` derive paths),
  `gpui-pre-apple/build.rs`, and `gpui-pre-macros` (`gpui_macros.rs` plus the
  `gpui_pre_facade_paths.rs` rewrite, which activates only when `gpui-kit` is
  in the graph; xtask and `deny.toml` keep it out).
- **License.** Apache-2.0 for every `gpui-pre*` crate and for `gpui-base`; each
  ships `LICENSE-APACHE` and keeps Zed's notices. `cargo deny check` passes
  licenses over the complete locked graph.
- **Immutability.** crates.io versions are immutable and checksummed in
  `Cargo.lock`; the build no longer has any git source. `deny.toml` now allows
  none (`allow-git = []`).
- **Fallback if the supply stops.** Ordo stays on `=0.3.8`/`=0.7.1`, which
  remain downloadable unless yanked; a yank does not break a `--locked` build
  that already has the lock entry. If Longbridge stops publishing, the options
  are, in order: (1) freeze at the last qualified pair; (2) vendor the adopted
  `gpui-base` behaviors into `ui` (Apache-2.0 permits it; the `ui` seam keeps
  the change local) and return to a Zed git revision for GPUI, which is what
  T0.3 qualified; (3) publish the same snapshot under Ordo's control from the
  recorded `zed-rev`. Each reruns this qualification.

## Pin enforcement

`[workspace.metadata.ordo.gpui-supply]` in the root `Cargo.toml` records
`gpui-pre`, `zed`, and `gpui-base`. `cargo xtask verify` step 4 (and
`cargo xtask offline`) now fails when:

- a workspace requirement on `gpui-pre*` or `gpui-base` is not one exact
  `=x.y.z` crates.io requirement equal to the recorded version (caret, partial,
  range, pre-release, git, and path requirements all fail);
- `Cargo.lock` takes a `gpui-pre*` or `gpui-base` entry from anywhere but
  crates.io. The lock reflects `[patch]` and `[replace]`, so a path or git
  override of the snapshot fails here even though the manifest requirement
  is untouched. This was verified end to end with a real
  `[patch.crates-io] gpui-pre = { path = … }`;
- `Cargo.lock` resolves a `gpui-pre*` or `gpui-base` version other than the
  recorded one, more than one `gpui-pre` or `gpui-base`, or a `gpui-base` that
  does not depend on the locked `gpui-pre`;
- `Cargo.lock` or a workspace manifest contains any other GPUI-family name
  (`gpui`, `gpui-*`, or `gpui_*`). This is an allow-list, so it covers Zed's
  `gpui`, gpui-fast, the GPUI Kit layers, and forks the list never named;
- anything from `zed-industries/zed` or `longbridge/` comes from git; any
  other git source must still name a full revision.

The `zed` field is informational. The locked checksum already fixes the
snapshot's bytes, and `gpui-pre`'s own `zed-rev` metadata is authoritative.

`deny.toml` bans the named families over the full graph as a second line. The isolation policies
(`domain`/`presentation`, `rendering`, `gpui-base` private to `ui`) now
recognize every GPUI-family name by prefix rather than by a list, because Cargo
metadata reports the `gpui-pre` package names, not the renamed `gpui` keys.
Each rule has a unit test in `xtask/src/lib.rs`. A manual mutation (changing
`gpui-base = "=0.7.1"` to `"0.7.1"`) fails with
`ui requires gpui-base ^0.7.1, expected exactly =0.7.1`.

Advisory exceptions were re-checked at the new pins. `RUSTSEC-2025-0141`
(syntect) no longer applies and was removed. The remaining four still apply:
`instant` comes directly from `gpui-base`, and the others come from the GPUI
graphics, SVG, and font stacks.

## Source changes required by the new revision

Only one source change was needed:

- `InputState::set_value` now takes `impl Into<InputContent>`, which has no
  conversion from a generic `impl Into<SharedString>`.
  `ui::primitives::Field::set_value` converts to `SharedString` first. Callers
  and behavior are unchanged.

## Behavior re-qualification

Every behavior adopted in `GPUI-BASE-T0.3.md` (Button, Input/InputBase,
Tooltip, Select/Combobox, Tabs, Collapsible, Sheet focus/dismiss,
ToastManager, and `motion::transition`) passes its existing focus, keyboard,
AccessKit role, name, and state tests with no test change. That covers the
`ui` lib suite, including the lab E2E
`the_milestone_keyboard_path_holds_state_focus_and_containment_across_viewport_switch`,
the `ordo` lib suite, including the shell E2E
`the_milestone_shell_path_holds_primitive_behavior_without_workspace_scroll`,
and every suite listed in `T0.3-MANUAL-GATE.md` §1.

Compared with the old pin, `gpui_base::init` now also initializes
`reduce_motion`, `root`, `text`, and `time_field` (`number_input` and `tree`
were already initialized). Every key binding these modules add is scoped to its
own key context (`Root`, `TextView`, `TimeField`, and so on). Ordo
renders only the `OrdoShell` and `OrdoLab` contexts, so none of them can fire.
`reduce_motion` is the only one with an effect, and that effect is wanted (see
the pin gaps below).

Behavior still recorded as a gap: GPUI exposes no disabled or read-only
AccessKit setter, so the description-based announcement stays.

### Recorded difference: image-asset completion

The canonical-slide lab's key test
(`lab_keys_cycle_fixture_viewport_scale_and_density`) asserted the old pin's
mechanism: a finished decode scheduled an on-next-frame callback, so the test
required `Window::simulate_next_frame` to deliver at least one callback.
`gpui-pre` rewrote the asset cache (`asset_cache.rs`, `CachedLoad`). A pending
load now records every view that called `use_asset`, and completion calls
`cx.notify` on each of them directly. No next-frame callback exists anymore.

- **Resolution.** The test now asserts the behavior instead: with no explicit
  notify, refresh, or simulated frame, the decode alone re-renders the lab and
  the incoming slide replaces the outgoing one. The other assertions (the
  outgoing image leaves the asset cache and atlas) are unchanged and pass.
- **Hazard closed upstream.** The old pin's `get_render_image` took the decode's
  first waiter away from same-frame `use_asset` calls, which could leave a
  viewport black. `fetch_asset` no longer subscribes or steals anything, so
  this hazard is gone. A mutation that switches the retire path to
  `get_render_image` now passes, because the viewports' `img` elements
  subscribe the same view. The production code still uses `use_render_image`,
  so the retire does not rely on that. Its comment now says so.
- **Accepted reduction.** The test therefore no longer tells `use` from `get`
  on the retire path. It still fails when the retire never takes the screen:
  a mutation that makes `retire_outgoing_upload` return early fails with "the
  incoming slide must take the screen once decoded".

## Pin gaps re-checked

| Gap | Status at `gpui-pre 0.3.8` / `gpui-base 0.7.1` | Ordo workaround |
| --- | --- | --- |
| Private test window | **Open.** `TestAppContext::test_window` is still `pub(crate)`. `TestWindow::simulate_appearance_change` exists but cannot be reached, and the public `simulate_*` surface still has no appearance change. The test platform still implements no `a11y_init`, so `debug_a11y_tree_json` stays `None` under test. `gpui-base` 0.7's new `test_support` reads per-element facts and does not expose a window tree. | Kept: appearance mapping and transition tests, plus the manual theme-following check; element-level AccessKit capture. |
| Tooltip on keyboard focus | **Open.** `div.rs` still registers tooltips only from mouse handlers, and the prepaint hover check still begins with `!window.last_input_was_keyboard()`. | Kept: the full text reaches keyboard users through the accessible name. |
| High-contrast signal | **Open.** No high-contrast or forced-colors API in `gpui-pre`. | Kept: Ordo paints its focus treatment in every mode. |
| AccessKit live regions | **Open.** The `aria_*` setters on `div` include label, description, selected, expanded, toggled, value, and position/level, but no live-region setter. | Kept: `ui::accessibility::LiveElement`. |
| wgpu row padding | **Open.** `gpui-pre-wgpu/src/wgpu_atlas.rs` still uploads with `bytes_per_row = width * bytes_per_pixel`, unpadded. | Kept: the 64-px row padding in `crates/ordo/src/canonical_views.rs`. |
| Scroll indicator | **Open in GPUI.** GPUI still ships no scrollbar element; `scrollbar_width` only reserves layout space. `gpui-base` exports a `scrollbar`, but adopting it is a new behavior qualification and outside T0.4a. | Kept: `ui::scroll`. |
| OS reduced motion | **Closed (Windows and macOS: read at startup only).** `gpui_base::init`, which `ui::init` already calls, now reads the platform setting into `App::reduce_motion`: Windows `SPI_GETCLIENTAREAANIMATION`, the Linux XDG portal `reduced-motion` key (followed live), and macOS. Under the test scheduler it does not consult the platform. | None to remove. Ordo never had a workaround here: it already honored `App::reduce_motion` and only lacked the platform reading. Ordo never sets the flag itself, so the system value is never overridden. No automated test covers the platform read, because `gpui-base` skips it under the test scheduler; the Windows manual gate below covers it. Following a *live* change on Windows needs a re-read (`apply_system_reduce_motion`) that Ordo does not yet trigger; that is recorded, not fixed here. |

No closed gap had Ordo code to remove.

## Rendering and native evidence

Canonical rendering does not depend on GPUI (`rendering` has no GPUI edge, and
xtask enforces this), so these runs check that the migration left it untouched.
Both ran on the new stack on Windows 11 (rustc 1.96.0), using the same commands
as the CI jobs:

- **Windows shaping probe.** Regenerated with `ORDO_UPDATE_FIXTURES=1`, no
  `git diff`; sha256 `d3976b55459555c19f617f636f2a5e41b78ea1dd95ebea6d7858504efa834b71`,
  identical to `RENDERING-T0.4.md`.
- **Canonical invariance.** `canonical_invariance` 3/3 against the committed
  golden; sha256 `488bb63d6e19dc23b3f924cb9be5a225504f0bf567f03ad5735d64bceb5a1b93`.
  That is the golden committed by T0.4.10 (`8f582df`). The
  `5708fd35…` value in `RENDERING-T0.4.md`'s packaged-invariance section
  matches neither committed golden, so it predates T0.4.10's regeneration and
  should be refreshed by the T0.4 owner.
- The T0.3 and T0.4 suites, including the canonical-slide lab test, pass on the
  new stack; see the commands below.

## Manual gate (pending)

`T0.3-MANUAL-GATE.md` scripts A (OS theme following), B (keyboard-only focus),
and C (Windows High Contrast) must be repeated on Windows at this stack, with a
screen-reader pass (Narrator or NVDA) over the shell's rail, a list, a Dialog,
and a Toast. Record the tester, the date, the commit, and the observations here.
The `[MANUAL]` box stays open until then.

| Script | Result |
| --- | --- |
| A. OS theme following | pending |
| B. Keyboard-only visual pass | pending |
| C. Windows High Contrast | pending |
| Screen reader (Narrator/NVDA) | pending |
| Reduced motion (Windows "Animation effects" off before launch: dialog and slideover open without movement) | pending |

## Commands

```
cargo xtask verify
cargo test -p rendering --locked --test shaping_probe   # with ORDO_UPDATE_FIXTURES=1, then git diff
cargo test -p native-runtime-tests --locked --test canonical_invariance
```
