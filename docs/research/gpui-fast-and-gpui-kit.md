# Research: adopting gpui-fast and GPUI Kit

Status: research for review. Nothing here changes a pin, a milestone, or
a decision record. Evidence was gathered on 2026-10-07 against
`longbridge/gpui-fast@8e1e198`, `longbridge/gpui-kit@dc00da18`, and the
crates.io registry.

Owner direction (2026-10-07): the GPUI source and `gpui-base` upgrade is its
own task, **T0.4a**, which runs after T0.4 and before T0.5. It is recorded in
`MILESTONES.md`, which owns its acceptance criteria. See
[Proposed sequencing](#proposed-sequencing).

## Summary

- **Ordo already uses GPUI Kit.** `longbridge/gpui-kit` is the renamed
  `longbridge/gpui-component`; the GitHub API answers both names with the same
  repository. Ordo's `gpui-base` dependency (`Cargo.toml:39`, pinned at
  `7885c416`, 2026-08-24, package `0.5.2`) comes from it. So "adopt GPUI Kit"
  really means **upgrading the qualified `gpui-base` from 0.5.2 to 0.7.x**.
  Ordo uses only that unstyled behavior layer, and only a handful of its
  behaviors.
  It does **not** use GPUI Kit's styled components (`gpui-component`).
  `deny.toml` bans that crate, and Decision 252 rejects it as a wholesale
  dependency (decision [0253](../decisions/0253-use-short-internal-crates-and-an-interactive-ui-laboratory.md)). See
  [GPUI Kit's components](#gpui-kits-components).
- **gpui-fast is a copy of Zed's GPUI, not an alternative to it.** It imports
  Zed's `crates/gpui*` unchanged and adds its own logic in `fast/` modules with
  one-line hooks. It merges Zed upstream regularly (now at
  `a1b71072e5`, 2026-10-05) and keeps upstream's public API by design. Leaving
  Zed for gpui-fast would put a three-to-four-person, two-week-old layer on top
  of Zed's GPUI.
- **Zed's GPUI is not slow-moving; Ordo's pin is old on purpose.** `crates/gpui`
  received 98 commits between Ordo's pin (`8b1497db`, 2026-08-17) and
  2026-10-07. Ordo is seven weeks behind because of its own policy: upgrades are
  scheduled qualification work (decision [0252](../decisions/0252-replace-electron-with-a-clean-slate-rust-and-gpui.md), `MVP-PLAN.md:252`).
  Longbridge adds three things on top of Zed: crates.io snapshots
  (`gpui-pre`), behavior and components (GPUI Kit), and a performance fork
  (gpui-fast).
- **The `gpui-base` upgrade forces the GPUI source question.** Since 0.6,
  `gpui-base` no longer reads GPUI from Zed's git repository. It reads the
  exactly pinned `gpui-pre =0.3.8` crates.io snapshots, with an optional
  `gpui-fast` backend. Ordo cannot move `gpui-base` forward and also stay on
  `zed-industries/zed` git without two incompatible GPUI type universes. Any
  `gpui-base` upgrade is therefore also a GPUI source migration.
- **Retained mode is attractive, but unmeasured for Ordo and too young to
  adopt now.** gpui-fast's own benchmarks show −73% to −94% main-thread CPU per
  frame on mostly unchanged windows. Ordo has no frame-cost measurement yet;
  T0.6 creates it. gpui-fast calls itself "experimental", has issues disabled,
  shipped three crates.io releases in two days (0.1.0–0.1.2), and switches
  retention off whenever an accessibility client is active.

**Recommendation:**

1. Do not switch during T0.4, which is the active task.
2. Immediately after T0.4 and before T0.5, run T0.4a, which moves to
   `gpui-pre` + `gpui-base 0.7.x` (owner-confirmed). Ordo needs this move
   anyway to keep `gpui-base` upgradable.
3. Build T0.6's frame-time and memory instrumentation on the upgraded stack.
4. Then run gpui-fast as an A/B experiment through its `[patch]` compatibility
   route against the T0.6 harness. Its public API is upstream's, so the
   experiment costs a manifest change, and so does reverting it.
5. Adopt gpui-fast only if it materially improves a budgeted metric without
   costing the memory budget.

The coding habits retained mode rewards are cheap and also benefit upstream
GPUI's `cached` views, so they can start now (see
[Writing Ordo for retention](#writing-ordo-for-retention-now)).

## Terminology: two different "retained" ideas

The word "retained" already means something in Ordo, and it is not what
gpui-fast means.

- **Ordo's "retained path"** (Decision 261, T0.6 AC at `MILESTONES.md:383-391`)
  is a *display-path* choice. A canonical `Frame` is painted into the GPUI
  scene as glyphs, quads, and images, instead of being rasterized to RGBA on
  the CPU and uploaded as an image. It decides how slide pixels reach the
  operator viewport and Program.
- **gpui-fast's "Retained Mode"** is a *framework* optimization. GPUI keeps
  each view's previous output (layout nodes, hitboxes, listeners, primitives)
  and reuses it while nothing the view read has changed. It applies to every
  view in the window, Ordo's chrome included.

The two are independent and compose: a slide painted through Ordo's retained
display path, inside a view gpui-fast retains, would not be re-walked when
nothing changed. When this document says "retention", it means gpui-fast's.

## What each project is today

| | gpui-fast | GPUI Kit (`gpui-base`) | `gpui-pre` |
| --- | --- | --- | --- |
| What it is | Zed's GPUI plus `fast/` modules: retained views, retained Taffy layout, text-measurement carry-over, scroll layers (cached GPU tiles), native window composition | Behavior layer (`gpui-base`), styled layer (`gpui-component`), assets, shell, webview | Weekly crates.io snapshot of Zed's GPUI crates |
| Repository | `longbridge/gpui-fast`, created 2026-09-24 | `longbridge/gpui-kit` (was `gpui-component`), created 2024-06-13 | built by gpui-kit's `script/bump-gpui.ts` from Zed; README points to Zed |
| Latest | 0.1.2 (2026-10-07); 0.1.0 and 0.1.1 on 2026-10-06 | 0.7.1 (2026-10-05); weekly cadence since 0.6.0 | 0.3.8 = `zed@279fe07` (2026-10-04) |
| Upstream base | `zed@a1b71072e5` (2026-10-05), recorded in `UPSTREAM` | `gpui-pre =0.3.8`, optional `gpui-fast 0.1.1` | — |
| Publisher | crates.io owner `huacnlee` | `huacnlee` | `huacnlee` (not Zed Industries) |
| License | Apache-2.0 (Zed copyright, modified fork) | `gpui-base` crate: Apache-2.0 | Apache-2.0, Zed notices kept |
| Maturity signals | README: "experimental". Issues disabled. ~3–4 Longbridge authors (Sunli, Jason Lee, Floyd Wang, ihavecoke); the rest of the history is imported Zed commits. CI on Linux, macOS, and Windows, including D3D11 scroll-layer pixel tests | 16.4k stars. Ships in Longbridge Pro. 408 commits since Ordo's pin; `crates/base/src` +63.5k / −5.7k lines across 168 files | Not pristine upstream: macros carry an injected `gpui_pre_facade_paths` rewrite, active only when `gpui-kit` is in the dependency graph |

`gpui-pre` is a Longbridge-maintained snapshot, not a Zed release. It is a
convenience that makes GPUI publishable on crates.io. The only `gpui` crate Zed
publishes on crates.io is 0.2.2 (2025-10-22), which neither project uses.

## GPUI Kit's components

GPUI Kit has three layers, and Ordo takes the bottom one only.

| Layer | Crate | Ordo today |
| --- | --- | --- |
| Behavior: interaction, focus, keyboard, measurement, state; no visual language | `gpui-base` | **Used, selectively.** Six files in `crates/ui`, behind Ordo wrappers. xtask stops it from leaving `ui`. |
| Styled components: 60+ shadcn-style controls with theme, sizing, icons, i18n | `gpui-component` | **Not used.** Banned in `deny.toml` (`bans.deny`). Rejected by decision [0253](../decisions/0253-use-short-internal-crates-and-an-interactive-ui-laboratory.md), by `DESIGN.md:371-375` ("never defines Ordo's visual language"), and by the T0.3 qualification (`GPUI-BASE-T0.3.md:36`). |
| Everything else: assets, `gpui-shell` (QuickJS scripting), `gpui-wry` (WebView), FPS overlay | `gpui-kit-assets`, `gpui-shell`, `gpui-wry`, `gpui-fps` | Not used. A WebView or script runtime would contradict Ordo's scope; `wry` is banned too. |

### What Ordo takes from `gpui-base`

The T0.3 qualification (`GPUI-BASE-T0.3.md:19-36`) adopted these behaviors:

- `Button`, including its tooltip;
- `Input` / `InputBase`;
- `Select` (its popup combobox behavior);
- `Tabs` / `Tab`;
- `Collapsible`;
- `Sheet`'s focus trap and dismissal;
- `ToastManager`'s bookkeeping;
- `motion::transition`.

It explicitly rejected these:

- Base `Dialog` and `Popover`, as modal or menu roots;
- `ToastStack`;
- springs;
- `Theme` and the `StyledExt` visual helpers.

Ordo draws every pixel itself from `DESIGN.md` tokens.

### Should Ordo take the styled components?

No. The reasons are the same as when T0.3 decided this, and the upgrade does
not change them:

- **Visual ownership.** `gpui-component` is a product vocabulary: its theme,
  sizing scale, and icon set. Ordo's look is defined by `DESIGN.md` and its
  approved references. Adopting styled controls means either restyling each
  one against a theme system Ordo does not own, or looking like "a stock
  component-library demo", which `DESIGN.md:371` forbids.
- **Accessibility and semantics.** T0.3 already found base behaviors whose
  roles were wrong for Ordo, such as `Dialog`'s unnamed host and `Popover`'s
  Dialog role. The styled layer wraps the same behaviors and adds more
  decisions Ordo would have to audit.
- **Dependency weight.** `gpui-component` pulls in `notify`, `rust-i18n`,
  `markdown`, `lsp-types`, `ropey`, `similar`, `uuid`, chart code, and the Kit
  icon assets, plus optional tree-sitter grammars. Most of that serves an
  editor or trading terminal, not a presenter.
- **The seam is already in the right place.** Ordo's `ui` primitives are deep
  Modules: callers see Ordo's Button and Field, never a library type.
  Swapping a behavior underneath is a local change. Adopting styled
  components would push library types into every screen and remove that
  locality.

The styled layer is still useful as **reference source**. Its controls show how
the base behaviors are meant to be composed, in the same way the HTML
acceptance references are visual references only.

### What the 0.7 upgrade offers `gpui-base` users

Modules added since Ordo's pin (0.5.2 → 0.7.1; none were removed):

- `nav_stack`, `observe`, `plot`, `questionnaire`, `reduce_motion`, `root`;
- `scroll_bounce`, `scrollable_mask`, `selectable_text`, `test_support`, `text`;
- `time_field`, `toolbar`, `touch_selection`, `undo_history`.

Three of these, plus three modules that already existed, map onto documented
Ordo gaps or upcoming work. Each still needs the usual qualification before
use:

| Ordo need | `gpui-base` 0.7 candidate | Note |
| --- | --- | --- |
| OS reduced-motion preference (a recorded T0.3 gap, `GPUI-BASE-T0.3.md` "`App::reduce_motion` is a third gap") | `reduce_motion`, run from `gpui_base::init` | It reads the Windows `SystemParametersInfoW` setting and the Linux XDG portal setting into `App::reduce_motion`. **This closes the gap with no Ordo code**, because Ordo already calls `gpui_base::init` and already honors the flag. |
| Virtualized thumbnails (T0.6 AC) and an ~800-Cue timeline | `virtual_list` (existed at the pin) | Evaluate it against GPUI's own `uniform_list`/`list`. Ordo uses neither yet. |
| Undo affordances in toasts (T0.3 "expiring confirmation/Undo") | `undo_history` | Ordo's Undo semantics belong to the domain and the Presentation Engine. At most this would be UI plumbing. Probably reject. |
| Test helpers | `test_support` | May cover gaps Ordo works around today; the test window is still `pub(crate)` in GPUI. |
| Library tables and trees (Phase 2) | `table`, `tree` (existed at the pin) | Defer to Phase 2 qualification. |
| Scroll indicator | `scrollbar` (exported at the pin) | Ordo built its own `ui::scroll`. The T0.3 record discusses only GPUI's missing scrollbar, and says "Adopting a base scrollbar later reruns this qualification" (`GPUI-BASE-T0.3.md:898`). Evaluate the base scrollbar during the upgrade; one that keeps its state in entities would also suit gpui-fast. |

## How each path fits Ordo's dependency graph

Ordo today (`Cargo.toml:30-43`):

```toml
gpui          = { git = "https://github.com/zed-industries/zed" }   # rev from Cargo.lock: 8b1497db
gpui_platform = { git = "https://github.com/zed-industries/zed", features = [...] }
gpui-base     = { git = "https://github.com/longbridge/gpui-component", rev = "7885c416" }
```

This works only because `gpui-base 0.5.2` named the same rev-less Zed git
source. The comment at `Cargo.toml:30-32` records why. `xtask` enforces the
pins by scanning `Cargo.lock` for `github.com/zed-industries/zed#<40 hex>` and
`github.com/longbridge/gpui-component?rev=<40 hex>`
(`xtask/src/lib.rs:557-593`).

`gpui-base 0.7.1` instead declares:

```toml
gpui          = { package = "gpui-pre",          version = "=0.3.8" }
gpui_platform = { package = "gpui-pre-platform", version = "=0.3.8", ... }
gpui-fast     = { package = "gpui-fast", version = "0.1.1", optional = true, ... }  # feature "gpui-fast"
```

Cargo's `[patch]` only replaces a package with one of the same name. That
leaves four routes:

| Route | Manifest shape | One GPUI in the build? | Notes |
| --- | --- | --- | --- |
| **A. Status quo** | unchanged | yes | Frozen at gpui-base 0.5.2 and Zed 2026-08-17. Known pin limitations remain (below). |
| **B. Upstream snapshot** | `gpui = { package = "gpui-pre", version = "=0.3.8" }`, `gpui-base = "=0.7.1"` | yes | Same GPUI code as Zed `279fe07`, through Longbridge's snapshot. This is the route GPUI Kit itself defaults to. |
| **C1. gpui-fast by feature** | `gpui = { package = "gpui-fast", version = "=0.1.x" }`, `gpui-base = { version = "=0.7.1", features = ["gpui-fast"] }` | **no** | `gpui-base`'s `gpui-pre` dependency is not optional, so both GPUI copies compile. GPUI Kit's README says so: "upstream packages still compile when Fast is selected". This doubles the heaviest compile unit and puts two `GPUI_CRATES` families in the graph. |
| **C2. gpui-fast by patch** | Route B, plus `[patch.crates-io] gpui-pre* = { git = "https://github.com/longbridge/gpui-fast", rev = "…" }` | yes | gpui-fast's `compat/` crates carry the `gpui-pre` names at `=0.3.8` and re-export gpui-fast (`compat/README.md`). The code keeps importing `gpui::`, and reverting means deleting the patch. The patch must track whatever snapshot `gpui-base` pins, or Cargo silently ignores it with a warning. |
| D. Zed git + new gpui-base | patch `gpui-pre` to Zed git | — | Not possible without a fork: Zed's packages are named `gpui`, not `gpui-pre`. Rejected. |

Route B is the prerequisite for any `gpui-base` upgrade. Route C2 is the
low-risk way to trial gpui-fast on top of B. Both B and C replace git pins with
exact `=` crates.io versions, which are immutable and checksummed in
`Cargo.lock`. The C2 patch is the only git source left.

The pin-enforcement Module needs a matching change:

- `validate_git_lock` expects the two git sources and fails without them. It
  would instead assert exact `=` requirements and the locked versions.
- `deny.toml`'s `allow-git` list and its pin-tied advisory ignores move with
  the change.
- `GPUI_CRATES` must learn the `gpui-pre*` and `gpui-fast*` names, so the
  `rendering` isolation policy keeps holding.

## What retained mode would buy Ordo

### The mechanism, briefly

From `docs/retained-mode.md` and `docs/architecture.md` in gpui-fast:

- **Views are reused while nothing they read changed.** Every `Entity<V:
  Render>` or `AnyView` in the tree is a retained subtree. gpui-fast records
  what each one read: entities, globals, `ScrollHandle`/`ListState` versions,
  pointer position, modifiers, and hovers. While none of that changed, the
  view is not rendered, laid out, prepainted, or painted; its output is copied
  from the last frame. A notified view nested in an unchanged one is spliced
  into the parent's previous output.
- **Layout is kept between frames.** Taffy nodes survive across frames, and
  text measurements are carried over or re-checked against the previous
  constraints, so a changed number that keeps its size does not dirty the
  layout above it.
- **Scroll layers** keep scrolled content as cached GPU tiles. This is
  implemented for wgpu, Metal, and Direct3D 11. Lists (`uniform_list`, `list`)
  are designed for but switched off.
- **Correctness is defined against upstream.** A retained frame must equal the
  frame drawn from scratch. An oracle test drives two windows through random
  histories and compares whole scenes and hitboxes frame by frame.
  `GPUI_VIEW_RETENTION=0` and `GPUI_SCROLL_LAYERS=0` turn each mechanism off at
  runtime.
- **Nothing opts in.** "Code written for upstream GPUI compiles and runs
  unchanged."

### Against Ordo's budgets

Ordo's adopted budgets:

- a 50 ms p95 Presenter command latency;
- under 5% idle CPU;
- smooth scrolling of a ~800-Cue timeline;
- a 512 MiB qualification budget and a 1 GiB ceiling (decisions [0028](../decisions/0028-desktop-shell-supported-hardware-and-performance-budgets.md)
  and [0252](../decisions/0252-replace-electron-with-a-clean-slate-rust-and-gpui.md), `MILESTONES.md:2680-2686`).

No frame-time number is adopted. T0.6 is the first task that records frame
times, dropped frames, and memory (`MILESTONES.md:379-381`).

| Budget | Likely effect of gpui-fast | Confidence |
| --- | --- | --- |
| Command latency (keydown → Preview/Program paint) | Helps if a command notifies only the Preview or Program view while the timeline, library, and chrome stay unchanged. That depends on Ordo splitting the Presenter window into views; one root view rebuilt on every command gains little ("all sixty" panels changed: −29% only). The 50 ms budget is far above GPUI's typical 1–10 ms frame cost, so this is headroom, not rescue. | Medium; needs T0.6 numbers |
| Idle CPU | Small. Upstream GPUI already draws nothing when nothing is notified. The gain is in frames that *are* drawn, for example a toast countdown or an animation in one corner. | Medium |
| Timeline scroll (~800 Cues) | Scroll layers apply to `ScrollHandle` containers, which Ordo uses. They do not yet apply to `uniform_list`/`list`, which a virtualized 800-Cue timeline would likely need. | Low until the timeline exists |
| Memory (512 MiB / 1 GiB) | **Costs memory.** There is a retained record per subtree, persistent Taffy nodes, and **up to 64 MB of tile textures per window** with scroll layers. Ordo runs a Preparation window, a Presenter window, and a full-screen Program window. T0.6's stated goal is a "materially lower memory baseline". | High that it costs; size unmeasured |
| Accessibility | Retention is off in any frame where AccessKit is active (`fast/retained.rs:514`, `self.a11y.is_active()`). Screen-reader users get upstream behavior: correct, but without the speed-up. Windows UI Automation can also be activated by tools that are not screen readers, so measurements must record whether an AT client was connected. | High (read in source) |

### Hazards found in Ordo's current code

Ordo has no custom layout or paint code. Its only custom `Element` is a
pass-through (`ui/src/accessibility.rs:194-262`), so gpui-fast's API-compatibility
claim should hold at compile time. The risks are behavioral: state that
retention cannot see, and redraws that defeat it.

- **Scrollbar hover and grab state lives in `Rc<Cell<…>>` and redraws through
  `window.refresh()`** (`ui/src/scroll.rs:67-78`, `:224`, `:288-289`,
  `:325-326`, `:356`). Correctness holds, because gpui-fast retains nothing
  during a refresh. But every scrollbar hover change rebuilds the whole window
  from scratch, which is exactly the cost retention removes.
- **Each scrollbar render spawns a post-frame measurement check**
  (`scroll.rs:205-226`). Under retention, a reused view does not render, so
  that check is not scheduled for it. The `ScrollHandle` version bump should
  rebuild the view whenever the surface really moved, which makes this
  probably harmless. Verify it with a test, not by argument.
- **The overlay host keeps render-time memo state in `Cell`/`RefCell`**
  (`ui/src/overlay.rs:243`, `:265-272`), and its tests drive frames with
  `window.refresh()` (`overlay.rs:1643` onward). Toast entrance seeding and
  measured heights rely on render running on the frame after a measurement. A
  retained toast host that is not notified would keep its last frame.
- **The `gpui-base` text selection global** was re-registered on every frame
  until gpui-fast's authors fixed it upstream ("rebuilt every view that read
  that global on every frame", `docs/architecture.md`). Ordo's adopted `Input`
  must be re-qualified at whatever `gpui-base` revision is paired with
  gpui-fast.

None of these is a known bug. Each is a place where the qualification must
compare `GPUI_VIEW_RETENTION=0` with the default and expect identical results.

### What neither gpui-fast nor `gpui-pre` 0.3.8 fixes

These pin limitations appear in Ordo's code and qualification docs. All were
checked in gpui-fast's tree and in the `gpui-pre 0.3.8` crate (Zed
`279fe07`), and remain unchanged in both:

- The wgpu atlas uploads with an unpadded `bytes_per_row`
  (`gpui_wgpu/src/wgpu_atlas.rs:288`), so Ordo's 64-px row padding in
  `ordo/src/canonical_views.rs:20-41` stays.
- There is no AccessKit live-region setter, so `LiveElement` stays
  (`ui/src/accessibility.rs:191-193`).
- There is no high-contrast signal (`docs/qualification/GPUI-BASE-T0.3.md:907-927`).

## Seams and depth

In this skill's vocabulary:

- **GPUI is not behind an Ordo seam, by decision.** GPUI is the permanent
  shell (`MVP-PLAN.md:250`). It is imported directly by `ui` (~10.2k LOC in
  14 files) and `ordo` (~4.0k LOC in 8 files), with 84 `#[gpui::test]` tests
  on `TestAppContext`/`VisualTestContext`. A GPUI swap is a whole-workspace
  dependency change, not an Adapter swap.
- **That is acceptable here because Cargo provides the seam.**
  `gpui = { package = … }` and `[patch]` are a real seam with two Adapters
  (`gpui-pre` and gpui-fast) that satisfy the same Interface: upstream's API,
  enforced on gpui-fast's side by `script/check-upstream` and its "no new
  public API" rule. The runtime switch `GPUI_VIEW_RETENTION=0` gives a second,
  in-binary seam for A/B testing. Wrapping GPUI in an Ordo Interface to "make
  it swappable" would fail the deletion test: it would add a pass-through
  layer over ~14k LOC without hiding any variation that Cargo does not already
  hide.
- **`gpui-base` is the opposite, and well placed.**
  - It is used in six files inside `ui`, and xtask keeps it there
    (`validate_ui_dependency_policy`, `xtask/src/lib.rs:326-348`).
  - The Ordo wrappers (Button, Field, Select, Tabs, Disclosure, the overlay
    host) are deep Modules: callers never see `gpui_base` types.
  - An upgrade's cost is therefore local to those wrappers and their tests.
  - Every `gpui_base` item Ordo imports still exists by name in 0.7.1:
    `Button`, `Collapsible`, `Input`/`InputState`/`InputBase`/`InputEditorStyle`,
    `Tabs`/`Tab`, `Select`, `actions::{SelectUp, SelectDown}`,
    `POPUP_PRIORITY`, the `Toast*` types, `Sheet`, `animation::ease_*`,
    `motion::{Transition, transition}`, and `init`.
  - The signatures were not compiled against 0.7.1. Given +63.5k lines of
    churn, expect breakage inside the wrappers, not beyond them.

## Policy fit

Decision 252 ([0252](../decisions/0252-replace-electron-with-a-clean-slate-rust-and-gpui.md)) and `MVP-PLAN.md:252` already set the
bar for third-party GPUI libraries:

- an exact pin;
- license review;
- native accessibility and focus tests;
- a contained Adapter;
- upgrades as "scheduled compatibility work with qualification evidence, not
  an automatic move".

`GPUI-BASE-T0.3.md:15-17` adds: "Any revision change … invalidates this
qualification."

| Requirement | Route B (`gpui-pre` + `gpui-base 0.7`) | Route C2 (+ gpui-fast) |
| --- | --- | --- |
| Exact pin | `=0.3.8` / `=0.7.1`, checksummed | plus a git rev for the patch |
| License | Apache-2.0 | Apache-2.0 |
| A11y/focus re-qualification | Required: rerun the `GPUI-BASE-T0.3` table | Required, and again with retention on and off |
| Contained Adapter | Unchanged: `gpui-base` stays private to `ui` | GPUI is uncontained by design; the "Adapter" is the Cargo patch |
| Maturity | Snapshot of Zed; used by GPUI Kit's own releases | Experimental, 2 weeks old, issues disabled, ~3–4 maintainers |
| Upstream track | Weekly snapshots; GPUI Kit pins exact versions after a caret range broke users (gpui-kit #3156) | Syncs Zed every few days; hook-only discipline keeps merges cheap |

Two further points:

- **Single-vendor risk.** Routes B and C both move Ordo's GPUI supply from Zed
  directly to one crates.io account at Longbridge. The code is still Zed's,
  except for gpui-fast's `fast/` modules and the `gpui-pre` macro facade.
  Falling back to Zed git remains possible, but only with `gpui-base` frozen or
  forked.
- **No runtime internet, telemetry, or login is introduced** by either route.
  gpui-fast's native window composition (WebView embedding) and GPUI Kit's
  shell and webview crates are out of Ordo's scope and should stay out of the
  graph.

## Proposed sequencing

The owner has placed the upgrade before T0.5, as T0.4a.

1. **Now (T0.4 in progress):** no dependency change. Optionally, start the
   code habits in the next section, which help upstream `cached` views equally.
2. **T0.4a, the framework upgrade, immediately after T0.4 closes and before
   T0.5** (`MILESTONES.md`):
   - Move to Route B.
   - Re-qualify `gpui-base`.
   - Re-establish T0.4's evidence on the new stack.

   Before T0.5 was chosen over "between T0.5 and T0.6" for three reasons:
   - T0.5 builds Display Control and the congregation window on GPUI's
     window and platform APIs, which would otherwise be qualified on a stack
     that is about to change.
   - T0.4's Windows canonical-invariance evidence has to be re-run after any
     GPUI or rendering dependency change anyway (`MVP-PLAN.md:942`). Doing it
     once, right after T0.4, is cheapest.
   - T0.6 then measures the stack Ordo will ship.
3. **T0.5**, then **T0.6 (performance spike)**, on the upgraded stack. Build
   the frame-time, latency, and memory instrumentation as planned. Record the
   per-window frame CPU split (build, prepaint, paint) where GPUI exposes it.
4. **gpui-fast experiment, using T0.6's harness:**
   - Add the Route C2 patch on a branch.
   - Run the test suite twice, default and with `GPUI_VIEW_RETENTION=0
     GPUI_SCROLL_LAYERS=0`, and require identical results.
   - Run T0.6's harness both ways on the minimum-profile Windows hardware.
   - Record the gains alongside RSS, GPU allocations, and whether an AT client
     was active.
5. **Adoption gate:** adopt gpui-fast only if all of the following hold:
   - it improves a budgeted metric materially;
   - it stays inside 512 MiB;
   - it passes accessibility and focus re-qualification;
   - and the project has shown a few months of stable upstream syncs and
     releases.

   Record the outcome as a new record in `docs/decisions/`, so
   "retained" keeps one meaning in each document.

## Milestone task

The upgrade is recorded as **T0.4a — GPUI source migration and `gpui-base`
re-qualification** in `MILESTONES.md`. That document owns its goal and
acceptance criteria.

- **Dependencies.** T0.5 and T0.6 now depend on T0.4a, and T0.10's range names
  it explicitly.
- **ID.** The letter suffix avoids renumbering T0.5–T0.10 and every
  cross-reference to them.
- **Scope.** gpui-fast is deliberately excluded from T0.4a: its acceptance
  criteria require it to stay out of the graph. The experiment in step 4 above
  is not yet a milestone. It becomes one only after T0.6's harness exists.

## Writing Ordo for retention now

These habits are harmless on upstream GPUI, help its explicit
`Entity::cached` / `AnyView::cached` (present at Ordo's pin, `view.rs:39`,
`:232`), and are what gpui-fast needs to pay off:

- Keep state a view's render reads in entities or globals, and change it with
  `update` + `cx.notify()`. Avoid untracked `Rc<Cell<…>>` and `RefCell` for
  anything render reads. The scrollbar's `near`/`over`/`grab` cells are the
  first candidates.
- Prefer notifying the owning view to `window.refresh()` for local changes.
  `refresh` redraws the whole window from scratch on both upstream and
  gpui-fast. `scroll.rs` documents why it uses refresh after a frame (GPUI
  "refuses to dirty a window mid-draw"). That reason applies to the post-frame
  measurement, not to hover changes.
- Split the Presenter window into views along change frequency: Preview,
  Program, timeline, and chrome. The finer the views, the less each command
  costs under retention, and the easier T0.6's attribution becomes.
- Notify or write globals only when a value actually changes, and never in
  prepaint or paint.

## Open questions for review

1. Is a Longbridge-published `gpui-pre` acceptable as Ordo's GPUI source, given
   that it is the only way to keep upgrading `gpui-base`? The alternatives are
   freezing at 0.5.2 or vendoring the adopted behaviors into `ui`.
2. ~~Own task or folded into T0.6?~~ Resolved: its own task, T0.4a, before
   T0.5.
3. What maturity evidence would make gpui-fast acceptable: a 1.0, production
   use in Longbridge Pro, a stretch of releases without regressions, or
   upstreaming of its mechanisms into Zed? gpui-fast is designed so each
   mechanism "can be proposed to GPUI on its own".

## Evidence

- gpui-fast: `README.md`, `UPSTREAM`, `compat/README.md`,
  `docs/retained-mode.md`, `docs/architecture.md` (sections "Constraints",
  "Costs and limits", "Results"), `docs/upstream-sync.md`,
  `.github/workflows/ci.yml`, `crates/gpui/src/fast/retained.rs:226,514`,
  `crates/gpui_wgpu/src/wgpu_atlas.rs:288`.
- GPUI Kit: `Cargo.toml:63-81` (the `gpui-pre` pin rationale, optional
  gpui-fast), `crates/base/Cargo.toml`, `crates/base/src/lib.rs:7-10`,
  `README.md` "GPUI Fast", commits `87d10ae5` (`gpui-pre` 0.3.8), `7eb5209c`
  (optional gpui-fast backend, 2026-10-06), `e0f95161` (exact snapshot pins).
- `gpui-pre-macros 0.3.8`: `src/gpui_pre_facade_paths.rs`.
- crates.io: `gpui-pre`, `gpui-fast`, `gpui-base`, `gpui-kit`, and `gpui` version
  histories and owners.
- Zed: commit dates of `8b1497db`, `279fe07`, and `a1b71072e5`; `crates/gpui`
  commit count from 2026-08-17 to 2026-10-07; `crates/gpui/src/view.rs` at
  `8b1497db`.
- Ordo: `Cargo.toml:30-43`, `xtask/src/lib.rs:326-376,557-602`,
  decisions [0250](../decisions/0250-use-a-hybrid-global-rail-and-local-tab-navigation-hierarchy.md)–[0253](../decisions/0253-use-short-internal-crates-and-an-interactive-ui-laboratory.md), [0260](../decisions/0260-separate-human-song-authoring-from-durable-exchange-and.md)–[0261](../decisions/0261-derive-display-pixels-through-the-compositor-not-a-cpu.md), `MVP-PLAN.md:250-252`,
  `MILESTONES.md:356-413`, `docs/qualification/GPUI-BASE-T0.3.md`,
  `crates/ui/src/{scroll.rs,overlay.rs,accessibility.rs}`,
  `crates/ordo/src/canonical_views.rs`.
