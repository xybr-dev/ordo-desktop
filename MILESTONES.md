# Ordo — MVP Execution Milestones

Detailed implementation sequence for the product defined by
[`MVP-PLAN.md`](./MVP-PLAN.md). Authority is by subject: this document owns task
order, status, acceptance criteria, and delivery gates; `CONTEXT.md` owns domain
terms and invariants; `DESIGN.md` owns visible behavior and visual tokens; and
`MVP-PLAN.md` owns product scope and strategic architecture. `docs/decisions/`
records adopted detail and resolves gaps, but any conflict must be reconciled
into the applicable shorter document rather than selected ad hoc.

Developers still learning Rust or GPUI should use
[`LEARNING-ROADMAP.md`](./LEARNING-ROADMAP.md) as a learning-first runway into
Phase 0. It explains prerequisites and exercises but does not duplicate task
status or alter the milestone order.

**Current state:** Planning and visual acceptance references are complete. Rust
+ GPUI is permanently selected for the clean-slate implementation. T0.1 is the
active task; local uncommitted scaffolding is not completion evidence.

**MVP target:** A local-first Windows preparation application and air-gapped
Windows 10 church installation that can maintain parish content, prepare a
Mass, transfer it in a self-contained `.ordo` artifact, present it safely on a
second display, recover from interruption, and produce a validated emergency
PowerPoint deck.

---

## How to read this document

Every task contains:

- **Goal** — why the task exists and the observable outcome it unlocks.
- **Changes** — intended files or locality. The repository is not scaffolded
  yet, so exact filenames may move when a deeper Module gives better locality.
  Later `crates/portability/` and `crates/powerpoint/` paths are conditional
  candidates, not authorization to create those crates. At T4.1 and T5.4,
  respectively, either record that the Interface earns extraction or keep the
  Module in the closest existing production crate and update every affected
  later Changes path before implementation.
- **Depends on** — tasks that must be complete before this task begins.
- **AC** — objectively checkable acceptance criteria.
- **E2E** — at least one end-to-end scenario, tagged by how it runs.

### Verification tags

| Tag | Meaning |
|---|---|
| `[AUTO]` | Runs without human judgment in ordinary CI; includes Module-interface, contract, integration, golden-frame, and property tests. |
| `[UI-AUTO]` | Exercises native GPUI views through `TestAppContext`, an internal action/render harness, deterministic screenshots, or another no-human native UI driver. |
| `[NATIVE-AUTO]` | Runs a packaged artifact on a native Windows runner; no human judgment is required. |
| `[MANUAL]` | A scripted human check is required for OS behavior, accessibility, visual judgment, or application integration that automation cannot establish honestly. |
| `[PHYSICAL]` | Requires the actual church PC, display/projector path, room, and human operator/observer. |

An E2E scenario may carry several tags. `[AUTO]` tests should run on every
change when practical; `[NATIVE-AUTO]` belongs in native build jobs; `[MANUAL]`
and `[PHYSICAL]` results use an explicit script and retained evidence.

### Definition of done for every task

- [ ] All task AC are checked with linked test output or scripted evidence.
- [ ] New domain behavior is tested through its owning Module Interface, not
      through private classes or SQL details.
- [ ] Error results are structured and actionable; no domain or safety failure
      is represented only by a console message.
- [ ] No runtime path requires internet access, a CDN, telemetry, or login.
- [ ] User-visible wording uses the terms in `CONTEXT.md`.
- [ ] Any changed visible behavior is reconciled with `DESIGN.md` and the
      approved reference relevant to that workspace.
- [ ] `cargo xtask verify` passes, and applicable native/manual gates are recorded.

### Scope guard

The following do not enter these milestones: Song merge and UUID redirects,
cloud/LAN sync, login, multi-parish tenancy, arbitrary PowerPoint import,
videos, audio playback, remote control, public sharing, special-rite
qualification, shortcut customization, and general per-slide styling.

---

## Phase 0 — Qualified native GPUI foundation

An instrumented, packaged, offline native shell proving the platform risks
before domain implementation expands. **Ends with:** one operator window and
one passive congregation window displaying the same fixture-based canonical
frame, with measured latency/memory, deterministic typography across Interface scale, system scale, and output
density on Windows,
asynchronous SQLite-worker access, and native Windows evidence (Linux is
deferred post-MVP; Decision 268).

### T0.1 — Repository, toolchain, and verification harness

**Goal:** Make every later claim reproducible from one pinned toolchain and one
verification command.

**Changes:** `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `deny.toml`,
`.cargo/config.toml`,
`AGENTS.md`, `crates/domain/`, `crates/storage/`, `crates/rendering/`,
`crates/presentation/`, `crates/ui/`, `crates/platform/`, `crates/ordo/`,
`tools/ui-lab/`, `xtask/`, `tests/`, CI workflows

**Depends on:** None.

**AC**

- [x] `[AUTO]` Start from a clean repository with no imported source history,
      migrations, or vendored GPUI platform code.
- [x] `[AUTO]` Pin a Rust toolchain and commit `Cargo.lock`; GPUI,
      `gpui_platform`, and every git dependency use exact revisions rather than
      floating branches.
- [x] `[AUTO]` Scaffold the seven short-named internal crates and development-only
      `ui-lab` with dependency direction matching `MVP-PLAN.md`; `domain`
      imports no GPUI, SQLite, filesystem,
      or operating-system window types.
- [x] `[AUTO]` Root `AGENTS.md` points coding agents to `CONTEXT.md`,
      `DESIGN.md`, `MVP-PLAN.md`, and the active `MILESTONES.md` task and makes
      this repository's committed material authoritative.
- [x] `[AUTO]` `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace`, dependency/license checks,
      contract validation, and design-token validation each run a real check
      and fail on an injected error.
- [x] `[AUTO]` `cargo xtask verify` runs the complete ordinary verification set
      in a stable order and exits nonzero on the first failed required gate.
- [x] `[AUTO]` Module and integration tests use temporary directories/databases
      and never read or mutate a developer's Parish data.
- [x] `[UI-AUTO]` A GPUI `TestAppContext` or equivalent internal harness opens
      the root view, dispatches one action, verifies state/render output, and
      closes without leaked background work.
- [x] `[AUTO]` Runtime resources are declared in a committed, schema-validated
      offline manifest, and a source scan rejects unintended runtime networking.

**Evidence:** [successful T0.1 verification run](https://github.com/xybr-dev/ordo-desktop/actions/runs/33284275041).

**E2E** `[UI-AUTO]` — From a clean checkout, run `cargo xtask verify`, open the
root view through `TestAppContext`, assert build-identity and Offline render
output, dispatch one navigation action, and close cleanly.

### T0.2 — Native runtime, window ownership, and task supervision

**Goal:** Establish permanent native ownership and keep slow work away from the
GPUI thread before features create unsafe shortcuts.

**Changes:** `crates/ordo/`, `crates/platform/src/windows/`,
`crates/platform/src/tasks/`, `crates/presentation/`,
`tests/native-runtime/`

**Depends on:** T0.1.

**AC**

- [x] `[AUTO]` The GPUI application thread owns lifecycle, entities, focus,
      input, and window composition but performs no blocking database,
      filesystem, pagination, hashing, image-encoding, or export work.
- [x] `[AUTO]` Define typed task ownership and cancellation: application
      shutdown joins or cancels storage and background workers without detached
      mutations or process-wide hangs.
- [x] `[AUTO]` Domain Modules accept and return domain types without importing
      GPUI entities, window handles, executor types, or platform errors.
- [x] `[AUTO]` The passive congregation view receives only complete Program
      frames plus output status; it has no storage, diagnostics, planning,
      clipboard, file-dialog, or operator-command Interface.
- [x] `[AUTO]` Worker failures become structured typed results; sensitive local
      paths and diagnostics never enter the congregation state tree.
- [x] `[UI-AUTO]` Closing the operator window closes the congregation window,
      cancels background work, joins all owned workers, and exits cleanly.
- [x] `[AUTO]` Preparation window options request native/server operating-system
      decorations, and borderless full-screen window options use the pinned GPUI
      Interface without vendored platform code.

**E2E** `[UI-AUTO]` — Open operator and passive congregation
windows, start controlled background work, close the operator, and assert both
windows and all owned workers terminate. Verify the congregation surface never
acquires operator actions or focusable controls.

### T0.3 — Operator design system, UI lab, and hybrid application shell

**Goal:** Prove Ordo's global navigation, local tabs, overlays, focus behavior,
and responsive chrome before feature pages depend on them.

**Changes:** `crates/ui/src/primitives/`, `crates/ui/src/tokens/`,
`crates/ui/src/accessibility/`, `crates/ordo/src/shell/`,
`tools/ui-lab/`, `tests/ui/primitives/`, `tests/ui/shell/`

**Depends on:** T0.1, T0.2.

**AC**

- [x] `[AUTO]` Implement System-default, Light, and Dark Preparation appearance
      from `DESIGN.md`, including Atkinson operator typography, semantic colors,
      spacing, radii, borders, elevation, Lucide icons, and reduced-motion
      rules; Presenter always selects graphite dark.
- [x] `[MANUAL]` Changing the operating-system theme while Ordo runs updates the
      System appearance live. The pinned GPUI revision exposes no public way to
      simulate a window-appearance change, so only the resolution mapping and
      the state transition are automated; see
      `docs/qualification/GPUI-BASE-T0.3.md`.
- [x] `[UI-AUTO]` Global destinations appear in a vertical application rail:
      Masses, Library, Themes, and Transfer & Backup; Settings, Help, and
      Diagnostics are anchored at the bottom.
- [x] `[UI-AUTO]` Global destinations are navigation, not tabs. Horizontal tabs
      are reserved for local views such as Masses' Upcoming, History, Archive,
      and Templates.
- [x] `[UI-AUTO]` The rail uses icons plus persistent labels when space permits
      and collapses to accessible labeled icons with tooltips when effective
      width or Interface scale requires it.
- [x] `[UI-AUTO]` Settings and `Ctrl++`/`Ctrl+-`/`Ctrl+0` select the supported
      100%, 125%, 150%, 175%, and 200% Interface scales, persist the
      installation preference, and never reflow canonical Slide content.
- [x] `[UI-AUTO]` The active destination remains non-color-only and survives
      opening and returning from a focused editor.
- [x] `[AUTO]` Buttons, fields, menus, tooltips, selects, comboboxes,
      disclosures, tabs, dialogs, and nested slideovers are Ordo-owned GPUI
      primitives with typed state and one application overlay layer. A Select
      is a button trigger plus a popup list; a Combobox is an editable Field
      trigger plus a filtered popup list. They are separate primitives.
- [x] `[UI-AUTO]` Dialogs, ordinary slideovers, and unpinned Presenter Ordo use
      one input-capturing dim scrim with no click-through; bottom-center toasts
      distinguish expiring confirmation/Undo from owned persistent failures.
- [x] `[UI-AUTO]` Slideovers enter in 400ms and exit in 200ms through
      interruptible reduced-motion-aware tokens.
- [x] `[AUTO]` The internal `ui` crate uses a pinned GPUI-compatible
      `gpui-base` revision only behind Ordo-owned primitive Interfaces; the
      styled `gpui-component` layer is not a production dependency.
- [x] `[AUTO]` Dependency and license checks cover GPUI, `gpui-base`, and their
      complete transitive graph before any closed-source distribution decision.
- [x] `[UI-AUTO]` `ui-lab` exposes curated reusable scenarios for every adopted
      primitive across meaningful states, both appearances, supported Interface
      scales, 1366×768 and 1920×1080 presets, long multilingual content,
      constrained overlays, and keyboard/focus behavior.
- [x] `[AUTO]` `ui-lab` depends on `ui` and fixture helpers only, is excluded
      from parish installers, and contains no duplicate domain workflow or
      alternate implementation of the `ordo` application.
- [x] `[AUTO]` No production dependency on React, a WebView, CSS/Tailwind,
      shadcn, Base UI, or browser runtime enters the Cargo workspace.
- [x] `[AUTO]` Any additional third-party GPUI behavior library has its exact
      revision, license, accessibility behavior, focus behavior, and removal
      seam qualified before it becomes a dependency.
- [x] `[UI-AUTO]` Every overlay remains within the visible app viewport; its
      body scrolls internally while header and actions remain reachable.
- [x] `[UI-AUTO]` Every bounded scrolling surface shows an Ordo-owned scroll
      indicator whenever its content exceeds its viewport, from one shared
      implementation; the pinned GPUI paints none of its own, see
      `docs/qualification/GPUI-BASE-T0.3.md`.
- [x] `[UI-AUTO]` `Tab` follows visible reading order through GPUI focus handles;
      `F6`/`Shift+F6` cycle only visible registered regions and restore the last
      meaningful target.
- [x] `[UI-AUTO]` AccessKit nodes expose roles, labels, descriptions, values,
      and actions for the shell and primitive corpus without exposing
      operator-only nodes through the congregation window.
- [x] `[MANUAL]` Keyboard-only and Windows High Contrast checks preserve the
      two-layer focus treatment and distinguish focus from selection.

**E2E** `[UI-AUTO]` — Open `ui-lab`, exercise the field, select, combobox,
dialog, and nested-slideover scenarios using only the keyboard, change from
1920×1080/100% to 1366×768/200%, and verify state, focus return, and viewport containment.
Then launch `ordo`, navigate from the expanded Masses rail to Library/Songs and its
local views, and verify that the same primitive behavior holds in the shell
without whole-workspace horizontal scroll.

**Evidence:** `cargo xtask verify` 9/9 green (84 `ui` + 16 `ordo` tests),
including the milestone E2E pair
`ui::lab::tests::the_milestone_keyboard_path_holds_state_focus_and_containment_across_viewport_switch`
and
`ordo::shell::tests::the_milestone_shell_path_holds_primitive_behavior_without_workspace_scroll`;
manual scripts and per-AC evidence index in
`docs/qualification/T0.3-MANUAL-GATE.md`, with both `[MANUAL]` passes recorded
above.

### T0.4 — Canonical rendering and bundled-font isolation spike

**Goal:** Prove that one 1920×1080 logical composition remains identical across
operator sizes, operating systems, display scaling, and preview consumers.

**Changes:** `crates/rendering/`, `crates/rendering/src/fonts/`,
`crates/ui/src/canonical_frame.rs`, `tests/golden-frames/`,
`tests/fixtures/rendering/`

**Depends on:** T0.1, T0.2.

**AC**

- [x] `[AUTO]` The Canonical Slide Renderer Interface accepts resolved semantic
      content—including optional authored Response and Psalm header strings—a
      Theme, and target density, and returns either one complete frame plus
      diagnostics or one structured failure; partial frames are impossible and
      the renderer performs no language lookup.
- [x] `[AUTO]` Composition occurs in explicit 1920×1080 logical coordinates
      with the minimum 120×80 safe-area inset and never uses operator viewport
      metrics; a Theme may increase but not reduce that inset.
- [x] `[AUTO]` Bundle Atkinson Hyperlegible Next and Noto Sans with pinned hash
      manifests; rendering never falls back to installed system fonts.
- [x] `[AUTO]` Golden fixtures cover Cebuano, English, Tagalog, mixed scripts,
      punctuation, long words, missing-glyph fallback, one-to-four lyric lines,
      Celebration/Song identification, Scripture, Psalm, response, General
      text/image, black, label-off, and overflow-warning Slides; each
      multilingual Response/Psalm fixture supplies its exact authored header.
- [x] `[AUTO]` The canonical layout/shaping path is explicit and separate from
      ordinary GPUI operator text layout; it never asks a preview container to
      reflow canonical content.
- [x] `[NATIVE-AUTO]` Serialized canonical logical glyph positions, wrapping,
      pagination inputs, safe area, and element bounds are byte-for-byte
      invariant across packaged Windows runs, 100–200% Ordo Interface
      scale, representative system scale, and output densities; raster
      sharpness is qualified separately per output density. The Linux half is
      retired to T6.2 (Decision 268).
- [x] `[AUTO]` Target density may change raster dimensions and sharpness but not
      logical composition.
- [x] `[UI-AUTO]` Timeline thumbnail, Inspector, Preview, and Program wrappers
      display the same completed fixture frame at their production sizes
      across both viewport presets and every Interface scale, scaled uniformly
      with identical internal pixels and a 4px operator-only viewport radius.
- [x] `[MANUAL]` Atkinson operator fit and Noto fallback are legible and stable
      in packaged Windows builds at both qualified resolutions. The Linux
      half is retired to T6.2 (Decision 268).
- [x] `[MANUAL]` The fixture-backed Canonical Slide Layout Gallery qualifies
      every adopted template and freezes projected numeric type levels only
      after long Cebuano/English cases remain comfortably readable.

**E2E** `[UI-AUTO]` — Render one mixed-language fixture once, display it in the
four operator viewports at their production sizes plus the congregation
renderer, capture every surface, normalize only outer scaling, and assert
identical canonical content and unchanged pagination. Preview and Program share
one production size by design (Decision 266).

### T0.4a — GPUI source migration and `gpui-base` re-qualification

**Goal:** Move Ordo to a GPUI source that `gpui-base` can keep following, and
re-qualify every adopted behavior, before T0.5 and T0.6 build on and measure
the shell. Since `0.6`, `gpui-base` declares GPUI through exactly pinned
`gpui-pre` crates.io snapshots of Zed rather than Zed's git repository, so a
`gpui-base` upgrade is also a GPUI source migration. Rationale and rejected
routes: `docs/research/gpui-fast-and-gpui-kit.md`.

**Changes:** `Cargo.toml`, `Cargo.lock`, `deny.toml`, `xtask/src/lib.rs`,
`crates/ui/`, `crates/ordo/`, `docs/qualification/GPUI-BASE-T0.4a.md`

**Depends on:** T0.4.

**AC**

- [x] `[AUTO]` `gpui`, `gpui_platform`, and `gpui_macros` resolve to one
      exactly pinned (`=`) `gpui-pre` snapshot, and `gpui-base` resolves to the
      exact release built against that snapshot. No `zed-industries/zed` or
      `gpui-component` git source remains in `Cargo.lock`. `cargo xtask verify`
      fails on a non-exact GPUI or `gpui-base` requirement, a second GPUI crate
      family, or a git GPUI source.
- [x] `[AUTO]` `gpui-component`, `gpui-kit`, `gpui-shell`, `gpui-wry`, and
      `gpui-fast` stay absent from the graph; `gpui-base` stays private to
      `ui`, and no GPUI crate reaches `domain`, `presentation`, or `rendering`.
- [x] `[UI-AUTO]` Every behavior adopted in `GPUI-BASE-T0.3.md` is
      re-qualified at the new revision: its focus, keyboard, AccessKit role,
      name, and state tests pass unchanged, or each difference is recorded with
      its resolution.
- [x] `[AUTO]` Each documented pin gap is re-checked and recorded as closed or
      still open: the private test window, tooltip on keyboard focus, the
      high-contrast signal, AccessKit live regions, the wgpu row-padding
      workaround, the scroll indicator, and operating-system reduced motion. A
      closed gap removes its Ordo workaround in this task.
- [x] `[AUTO]` The qualification record names the new GPUI supply (the
      `gpui-pre` snapshot and the Zed commit it reproduces), its license, and
      the fallback if that supply stops.
- [x] `[UI-AUTO]` The T0.3 and T0.4 suites, including the canonical-slide lab
      test, pass on the new stack.
- [x] `[NATIVE-AUTO]` The Windows shaping probe and canonical-invariance job
      pass on the new stack.
- [x] `[MANUAL]` The `T0.3-MANUAL-GATE.md` focus, theme-following, and
      screen-reader checks are repeated on Windows and recorded.

**E2E** `[AUTO]` `[NATIVE-AUTO]` — From a clean checkout, `cargo xtask verify`
passes with `--locked` on the new pins, and the native Windows jobs reproduce
T0.4's canonical invariance evidence unchanged.

### T0.4b — Windows-only development rescope of T0.4 Linux evidence

**Goal:** Close T0.4 honestly after development moved to Windows-only
(Decision 268). The packaged-Linux manual script and the Linux half of T0.4's
native invariance criterion were not run; this task rescopes them so T0.4's
remaining gates name evidence that is actually produced, and every deferred
Linux check has one named owning task rather than silently disappearing.

**Changes:** `MILESTONES.md`, `MVP-PLAN.md`, `decision.md`,
`docs/qualification/T0.4-MANUAL-GATE.md`,
`docs/qualification/RENDERING-T0.4.md`, `.github/workflows/verify.yml`

**Depends on:** T0.4, T0.4a.

**AC**

- [x] `[AUTO]` A decision-ledger entry records the Windows-only development
      scope: which platform produces Phase 0 native and manual evidence, and
      whether Linux remains an MVP preparation/packaging target or is deferred.
      `MILESTONES.md` and `MVP-PLAN.md` are reconciled to that entry, with no
      conflicting Linux claim left in either.
- [x] `[AUTO]` T0.4's `[NATIVE-AUTO]` invariance criterion and `[MANUAL]`
      operator-fit criterion name only platforms that produce evidence during
      development; each removed Linux check moves to one named task (the
      retired T6.2 when Linux is deferred) with its original wording
      preserved.
- [x] `[AUTO]` `T0.4-MANUAL-GATE.md` script B is rewritten or retired to match
      the rescope and links the task that now holds packaged-Linux legibility.
- [x] `[NATIVE-AUTO]` The Windows shaping probe and canonical-invariance jobs
      remain mandatory in CI; any Linux native job is either kept passing or
      removed with its coverage recorded against the owning task.
- [x] `[AUTO]` Golden frames, serialized canonical layouts, and bundled-font
      manifests are unchanged; this task changes scope, not rendering.
- [x] `[MANUAL]` The owner confirms the rescope, after which T0.4's remaining
      criteria are checked off against the recorded Windows evidence.

**E2E** `[AUTO]` — From a clean checkout, `cargo xtask verify` passes with
`--locked`, and a search of the normative documents finds no T0.4 gate that
requires evidence no development platform produces.

### T0.5 — Display Control and passive congregation-window spike

**Goal:** Prove safe two-display orchestration without coupling Presentation
state to GPUI or operating-system window details.

**Changes:** `crates/platform/src/display/`,
`crates/platform/src/congregation_window.rs`, `crates/presentation/`,
`crates/ordo/src/output.rs`, `tests/display/`,
`tests/fixtures/display-topologies/`, `tests/native-runtime/`, `xtask/`,
`.github/workflows/verify.yml`

**Depends on:** T0.2, T0.4, T0.4a.

**AC**

- [x] `[AUTO]` Define a small Display Control Interface for discovery,
      identification, assignment, output lifecycle, topology, and status; an
      in-memory Adapter drives deterministic tests and GPUI/native platform
      code is the production Adapter.
- [x] `[UI-AUTO]` The application starts with no congregation-output window.
- [x] `[UI-AUTO]` Explicit output enablement creates exactly one passive,
      borderless congregation window on the assigned display and shows black
      before any content.
- [x] `[UI-AUTO]` The congregation window cannot take intentional operator
      focus, receive operator controls, or expose operator-only context.
- [x] `[AUTO]` Display disappearance enters a disconnected result without
      silently selecting another display; reconnecting does not expose content
      until the documented restore path completes.
- [x] `[AUTO]` Topology records relative direction from any operator-display
      arrangement rather than assuming the congregation display is rightward.
- [x] `[MANUAL]` Pointer hiding over Program and operator cursor-location
      assistance work for displays positioned left, right, above, and below.
- [x] `[NATIVE-AUTO]` Packaged builds can enumerate the native runner display
      and survive a synthetic topology refresh without a crash.

**E2E** `[UI-AUTO]` — With a fake two-display Adapter, enable output, verify
black-first Program, take a fixture frame live, disconnect the assigned display,
assert protective Disconnected state, reconnect it under a new OS ID with
matching evidence, and assert Program remains black until explicitly restored.

### T0.6 — Presentation Engine and performance instrumentation spike

**Goal:** Establish the deterministic hot path and prove native GPUI can meet
the adopted command-latency while establishing a materially lower memory
baseline before full UI construction.

**Changes:** `crates/presentation/`,
`crates/ordo/src/performance/`, `xtask/src/measure_presenter.rs`,
`tests/presentation/`, `tests/performance/`

**Depends on:** T0.2, T0.4, T0.4a, T0.5.

**AC**

- [x] `[AUTO]` A pure Presentation Engine accepts commands and returns complete
      state without importing GPUI, SQLite, filesystem, or platform modules.
- [x] `[AUTO]` Instrument keydown→Preview paint and keydown→Program paint at
      p50, p95, and p99 for navigation, Freeze, Blackout, and Space.
- [ ] `[AUTO]` The measured hot path contains no SQLite/filesystem access,
      autosave, pagination, image decode, blocking worker wait, or full-timeline
      rerender.
- [x] `[AUTO]` Rapid command sequences are ordered, coalesced only where
      semantically safe, and never lose the final valid operator intent.
- [x] `[AUTO]` Record process RSS/private memory, GPU allocations where the
      platform exposes them, frame times, dropped frames, startup time, worker
      queue latency, and idle CPU.
- [x] `[AUTO]` Full-resolution frame cache size is bounded; thumbnails are
      genuinely lower-resolution, virtualized, and released when obsolete.
- [x] `[AUTO]` Compare the two candidate display paths for the same canonical
      frame and record latency and held bytes for each: the CPU raster path
      (`rendering::rasterize` composited to RGBA, encoded, decoded, uploaded)
      and a retained path that walks the logical `Frame` straight into the
      GPUI scene through its already-shaped glyph, quad, and image painting,
      letting the GPUI sprite atlas cache glyph masks. Every surveyed
      presentation engine draws its live output from a retained scene and
      rasterizes only for thumbnails, transitions, capture, and export; this
      measurement decides whether Ordo does the same. Record the per-slide
      cost of each path separately, because it sets T1.3's scope.
- [x] `[AUTO]` If the retained path is measured, confirm the GPUI text system
      resolves the bundled Atkinson Hyperlegible Next and Noto Sans faces the
      canonical shaper uses, rather than a same-named system installation, on
      both packaged targets (Linux retired, Decision 268). Shaped glyph IDs
      are face-intrinsic, so a silent substitution would mispaint without
      failing.
- [ ] `[NATIVE-AUTO]` On minimum-profile Windows test hardware, p95
      Presenter commands are below 50ms and total memory never exceeds the
      1 GiB absolute release ceiling. The Linux half is retired to T6.2
      (Decision 268).
- [ ] `[NATIVE-AUTO]` Record empty-shell, large-library Preparation, and
      representative 500-Cue Presenter memory baselines across three runs.
      Every run stays within the 512 MiB qualification budget and the 1 GiB
      absolute ceiling; exceeding either blocks Phase 1 pending an explicit
      budget and native-memory rationale revision.
- [x] `[MANUAL]` Hardware acceleration is active by default; any software-
      rendering fallback is diagnosed and recorded rather than silently chosen.

**E2E** `[NATIVE-AUTO]` — Run a packaged 500-Cue fixture through 2,000 mixed
navigation/state commands, collect latency and process-memory reports, assert
the final state against a pure Engine oracle, and fail on the adopted p95 or
hard-memory ceiling.

### T0.7 — SQLite owner worker and migration runner spike

**Goal:** Prove safe asynchronous local persistence without placing database
work on the Presenter hot path.

**Changes:** `crates/storage/`, `crates/storage/src/worker.rs`,
`crates/storage/src/migrations/`, `migrations/`, `tests/storage/`

**Depends on:** T0.1, T0.2.

**AC**

- [x] `[AUTO]` One supervised Rust worker thread is the sole owner of the pinned
      bundled `rusqlite` connection; no GPUI entity or feature view can acquire
      a raw connection.
- [x] `[AUTO]` The Storage Module exposes domain-shaped asynchronous operations;
      callers cannot submit SQL or control transactions.
- [x] `[AUTO]` A numbered migration runner applies migrations in order inside
      transactions and records their identifiers and checksums.
- [x] `[AUTO]` Re-running at head is a no-op; a failed migration rolls back both
      schema changes and migration history.
- [x] `[AUTO]` Foreign keys, WAL/recovery policy, busy handling, and orderly
      shutdown are explicit and fixture-tested.
- [x] `[AUTO]` Concurrent typed requests are serialized or coordinated
      deterministically without blocking the GPUI thread or Presenter hot path.
- [x] `[AUTO]` An unknown newer schema is refused without mutation.
- [x] `[AUTO]` A worker panic, closed request channel, and application shutdown
      each yield a supervised structured result and do not crash or hang the UI
      test harness.

**E2E** `[AUTO]` — Open a fresh temporary database through the Storage Module,
run migrations to head, run again as a no-op, inject a broken next migration,
and assert the schema, data, and migration ledger remain at the last valid
version.

### T0.8 — Versioned contracts and universal `.ordo` container spike

**Goal:** Freeze the portable contract shape before library, snapshot, and
transfer data begin depending on it.

**Changes:** `contracts/`, `tools/artifact-container-spike/`,
`tests/fixtures/contracts/`, `tests/artifact-container/`, `docs/contracts/`

**Depends on:** T0.1.

**AC**

- [x] `[AUTO]` Publish versioned JSON Schemas for Song import, Liturgy import,
      snapshot manifest, Celebration Package, Parish Backup, diagnostic report,
      and recovery journal.
- [x] `[AUTO]` Each schema has minimal, representative, boundary, invalid,
      unknown-version, missing-reference, and tampered-hash fixtures.
- [x] `[AUTO]` End-user exchange uses `.ordo` ZIP containers with required
      `manifest.json`, structured JSON payload, optional `assets/`, explicit
      artifact kind, schema version, stable IDs, and hashes.
- [x] `[AUTO]` JSON payloads remain human-readable and directly usable as AI
      extraction contracts and test fixtures.
- [x] `[AUTO]` Inspection enforces archive-size, entry-count, decompression,
      traversal, duplicate-path, link, manifest, and checksum limits before any
      Parish mutation.
- [x] `[AUTO]` Unknown kinds or future versions fail with a compatible-version
      message rather than being guessed; Invalid is distinct from Needs decision.
- [x] `[AUTO]` Creation writes to a temporary sibling, reopens and verifies it,
      then publishes atomically or recoverably without destroying an existing
      destination.

**E2E** `[AUTO]` — Create each artifact kind from its smallest valid fixture,
reopen and route it by manifest kind, then run a corpus of malformed/traversal/
tampered archives and assert no file escapes staging and no Parish mutation
occurs.

### T0.9 — Emergency PowerPoint feasibility spike

**Goal:** Prove that the chosen library can generate the exact static fallback
contract and that the church PowerPoint can reopen it.

**Changes:** `tools/powerpoint-spike/`, `tests/powerpoint/`,
`tests/fixtures/powerpoint/`, `xtask/src/inspect_pptx.rs`

**Depends on:** T0.4.

**AC**

- [ ] `[AUTO]` Generate full-bleed 16:9 image slides without editable text
      substitution or recomposition.
- [ ] `[AUTO]` Create actual PowerPoint sections, black lead-in/closing Safety
      sections, and speaker notes on operator-only black slides.
- [ ] `[AUTO]` Reopen the produced archive and verify slide order, section
      membership, image dimensions/hashes, notes, and safety slides.
- [ ] `[AUTO]` Failed generation or validation leaves the existing destination
      untouched and no apparently successful file behind.
- [ ] `[MANUAL]` The generated fixture opens and presents correctly in the exact
      PowerPoint version installed on the Windows 10 church PC.

**E2E** `[AUTO]` `[MANUAL]` — Export a fixture containing identification,
lyrics, response, reading, Hold, and Blackout Cues; automatically inspect the
deck, then present it in church PowerPoint and confirm image fidelity, black
operator-only slides, notes, and sections.

### T0.10 — Qualification gate and architecture record

**Goal:** Prevent feature implementation from proceeding on an unqualified
platform assumption or an undocumented exception.

**Changes:** `docs/qualification/milestone-0/`, `MILESTONES.md`,
`MVP-PLAN.md`, architecture notes only when a genuine trade-off warrants them

**Depends on:** T0.1–T0.9, including T0.4a.

**AC**

- [ ] `[AUTO]` All Phase 0 automated and native reports identify the exact
      commit, Rust toolchain, GPUI/platform/dependency revisions, OS, GPU, and
      fixture hashes.
- [ ] `[MANUAL]` Windows 10 22H2 packaged checks are signed off with failures
      and reruns retained. Supported-Linux packaged sign-off is retired to
      T6.2 (Decision 268).
- [ ] `[AUTO]` Any failed proof is resolved inside the responsible GPUI/native
      Adapter or Module Implementation and requalified; domain Interfaces
      remain independent of the workaround.
- [ ] `[MANUAL]` If minimum hardware cannot meet a hard gate after profiling,
      release is blocked or the supported hardware profile is explicitly
      revised; failures are not hidden by adding a second production shell.
- [ ] `[AUTO]` Phase 0 fixture and instrumentation code remains reusable in later
      regression tests rather than becoming an abandoned spike directory;
      reusable fixtures, contract cases, inspectors, and instrumentation remain
      outside `tools/*-spike`, while disposable generator prototypes are never
      imported by production crates.

**E2E** `[NATIVE-AUTO]` `[MANUAL]` — From one clean commit, build both native
packages, launch the operator and congregation walking skeleton, exercise task
supervision, storage, canonical rendering, navigation, display loss, and
performance scripts, and produce one indexed qualification bundle with no
missing result.

---

## Phase 1 — Safe Presenter walking skeleton

Offline and fixture-driven; no parish database or editor. **Ends with:** a
representative Mass fixture can be presented safely for a complete simulated
session, with deterministic state, responsive Live Control, unavailable-Cue
protection, and fail-closed recovery.

### T1.1 — Presentation snapshot fixture and Cue model

**Goal:** Give the Presenter one immutable, realistic input contract independent
of planning and persistence.

**Changes:** `crates/domain/src/presentation.rs`, `crates/presentation/src/snapshot_reader.rs`,
`tests/fixtures/presentations/normal-mass/`,
`tests/fixtures/presentations/recovery-drill/`

**Depends on:** T0.4, T0.8.

**AC**

- [ ] `[AUTO]` Build sanitized Normal Mass and Recovery Drill fixtures with the
      six Mass sections and complete stable IDs/hashes.
- [ ] `[AUTO]` Cover projectable lyrics, identification, responses, readings,
      timed General Presentations, operator-only triggers, Wait, Hold, Blackout,
      long wrapping, repeated chorus/refrain occurrences, both Themes, and an
      unavailable Cue.
- [ ] `[AUTO]` Every Cue belongs to exactly one Mass item; Mass sections own no
      Cue and no projectable output.
- [ ] `[AUTO]` Operator-only triggers and Hold provide black projectable output
      while retaining private operator context; Wait Cues retain private context
      and deliberately leave Program unchanged.
- [ ] `[AUTO]` Snapshot loading validates every required frame/dependency before
      exposing a usable presentation input.

**E2E** `[AUTO]` — Load both fixtures through the snapshot reader, flatten their
Mass-item/Cue order, assert stable expected positions and hashes, and reject a
copy with one missing or mismatched required dependency.

### T1.2 — Complete Presentation Engine state model

**Goal:** Implement every adopted live-state transition once, behind the Engine
Interface used by UI and tests.

**Changes:** `crates/presentation/src/engine.rs`, `crates/presentation/src/commands.rs`,
`crates/presentation/src/state.rs`, `tests/presentation/state-model.*`

**Depends on:** T0.6, T1.1.

**AC**

- [ ] `[AUTO]` Implement output disabled, enabled-black, Live, Frozen, Blackout,
      and Disconnected states exactly as the MVP state table defines.
- [ ] `[AUTO]` Left/Right and Up/Down traverse Cues; Page Up/Down traverse Mass
      items; Home/End go to first/last Cue.
- [ ] `[AUTO]` Live navigation links Preview and Program; Frozen and Blackout
      navigation changes Preview only; Space takes valid Preview live.
- [ ] `[AUTO]` `F` toggles Freeze, `B` activates Blackout, and unchanged/repeated
      commands are deterministic.
- [ ] `[AUTO]` Content-to-content changes cut; content-to/from Blackout or Hold
      exposes interruptible reversible 500ms fade progress without queueing
      commands or delaying the underlying state transition.
- [ ] `[AUTO]` Explicit item/section auto-play advances eligible General
      Presentation Cues from inherited eight-second/per-Slide durations, wraps
      inside scope, resets each newly shown Cue's timer, and stops on outside
      navigation, Freeze, Blackout, exit, or recovery.
- [ ] `[AUTO]` Selecting an operator-only Cue while Live sends black, not private
      content, to Program.
- [ ] `[AUTO]` An unavailable future Cue preserves the last valid Program,
      enters protective Freeze when necessary, remains navigable, and cannot be
      taken live.
- [ ] `[AUTO]` Property tests generate long valid command sequences and assert
      Program is always black, disconnected, or one complete valid frame.

**E2E** `[AUTO]` — Replay the Recovery Drill's canonical command script through
the Engine, assert state after every command, inject a missing future frame, and
verify no command sequence exposes an error frame or operator-only content.

### T1.3 — Canonical frame scheduling, caches, and failure propagation

**Goal:** Deliver frames fast without letting preloading, failure, or memory
pressure weaken Program safety.

**Scope is conditional on T0.6's display-path measurement.** This task was
written assuming every viewport consumes a full-resolution CPU raster, which
makes that raster the expensive artifact worth scheduling, bounding, and
evicting. If T0.6 adopts the retained path, no such artifact exists on the
display path: the sprite atlas caches glyph masks across every slide and
viewport, and the remaining caches are decoded foreground images by asset hash
and logical `Frame` values by frame identity, neither of which needs a byte
budget or a preload window. The raster and thumbnail cache criteria below then
drop, and the failure-propagation and cancellation criteria stay, since they
are independent of how pixels are produced. Reconcile this task against the
T0.6 result before starting it; do not implement both designs.

**Changes:** `crates/rendering/src/frame_scheduler.rs`, `crates/rendering/src/frame_cache.rs`,
`crates/rendering/src/thumbnail_cache.rs`, `tests/rendering/cache.*`

**Depends on:** T0.4, T0.6, T1.1.

**AC**

- [ ] `[AUTO]` Program and Preview frames are pinned while in use; nearby-Cue
      preloading has a measured fixed bound.
- [ ] `[AUTO]` Thumbnail generation uses a separate low-resolution cache and
      never retains a full-size frame solely for an offscreen timeline item.
- [ ] `[AUTO]` Cache eviction releases decoded resources and never evicts the
      active Program frame.
- [ ] `[AUTO]` Obsolete render requests are cancelled or ignored without
      replacing a newer Preview.
- [ ] `[AUTO]` Decode/render failure becomes the same structured Unavailable Cue
      result consumed by the Engine and Preflight.
- [ ] `[AUTO]` A 500-Cue fixture remains within the Phase 0 measured memory
      envelope during forward, reverse, and random navigation.

**E2E** `[AUTO]` — Navigate rapidly through 500 mixed Cues while injecting slow
and failed frame jobs; assert latest-selection Preview, stable valid Program,
bounded decoded resources, and no visible partial frame.

### T1.4 — Responsive three-column Live Control

**Goal:** Make the complete Presenter model operable with the approved hierarchy
at every qualified effective size.

**Changes:** `crates/ordo/src/presenter/`, `crates/ordo/src/commands/`,
`tests/e2e/presenter/`, approved `design-references/`

**Depends on:** T0.3, T1.2, T1.3.

**AC**

- [ ] `[UI-AUTO]` Wide layout shows pinned Ordo rail, grouped Cue timeline, and
      Output Dock with Program and Preview.
- [ ] `[UI-AUTO]` Compact layout starts with collapsed Ordo rail and retains
      timeline plus stacked Program/Preview; high-scale layout keeps both
      monitors simultaneously visible above the timeline when required.
- [ ] `[UI-AUTO]` Visual Slide Grid and Compact Cue List preferences persist
      locally and never change Presentation Engine state.
- [ ] `[UI-AUTO]` Grid thumbnails stay at least 300px where the preferred layout
      can remove a column; sparse rows never stretch frames.
- [ ] `[UI-AUTO]` Ordo groups display Mass-item titles only and scroll the full
      timeline to a group rather than filtering it.
- [ ] `[UI-AUTO]` Output Dock keeps Program, Preview, output status, Freeze,
      Blackout, and contextual primary action visible outside timeline scroll.
- [ ] `[UI-AUTO]` Canonical thumbnails and monitors remain identical completed
      frame viewports through resize and Interface-scale changes.
- [ ] `[MANUAL]` The packaged UI matches the approved Presenter references in
      hierarchy, state clarity, focus visibility, and dim-room usability.

**E2E** `[UI-AUTO]` — Drive the same active Cue/state through 1920×1080,
1366×768, and 1366×768/200% layouts; switch timeline mode and rail state; assert
no Engine mutation, no missing high-stakes control, and identical frames.

### T1.5 — Presenter commands, focus, and shortcut suspension

**Goal:** Make keyboard operation fast while ensuring text entry and overlays
cannot accidentally change Program.

**Changes:** `crates/ordo/src/commands/registry.rs`,
`crates/ordo/src/focus_regions/`, `crates/ordo/src/shortcut_reference/`,
`tests/e2e/presenter/keyboard.*`

**Depends on:** T0.3, T1.4.

**AC**

- [ ] `[AUTO]` One command registry is the source for registered keys and the
      visible Keyboard shortcuts reference.
- [ ] `[UI-AUTO]` `O` opens/focuses the Ordo rail without changing pin state;
      `Esc` closes only the topmost temporary surface and restores the active
      Cue focus.
- [ ] `[UI-AUTO]` `A` toggles eligible item auto-play; a visible menu owns Loop
      this section, while ineligible liturgical content explains refusal.
- [ ] `[UI-AUTO]` `F6` cycles Ordo rail, Cue timeline, and Output controls while
      skipping a closed unpinned rail and never changing live state.
- [ ] `[UI-AUTO]` Passive Ordo-rail and shortcut-reference focus leave only
      `F`, `B`, and Space active. `A` is active only while focus is in the Cue
      timeline or Output Dock and no text editor, emergency editor, or true
      modal owns focus.
- [ ] `[UI-AUTO]` A focused text field, emergency editor, or true modal suspends
      every single-key Presenter command until closed.
- [ ] `[UI-AUTO]` Cue focus names include position, meaningful text/type, and
      project/wait/hold/Blackout behavior without exposing private context to
      the congregation renderer.
- [ ] `[MANUAL]` Keyboard-only and Windows screen-reader checks announce expected
      transitions politely and unexpected safety events urgently without
      duplicate queues.

**E2E** `[UI-AUTO]` — Focus every Presenter region and overlay, exercise all
registered keys, type the letters `fbo` and spaces into a text field, and assert
only commands valid for the current focus context reach the Engine.

### T1.6 — Output lifecycle and safe Presenter entry/exit shell

**Goal:** Integrate the Presenter with the native GPUI window lifecycle while
keeping congregation exposure deliberate and fail-closed.

**Changes:** `crates/platform/src/presenter_mode.rs`, `crates/platform/src/display/output_controller.rs`,
`crates/ordo/src/presenter/session_header/`, `tests/e2e/output-lifecycle.*`

**Depends on:** T0.5, T1.4, T1.5.

**AC**

- [ ] `[UI-AUTO]` Presenter Mode transforms the existing main window into
      native-full-screen Live Control; no second operator window remains.
- [ ] `[UI-AUTO]` Entry begins with congregation output disabled and no output
      window; the operator can rehearse fully in this state.
- [ ] `[UI-AUTO]` Enabling output requires an assigned available display and
      creates black Program before Space can take Preview live.
- [ ] `[UI-AUTO]` Exit is direct while output is disabled; while enabled it
      requires the adopted confirmation, forces Program black, closes output,
      and restores the same main window and preparation context.
- [ ] `[UI-AUTO]` Display disconnection updates visible state and Engine state
      without reassigning output or showing stale confidence.
- [ ] `[MANUAL]` Native fullscreen, Alt+Tab behavior, window focus, cursor hiding,
      and recovery work in packaged Windows builds.

**E2E** `[UI-AUTO]` — Enter Presenter from a fixture host, rehearse with no
output window, enable fake output, take a Cue live, attempt exit and cancel,
confirm exit, and verify black-before-close plus restoration of the exact host
selection and scroll state.

### T1.7 — Fixture-session journal and fail-closed restart recovery

**Goal:** Prove interrupted-session recovery before live edits or database-backed
plans make the state larger.

**Changes:** `crates/presentation/src/journal/`, `crates/ordo/src/recovery.rs`,
`tests/presentation/recovery.*`, `tests/e2e/recovery.*`

**Depends on:** T0.8, T1.2, T1.6.

**AC**

- [ ] `[AUTO]` Journal schema records snapshot identity, Preview, last valid
      Program context, state intent, output evidence, and monotonic sequence.
- [ ] `[AUTO]` Writes are asynchronous, ordered, compact, and absent from the
      keydown-to-paint path.
- [ ] `[AUTO]` Torn, stale, unknown-version, or mismatched-snapshot journals are
      rejected without guessing.
- [ ] `[UI-AUTO]` Unexpected restart reopens directly into recovered Presenter
      with output black and a persistent recovery strip, not a blocking dialog.
- [ ] `[UI-AUTO]` Preview and navigation intent return; prior Program is shown
      only as private context and is never claimed as external output.
- [ ] `[UI-AUTO]` Space on a valid recovered Preview takes it live and clears
      the recovery strip; ordinary safe exit remains available.

**E2E** `[UI-AUTO]` — Terminate the app after a journaled Live, Frozen, and
Blackout point, restart each case, and assert identical restored operator
context with black external Program until one explicit valid Space command.

### Phase 1 gate

- [ ] `[AUTO]` State-model, golden-render, cache, accessibility, responsive, and
      recovery suites pass without SQLite or editable library UI.
- [ ] `[NATIVE-AUTO]` A full Recovery Drill fixture run passes in packaged
      Windows builds within latency and memory budgets.
- [ ] `[MANUAL]` A human can run the sanitized Normal Mass fixture from keyboard
      alone without exposing private content or losing state clarity.

---

## Phase 2 — Durable parish Song Library

Offline local authoring. **Ends with:** Songs and Arrangements can be created,
searched, edited, sequenced, rendered, versioned, imported, archived, restored,
and safely retained through reload—without Mass Planning yet.

### T2.1 — Parish schema, managed assets, and domain persistence

**Goal:** Turn the Storage spike into the durable parish foundation used by
every authoring Module.

**Changes:** `crates/storage/src/migrations/`, `crates/storage/src/parish_store/`,
`crates/storage/src/assets/`, `crates/domain/src/identity.rs`, `tests/storage/parish.*`

**Depends on:** T0.7, T0.8.

**AC**

- [ ] `[AUTO]` Store canonical lowercase UUIDv4 identities and explicit creation
      times; titles never serve as identity.
- [ ] `[AUTO]` Initial migrations cover Parish, settings, Choir, Song,
      Arrangement, section, occurrence, Theme placeholders, revision ledger,
      managed asset, snapshot/package/backup placeholders, and schema metadata.
- [ ] `[AUTO]` Managed assets use stable IDs and hashes and commit consistently
      with database references; no committed record can reference a missing
      asset.
- [ ] `[AUTO]` Cache files are clearly separate from authoritative assets and
      can be deleted and rebuilt without content loss.
- [ ] `[AUTO]` Every write uses parameterized SQL, transactions where invariants
      cross records, and enforced foreign keys.
- [ ] `[AUTO]` Storage errors preserve prior committed state and return a stable
      domain-relevant failure.
- [ ] `[AUTO]` `ERD.md` is generated/maintained with these authoritative
      migrations: its Mermaid physical overview and per-table columns, types,
      null/default/constraint/index/FK/ownership/lifecycle details correspond
      directly to the schema rather than duplicating domain prose.

**E2E** `[AUTO]` — Create a temporary Parish with Choir, Song, Arrangement,
sections, occurrences, and an image asset through Module Interfaces; restart the
storage worker and assert exact rehydration, then inject an asset-write failure
and verify atomic rollback.

### T2.2 — Edit revisions, autosave sessions, and immediate Undo

**Goal:** Establish one reusable edit-safety mechanism before authoring screens
multiply.

**Changes:** `crates/storage/src/revisions/`, `crates/ordo/src/edit_session/`,
`crates/domain/src/revisions.rs`, `tests/revisions/`

**Depends on:** T2.1.

**AC**

- [ ] `[AUTO]` Every committed edit point creates a durable content-hashed Edit
      revision linked to its parent; restoration creates a later revision.
- [ ] `[AUTO]` Continuous text edits coalesce after one second of inactivity or
      focus departure; structural add/remove/reorder and semantic replacement
      are discrete points.
- [ ] `[AUTO]` Immediate Undo/Redo is scoped to the currently open entity and is
      cleared on entity departure or restart.
- [ ] `[UI-AUTO]` Focused text fields consume native text Undo before committed
      edit_session Undo.
- [ ] `[AUTO]` History groups revisions from one entity editing session and ends
      on entity navigation, close, or 30 minutes without committed changes.
- [ ] `[UI-AUTO]` Quiet Saving…, Saved, and Couldn't save states are accurate;
      failed pending edits are retained and block silent navigation loss.

**E2E** `[UI-AUTO]` — Type several fields, reorder twice, wait for autosave,
Undo/Redo through the expected points, restart, and restore a prior durable
revision; assert current content and immutable history at each step.

### T2.3 — Song, Arrangement, Choir, and repertoire domain behavior

**Goal:** Implement the complete MVP Song Library model without UI concerns.

**Changes:** `crates/domain/src/song_library/interface.rs`,
`crates/domain/src/song_library/model/`, `tests/song-library/domain.*`

**Depends on:** T2.1, T2.2.

**AC**

- [ ] `[AUTO]` A Song work owns identity, title, aliases, original attribution,
      provenance, rights notes, liturgical moments, applicability, and archive
      state; primary title is the only creation requirement.
- [ ] `[AUTO]` An Arrangement belongs to one Song and independently owns title,
      languages, Choir associations, rendition attribution, applicability,
      sections, Sequence, pagination state, and archive state.
- [ ] `[AUTO]` Numbered settings such as Kyrie 8th and Kyrie 13th remain separate
      Song works; translated renditions retaining one composition are separate
      Arrangements.
- [ ] `[AUTO]` Multiple Choirs may share one exact Arrangement; Songs and
      Arrangements remain valid without a Choir.
- [ ] `[AUTO]` Normalized case-insensitive duplicate evidence never rewrites
      authored capitalization or automatically unites entities.
- [ ] `[AUTO]` Archival is non-cascading and reversible; permanent deletion is
      allowed only after reference_safety checks and never cascades.
- [ ] `[AUTO]` Song merge and redirect behavior is absent from the MVP Interface.

**E2E** `[AUTO]` — Create two numbered Kyries, one Song with Cebuano and English
Arrangements shared across different Choir sets, archive the parent and one
child independently, restore the parent, and assert exact visibility and
selection eligibility.

### T2.4 — Persistent Library/Songs and Choir workspaces

**Goal:** Provide stable discovery and selection surfaces before detailed
Arrangement editing.

**Changes:** `crates/ordo/src/songs/library/`, `crates/ordo/src/songs/choirs/`,
`crates/domain/src/song_library/search.rs`, `tests/e2e/songs/library.*`

**Depends on:** T0.3, T2.3.

**AC**

- [ ] `[UI-AUTO]` Library exposes local Songs, Responses & Prayers, and General
      Presentations views; this task implements Songs and Choirs without
      placeholder repository/API layers for the later views.
- [ ] `[UI-AUTO]` Songs uses the permanent master-detail shell for empty,
      populated, no-match, archived-only, and selected states.
- [ ] `[UI-AUTO]` Empty state offers Create song and Import `.ordo` without an
      onboarding dashboard or layout replacement.
- [ ] `[AUTO]` Search is normalized and case-insensitive across Song titles,
      aliases, attribution/source, Arrangement title/attribution, Choir,
      language, and full lyrics.
- [ ] `[AUTO]` Choir/language/occasion filters retain a Song only when one
      Arrangement satisfies every active filter; missing metadata is
      Unspecified.
- [ ] `[UI-AUTO]` Match evidence explains alias or lyric hits and matching vs
      total Arrangements without changing authored text.
- [ ] `[UI-AUTO]` Selection, query, filters, and scroll survive entering and
      returning from an Arrangement.
- [ ] `[UI-AUTO]` Choir management supports name-only creation, optional details,
      archive visibility, and duplicate-safe inline quick-create.
- [ ] `[UI-AUTO]` Include archived is off by default and deliberately adds
      archived Songs/Arrangements to search without changing their state.

**E2E** `[UI-AUTO]` — Seed mixed Songs/Arrangements, combine search with three
Arrangement-scoped filters, open an Arrangement and return, then verify the same
query, evidence, selection, and scroll; repeat from every empty-state variant.

### T2.5 — Arrangement-free Song creation and duplicate preview

**Goal:** Let users establish correct Song identity without prematurely forcing
Arrangement data.

**Changes:** `crates/ordo/src/songs/new_song/`,
`crates/domain/src/song_library/duplicate_evidence.rs`, `tests/e2e/songs/create.*`

**Depends on:** T2.3, T2.4.

**AC**

- [ ] `[UI-AUTO]` New Song requires only primary title and excludes lyrics,
      language, Choir, section, Sequence, and slide-boundary fields.
- [ ] `[UI-AUTO]` Optional identity metadata matches the adopted Song-work
      ownership.
- [ ] `[AUTO]` Possible existing Songs are ranked from normalized identity
      evidence without automatic selection.
- [ ] `[UI-AUTO]` Evidence is collapsed to a compact count; choosing a candidate
      opens a nested identity-only preview rather than the full Song page.
- [ ] `[UI-AUTO]` Back preserves the staged form; Use this existing Song
      explicitly abandons it and selects the existing result.
- [ ] `[UI-AUTO]` Create song commits once and lands on a valid No Arrangements
      yet detail with optional Add first Arrangement.

**E2E** `[UI-AUTO]` — Enter a near-duplicate title and metadata, inspect two
candidates, return without losing form state, create the new Song, reload, and
verify it exists with no fabricated Arrangement.

### T2.6 — Arrangement details and lifecycle surface

**Goal:** Create and revise Arrangement metadata at one explicit transactional
boundary.

**Changes:** `crates/ordo/src/songs/arrangement_details/`,
`crates/domain/src/song_library/arrangements.rs`, `tests/e2e/songs/arrangement_details.*`

**Depends on:** T2.3, T2.5.

**AC**

- [ ] `[UI-AUTO]` New/Edit Arrangement uses the same viewport-bounded floating
      details surface with scenario-specific Create or Save action.
- [ ] `[AUTO]` At least one language is required; title and Choirs are optional,
      and blank display title derives from language/Choir context.
- [ ] `[UI-AUTO]` Composer/original author are absent; translator/adapter,
      applicability, source, rendition, and rights notes are available.
- [ ] `[UI-AUTO]` Editing a multi-Choir shared Arrangement warns before Save;
      Duplicate Arrangement is available as the safe branch.
- [ ] `[UI-AUTO]` Cancel discards staged metadata without disturbing the
      underlying page; successful creation lands in Lyrics.
- [ ] `[AUTO]` Metadata Save creates a revision without mutating immutable
      snapshots or existing Mass-local overrides.

**E2E** `[UI-AUTO]` — Create a blank-title two-language Arrangement shared by
two Choirs, verify derived label, edit it through the warning, cancel a second
edit, duplicate it, and assert independent identities and revision histories.

### T2.7 — Keyboard-first Lyrics authoring and slide boundaries

**Goal:** Encode reusable lyrics once while making presentation splitting fast,
visible, and reversible.

**Changes:** `crates/ordo/src/songs/lyrics/`, `crates/domain/src/song_library/sections.rs`,
`crates/domain/src/pagination/manual_boundaries.rs`, `tests/e2e/songs/lyrics.*`

**Depends on:** T0.4, T2.2, T2.6.

**AC**

- [ ] `[UI-AUTO]` Lyrics and Sequence are the only primary Arrangement task
      tabs; metadata remains in Arrangement details.
- [ ] `[UI-AUTO]` An Arrangement may persist with no sections and offers Add
      first section without fabricating Verse 1.
- [ ] `[UI-AUTO]` Section creation requires one free-form unique nonblank name,
      offers editable naming suggestions, then focuses its lyric editor.
- [ ] `[UI-AUTO]` Normal newline continues a slide; one blank source line starts
      the next slide; removing it joins slide groups through ordinary editing.
- [ ] `[UI-AUTO]` Adjacent mouse controls show exact targets such as Split after
      line 2 and Join slide 2 with slide 1; Join is disabled for the first slide.
- [ ] `[AUTO]` One section may generate multiple slides without creating another
      section or Sequence occurrence.
- [ ] `[UI-AUTO]` Generated previews use fixed-size 16:9 thumbnails that wrap
      without stretching sparse rows; selection drives one canonical Inspector.
- [ ] `[AUTO]` Reordering the Lyrics section list changes authoring order only,
      supports Alt+Up/Down and Undo, and never changes Sequence order.
- [ ] `[AUTO]` Removing a referenced section confirms and atomically removes all
      occurrences; Undo restores lyrics, boundaries, approval, and positions.

**E2E** `[UI-AUTO]` — Create Verse 1 and Chorus entirely by keyboard, enter
lyrics containing two slide groups, split and join with both text and buttons,
reorder sections, remove/Undo one referenced section, reload, and compare
canonical thumbnails with expected fixtures.

### T2.8 — Linked performance Sequence editor

**Goal:** Build sung order from reusable sections without duplicating chorus
lyrics or weakening occurrence identity.

**Changes:** `crates/ordo/src/songs/sequence/`,
`crates/domain/src/song_library/sequence.rs`, `tests/song-library/sequence.*`,
`tests/e2e/songs/sequence.*`

**Depends on:** T2.7.

**AC**

- [ ] `[UI-AUTO]` The Sequence workspace uses Available sections, ordered
      expandable occurrences, and canonical Inspector panes.
- [ ] `[AUTO]` Section creation never implicitly adds an occurrence; Add to
      Sequence inserts after selection or appends when none is selected.
- [ ] `[AUTO]` Repeat occurrence creates another reference to the same section
      immediately after selection without copying lyrics/slides.
- [ ] `[AUTO]` Drag or Alt+Up/Down reorders whole occurrences atomically; every
      generated child slide moves with its occurrence.
- [ ] `[AUTO]` Delete acts only while an occurrence list row owns focus, removes
      the occurrence but not its section, chooses next then previous, and Undo
      restores it.
- [ ] `[UI-AUTO]` Empty Sequence is a saved Incomplete state with Add first
      occurrence; unused sections do not warn.
- [ ] `[AUTO]` Readiness considers only referenced sections, deduplicates repeated
      section findings, and reports affected occurrence count.
- [ ] `[UI-AUTO]` Expand/collapse-all changes disclosure only; Inspector follows
      flattened occurrence/slide position.

**E2E** `[UI-AUTO]` — Build `Verse 1 → Chorus → Verse 2 → Chorus`, edit Chorus
once, assert both occurrences update, reorder and Delete/Undo by keyboard, and
verify exact flattened Cue order and canonical frames after reload.

### T2.9 — Entity-owned History, archive, restore, and safe deletion

**Goal:** Make ordinary library evolution recoverable without introducing the
deferred Song-merge system.

**Changes:** `crates/ordo/src/history/`,
`crates/domain/src/song_library/lifecycle.rs`,
`crates/domain/src/song_library/reference_safety.rs`,
`crates/storage/src/song_references.rs`, `tests/e2e/songs/history-lifecycle.*`

**Depends on:** T2.2, T2.3, T2.8.

**AC**

- [ ] `[UI-AUTO]` Song and Arrangement headers expose their own History actions
      and full-app floating revision slideovers.
- [ ] `[UI-AUTO]` Nested revision comparison is read-only; Arrangements provide
      Overview, Lyrics, Sequence, and Slides views.
- [ ] `[UI-AUTO]` Restore is whole-entity only and uses the adopted two-stage,
      uncarded confirmation footer; restoration creates a later revision.
- [ ] `[AUTO]` Song archive suppresses child selection without changing child
      archive states; Arrangement archive is independent.
- [ ] `[UI-AUTO]` Archived entities are inspect-only until restored and remain
      discoverable through explicit status filters.
- [ ] `[AUTO]` Permanent Arrangement deletion and childless Song deletion are
      blocked by any durable reference and never cascade; safe deletion removes
      owned revisions only after explicit non-undoable confirmation.
- [ ] `[UI-AUTO]` Archive offers immediate Undo; permanent deletion does not.
- [ ] `[UI-AUTO]` Archiving the selected Song chooses the next visible active
      result, otherwise the previous; when neither exists, the active-list empty
      state receives focus while Undo remains available.
- [ ] `[AUTO]` No merge, survivor, redirect, or reversal action exists.

**E2E** `[UI-AUTO]` — Edit an Arrangement across two History sessions, compare
and restore an old revision, archive/restore it, attempt deletion with a fixture
reference and observe the blocker, remove that reference, and complete the safe
permanent-deletion path.

### T2.10 — Automatic-first Song import review

**Goal:** Accept durable machine exchange and lightweight human Song authoring
without Ordo interpreting arbitrary PowerPoint files.

**Changes:** `crates/domain/src/song_library/import/`, `crates/ordo/src/songs/import_review/`,
`contracts/song-import/`, `tests/fixtures/song-import/`,
`tests/e2e/songs/import.*`

**Depends on:** T0.8, T2.3, T2.5, T2.7, T2.8, T2.9.

**AC**

- [ ] `[AUTO]` Accept one or many `song-import` `.ordo` artifacts whose JSON
      payload obeys the published AI-authoring contract; never inspect PPT/PPTX.
- [ ] `[AUTO]` Accept one `.md`/`.txt` as one Song plus one Arrangement with YAML
      front matter, bracket labels, hard blank-line Slide boundaries,
      `[Unlabeled]`, and uniquely resolved forward/backward `[Repeat: label]`
      references; unresolved repeats preserve a saveable but unready draft.
- [ ] `[UI-AUTO]` Markdown opens directly as an unsaved Song/Arrangement editor
      with inline validation and ordinary duplicate comparison; `.ordo` retains
      batch review, stable identity, and idempotent machine exchange.
- [ ] `[AUTO]` Normalize identity comparisons case-insensitively and classify
      Add new Song, Attach Arrangement, Skip unchanged, or Needs decision with
      concrete evidence.
- [ ] `[AUTO]` Resolve Song identity once per document and classify contained
      Arrangements independently without distributing them across Song works.
- [ ] `[UI-AUTO]` Imported document and nearest local candidate open in one
      read-only nested comparison with Overview, Lyrics, Sequence, and Slides.
- [ ] `[UI-AUTO]` A changed near-duplicate Arrangement offers Add as new,
      Update existing, or Skip; Update replaces the whole Arrangement as one
      restorable revision after impact disclosure and never internally merges.
- [ ] `[AUTO]` Invalid documents are inspectable but excluded; all Needs decision
      rows must resolve before one atomic batch commit.
- [ ] `[AUTO]` Any contained invalid Arrangement rejects its whole document; a
      commit failure leaves the entire approved batch unchanged.
- [ ] `[AUTO]` Reimport of unchanged stable identities is idempotent.

**E2E** `[UI-AUTO]` — Import a batch containing one new Song, one unchanged
Arrangement, one attachable Arrangement, one ambiguous replacement, and one
invalid document; resolve the ambiguity, commit, reload, and assert exact atomic
results and revision evidence. Repeat unchanged as a no-op, then open a Markdown
Song with a forward repeat and one unresolved label, repair it in the editor,
save, and assert the exact linked Sequence and Slide boundaries.

### Phase 2 gate

- [ ] `[AUTO]` External JSON fixtures round-trip through `.ordo` without Ordo
      depending on PowerPoint interpretation.
- [ ] `[UI-AUTO]` A user can create a Song, encode sections, build repeated
      Sequence occurrences, search it, revise it, import another, and recover an
      earlier revision after application reload.
- [ ] `[AUTO]` Lifecycle and import fault injection never leaves broken durable
      references or partial approved batches.

---

## Phase 3 — Themes, templates, and complete Mass Planning

The complete offline preparation application. **Ends with:** an operator can
create a Celebration, prepare several language/choir-specific Mass plans from
templates and library content, resolve pagination/readiness, inspect every Cue,
and reopen everything after restart. Presentation still uses a generated
candidate; portable immutable snapshots arrive in Phase 4.

### T3.1 — Pagination Module and proposal corpus

**Goal:** Centralize automatic pagination and overflow policy before Themes and
Mass content depend on stable slide divisions.

**Changes:** `crates/domain/src/pagination/interface.rs`, `crates/domain/src/pagination/proposal.rs`,
`crates/domain/src/pagination/validation.rs`, `tests/pagination/`,
`tests/fixtures/pagination/`

**Depends on:** T0.4, T2.7.

**AC**

- [ ] `[AUTO]` The Pagination Interface accepts authored content, intentional
      boundaries, Theme metrics, and slide kind and returns proposals plus
      structured fit findings.
- [ ] `[AUTO]` Lyrics target two logical phrases and no more than four rendered
      lines; splitting precedes font reduction and automatic reduction stops at
      64 canonical units.
- [ ] `[AUTO]` Authored line breaks and blank-line slide boundaries remain
      intentional inputs rather than text normalized away.
- [ ] `[AUTO]` Corpus tests cover one-to-five-line lyrics, repeated sections,
      prose responses/readings, long unbreakable words, punctuation, mixed
      languages, safe-area overflow, and minimum-font failure.
- [ ] `[AUTO]` Content or relevant Theme-metric edits invalidate prior review;
      unrelated metadata changes do not.
- [ ] `[AUTO]` The Interface never silently clips, invents a boundary inside an
      unsafe unit, or returns a partial render as valid.

**E2E** `[AUTO]` — Run the full pagination corpus through proposal, canonical
render, manual boundary adjustment, approval, relevant edit, and invalidation;
assert stable proposals and exact review-state transitions.

### T3.2 — Theme domain and protected Ordo Black fallback

**Goal:** Guarantee renderable output before parish customization begins.

**Changes:** `crates/domain/src/themes/interface.rs`, `crates/domain/src/themes/model/`,
`crates/domain/src/themes/ordo_black.rs`, `crates/rendering/src/theme_resolution.rs`,
`tests/themes/`

**Depends on:** T0.4, T2.1, T3.1.

**AC**

- [ ] `[AUTO]` Theme owns typography, text colors, fallback/background color,
      optional image/crop, layout values, safe area, and revision identity.
- [ ] `[AUTO]` Theme safe area may increase but never reduce the canonical
      120×80 minimum; semantic templates retain renderer-owned header/body/
      footer anchors rather than arbitrary element positioning.
- [ ] `[AUTO]` Protected Ordo Black is always available, deterministic, and
      independently renderable without a decorative asset.
- [ ] `[AUTO]` Ordo Black can be selected or duplicated but not edited, renamed,
      archived, or deleted.
- [ ] `[AUTO]` Missing/corrupt Theme image resolves coherently through saved
      fallback color, yields Degraded diagnostics, and never produces a partial
      image frame.
- [ ] `[AUTO]` Contrast, overflow, aspect/crop, and source-resolution findings
      are structured; non-16:9 and below-1920×1080 image findings are advisory.
- [ ] `[AUTO]` Canonical specimen fixtures cover Celebration title, Song
      identification, Lyrics with/without label, Response, Reading
      identification/passage, Psalm, and General text/image kinds.

**E2E** `[AUTO]` — Remove every parish Theme and verify Ordo Black still renders
all specimen kinds; corrupt a saved background image and assert complete
fallback-color frames plus one deduplicated Degraded finding.

### T3.3 — Theme Library and staged Theme Editor

**Goal:** Let users safely author shared visual identity without surprise
propagation or per-slide styling.

**Changes:** `crates/ordo/src/themes/library/`,
`crates/ordo/src/themes/editor/`, `crates/ordo/src/themes/history/`,
`tests/e2e/themes/`

**Depends on:** T0.3, T2.2, T3.2.

**AC**

- [ ] `[UI-AUTO]` Theme Library always shows Ordo Black under System Theme and
      uses a distinct No parish themes yet state with Create parish theme.
- [ ] `[UI-AUTO]` Editor specimen navigation covers every canonical template
      from T3.2 plus Current Mass slide without changing saved content.
- [ ] `[UI-AUTO]` Typography, Appearance, and Layout task groups expose only
      adopted Theme-level properties with live canonical specimen previews.
- [ ] `[UI-AUTO]` Text/background color selection shows current and staged
      values, contrast evidence, and fallback behavior without generic color
      dashboard chrome.
- [ ] `[UI-AUTO]` Editing a shared Theme is staged in place with affected-plan
      warning; Save creates one revision, while Duplicate creates a safe new
      Theme and redirects the editing context.
- [ ] `[UI-AUTO]` Theme Library supports Rename, Duplicate, Set as parish
      default, recoverable Archive, and Restore; archiving the current parish
      default first requires selecting its replacement.
- [ ] `[UI-AUTO]` Dirty navigation offers stay, discard, or save; returning from
      contextual edit restores staged Mass Theme chooser context.
- [ ] `[UI-AUTO]` Theme History compares and restores whole revisions through
      the common nested History interaction.
- [ ] `[AUTO]` Saved changes propagate automatically to editable inheriting
      plans; immutable snapshots remain unchanged and exception review identifies
      affected item overrides.

**E2E** `[UI-AUTO]` — Duplicate Ordo Black, alter type/color/image crop/layout,
review all specimens and contrast warnings, save, inspect History, restore the
prior revision, and verify an existing inheriting plan updates while a fixture
snapshot remains byte-identical.

### T3.4 — Mass Template model and six-section scaffold

**Goal:** Provide reusable language-specific starting structures without
pretending every Celebration has one universal Ordo.

**Changes:** `crates/domain/src/mass_planning/templates/`, `crates/domain/src/mass_sections.rs`,
`tests/mass-planning/templates.*`

**Depends on:** T2.1, T3.2.

**AC**

- [ ] `[AUTO]` New templates start with independently identified editable
      sections: Before Mass, Introductory Rites, Liturgy of the Word, Liturgy of
      the Eucharist, Concluding Rites, and After Mass; Communion remains inside
      Liturgy of the Eucharist.
- [ ] `[AUTO]` Every template has a language; multiple independently named
      templates may share a language and one may be its explicit fallback.
- [ ] `[AUTO]` A template owns optional default Response and Responsorial Psalm
      projected-header strings for its language. Instantiation copies them into
      the Mass plan; neither the renderer nor Celebration translates them.
- [ ] `[AUTO]` Sections use UUID plus unique nonblank label, own ordered items,
      carry no inferred behavior, and enforce at least one section/exactly-one
      section membership for every item.
- [ ] `[AUTO]` Template items may be concrete content or typed Song/Reading slots
      and may carry advisory applicability/conditional defaults.
- [ ] `[AUTO]` Conditional evaluation uses Celebration context but produces a
      visible recommendation/override result; uncertainty warns rather than
      silently omitting content.
- [ ] `[AUTO]` Template instantiation copies its structure into an independent
      Mass plan while concrete Songs, Responses/Prayers, and General
      Presentations retain reusable Library links; later template-structure
      edits never synchronize either direction.
- [ ] `[AUTO]` Recommendation ranks applicable same-language templates and
      exposes the language fallback without prohibiting manual selection.

**E2E** `[AUTO]` — Build Ordinary Sunday, Lent, and language-fallback templates;
instantiate each under matching and ambiguous Celebration contexts, assert
conditional Gloria behavior and unresolved slots, then edit the source and
prove existing plans do not change.

### T3.5 — Template Library and editor workspace

**Goal:** Make reusable Mass structure manageable in the same interaction
language as Mass Planning.

**Changes:** `crates/ordo/src/masses/templates/`,
`crates/ordo/src/mass_structure/`, `tests/e2e/templates/`

**Depends on:** T0.3, T3.4.

**AC**

- [ ] `[UI-AUTO]` Masses → Templates is a local Masses tab, not Settings or a
      global destination.
- [ ] `[UI-AUTO]` Empty workspace retains the permanent list/editor shell and
      offers Create template; only name and language are initially required.
- [ ] `[UI-AUTO]` Editor supports section add/rename/reorder/remove, item
      add/reorder/remove, expand/collapse-all, and conditional/slot controls.
- [ ] `[UI-AUTO]` Section header actions, insertion targeting, focus-scoped
      Delete/Alt+Up/Down, and minimum structural invariants match Mass Planning.
- [ ] `[UI-AUTO]` Empty sections remain visible with counts while authoring;
      collapse state is local UI state and absent from revisions.
- [ ] `[AUTO]` Template archive/history behavior uses ordinary edit revisions
      and cannot mutate instantiated plans.

**E2E** `[UI-AUTO]` — Create a Cebuano Sunday template, rename/reorder sections,
add conditional Gloria plus Song and Reading slots, set it as fallback, reload,
and instantiate an independent plan with exact copied structure.

### T3.6 — Celebration and Mass-plan domain behavior

**Goal:** Implement the language-neutral Celebration and independently scheduled
Mass-plan relationship that organizes all preparation.

**Changes:** `crates/domain/src/mass_planning/interface.rs`,
`crates/domain/src/mass_planning/celebrations/`, `crates/domain/src/mass_planning/plans/`,
`tests/mass-planning/celebrations.*`

**Depends on:** T2.1, T3.4.

**AC**

- [ ] `[AUTO]` Celebration requires liturgical date and title and may own season,
      color, applicability tags, and several independent Mass plans.
- [ ] `[AUTO]` A Mass plan requires scheduled start and primary language and may
      own Choir, Presider, location, private notes, Theme, sections, items,
      copied default Response/Psalm headers, and local overrides.
- [ ] `[AUTO]` Celebration date is distinct from scheduled start; an explicitly
      confirmed Anticipated Mass may occur on the preceding civil day and keeps
      the later Celebration context.
- [ ] `[AUTO]` Duplicate for another Mass stays under one Celebration; same-
      language duplication copies fully, while different language derives from
      an applicable target-language template and independently authored text.
- [ ] `[AUTO]` Reuse for another Celebration creates an independent plan and
      requires a new Celebration/date; no synchronization remains and every
      copied Reading starts Needs Review regardless of language, with explicit
      per-item and confirmed all-readings review clearing.
- [ ] `[AUTO]` Each plan generates an enabled Celebration-title Cue before its
      first section, inherits Celebration title by default, and may override
      projected title/appearance without adding Celebration language maps.
- [ ] `[AUTO]` Presider/location editable suggestions and Parish defaults copy
      only into new plans and never retroactively rewrite saved plans.
- [ ] `[AUTO]` Shared Celebration edits recompute readiness/review evidence but
      do not silently change plan items, schedules, reused plans elsewhere, or
      immutable snapshots.

**E2E** `[AUTO]` — Create one Sunday Celebration with Saturday Anticipated,
Cebuano Sunday, and English Sunday plans; duplicate and derive across languages,
reuse one into a new date, change source/Parish defaults, and assert every
ownership boundary.

### T3.7 — Celebration-grouped Masses workspace

**Goal:** Make chronological Mass planning discoverable without flattening the
Celebration relationship.

**Changes:** `crates/ordo/src/masses/index/`,
`crates/ordo/src/masses/creation/`, `tests/e2e/masses/index.*`

**Depends on:** T0.3, T3.5, T3.6.

**AC**

- [ ] `[UI-AUTO]` Masses is the default global destination and owns Upcoming,
      History, Archive, and Templates horizontal local tabs.
- [ ] `[UI-AUTO]` Upcoming and History show chronological Celebration groups
      with subordinate plan rows containing actual time, language, Choir, and
      readiness; hierarchy remains with one plan.
- [ ] `[AUTO]` Upcoming ordering uses each Celebration's earliest scheduled plan
      and accurately displays preceding-day Anticipated plans; a Celebration
      without plans uses the local start of its liturgical date as its fallback
      ordering key.
- [ ] `[UI-AUTO]` + New Celebration requires only date/title; after Save it
      offers Create from template, Reuse an earlier Mass plan, or Do this later.
- [ ] `[UI-AUTO]` Empty Celebration remains valid and offers plan creation
      inline; existing groups offer + Add another Mass.
- [ ] `[UI-AUTO]` Celebration context menus own shared actions; plan rows own
      plan-specific actions including Present.
- [ ] `[UI-AUTO]` Historical plans open read-only by default and expose an
      explicit Edit past plan action; Archive/Restore and Reuse for another
      Celebration preserve their adopted ownership and revision semantics.
- [ ] `[UI-AUTO]` Opening a focused plan preserves originating tab/group/scroll
      and breadcrumb return restores them.

**E2E** `[UI-AUTO]` — Create an empty Celebration, add three plans including an
anticipated one, move the date from Upcoming to History with a test clock, open
one read-only and enter Edit past plan, return, archive/restore another, reuse a
third under a new Celebration/date, and verify hierarchy, order, revisions,
labels, and retained context.

### T3.8 — Three-pane Mass Planning shell and section structure

**Goal:** Implement the stable plan-level workspace before individual Mass-item
editors fill its center pane.

**Changes:** `crates/ordo/src/masses/planner/`,
`crates/ordo/src/mass_structure/`, `tests/e2e/masses/planner-shell.*`

**Depends on:** T0.3, T3.6, T3.7.

**AC**

- [ ] `[UI-AUTO]` Plan header, Mass outline, selected-item editor, and Inspector
      follow the approved three-pane ownership and responsive reference.
- [ ] `[UI-AUTO]` The automatic Celebration-title Cue is selectable before the
      first section, defaults enabled, and remains available as the idle Cue.
- [ ] `[UI-AUTO]` Header owns Celebration context, autosave, readiness access,
      Mass Theme, Mass details, and Present; outline owns only its title and
      top-right + Add Mass item.
- [ ] `[UI-AUTO]` Six default sections and all custom sections remain visible
      in authoring with counts and individual/all disclosure; Presenter omits
      empty sections.
- [ ] `[AUTO]` Clicking a section header establishes insertion target and clears
      item selection; exactly one selected item otherwise drives editor and
      Inspector.
- [ ] `[AUTO]` Drag or Alt+Up/Down moves a whole section or focused item within
      the hierarchy while retaining one-section membership and Undo.
- [ ] `[UI-AUTO]` Section menu supports rename, add above/below, move, and only
      eligible removal; item right-click/overflow uses the same action model.
- [ ] `[UI-AUTO]` At 1366×768/100% all three panes remain. When the effective
      workspace cannot preserve 240px outline, 520px editor, and 300px
      Inspector minima, Inspector then outline become opposing labeled
      slideovers, one at a time, with no whole-workspace horizontal scroll.
- [ ] `[UI-AUTO]` Resizing restores fixed panes, selection, scroll, autosave,
      and focus context without persisting a content revision.

**E2E** `[UI-AUTO]` — Populate every section, reorder sections/items by mouse
and keyboard, remove/Undo an item, cycle F6 regions, resize through every layout
state, and compare both target resolutions with the approved reference hierarchy.

### T3.9 — Add Mass item and focused content editors

**Goal:** Support every MVP Mass-item type through its owning editor rather than
one ambiguous generic form.

**Changes:** `crates/ordo/src/masses/add_item/`,
`crates/ordo/src/masses/item_editors/`, `crates/domain/src/mass_planning/items/`,
`tests/e2e/masses/items.*`

**Depends on:** T3.8.

**AC**

- [ ] `[UI-AUTO]` + Add Mass item opens a categorized chooser for Song,
      Reading, linked Response/Prayer, General Presentation, Custom sequence,
      Hold, and Blackout; it inserts after current selection or appends to the
      active section when none is selected. If neither an item nor section is
      active, including in an item-empty plan, the chooser requires selecting
      one of the plan's sections before an item type can be chosen.
- [ ] `[AUTO]` Song items reference a library Arrangement by default, own an
      optional identification Cue, and expand the exact section-occurrence
      Sequence into position-based Cues.
- [ ] `[UI-AUTO]` Song picker uses a floating right slideover with preview,
      filters, and scenario-adapted primary commit; selection alone does not
      mutate the plan.
- [ ] `[UI-AUTO]` Mass-local Song customization explicitly copies the reusable
      Arrangement; linked repeated sections update together unless Edit this
      occurrence only creates a local fork.
- [ ] `[UI-AUTO]` Change song/arrangement guards detachment of local changes;
      Return to library arrangement uses one direct guarded comparison, not a
      nested picker.
- [ ] `[AUTO]` Reading supports language, reference, body, responses/acclamations,
      optional full-reading projection, hard blank-line boundaries, ordered
      language-template Gospel blocks, and multiple Reading slots/items.
- [ ] `[AUTO]` Responsorial Psalm generates `n + 1` linked refrains around `n`
      cantor verses; private cantor Wait Cues leave Program unchanged and may
      be enabled for projection. Its item may inherit, hide, or override the
      Mass plan's authored Psalm header.
- [ ] `[AUTO]` Response sequence owns ordered exchange blocks with optional
      operator-only trigger then required congregation response; role defaults
      are Priest (`P`) and congregation (`T`) but remain extensible. Reusable
      entries stay linked until Customize for this Mass, and the item may
      inherit, hide, or override the Mass plan's authored Response header.
- [ ] `[AUTO]` A General Presentation item may add Mass-local private delivery
      context alongside its constrained Text/Image Slides; that context is
      operator-only and never enters a canonical frame or reusable source.
- [ ] `[AUTO]` General Presentation Text is the focused projectable
      announcement path; Custom sequence supports deliberate mixed projectable
      and operator-only Cue behavior; Hold is an ordinary black Cue; Blackout
      is a state-changing Cue.
- [ ] `[UI-AUTO]` Remove from this Mass never deletes reusable source content
      and offers immediate Undo.

**E2E** `[UI-AUTO]` — Starting from one active section, add every item type,
configure a Song with repeated Chorus, a two-block response, readings, Hold, and
Blackout, reorder them, customize/return one Song, and assert exact flattened
Cue behavior and source-library immutability.

### T3.10 — Mass details, Parish defaults, and Choir selection

**Goal:** Make metadata complete but lightweight, with clear shared-versus-plan
ownership.

**Changes:** `crates/ordo/src/masses/details/`,
`crates/ordo/src/settings/parish/`, `crates/domain/src/mass_planning/metadata.rs`,
`tests/e2e/masses/details.*`

**Depends on:** T2.4, T3.6, T3.8.

**AC**

- [ ] `[UI-AUTO]` Mass details opens as a viewport-bounded floating slideover
      without losing selected item.
- [ ] `[UI-AUTO]` Shared Celebration fields and plan-specific fields are
      visually separated and labeled; liturgical grade/rank is absent.
- [ ] `[UI-AUTO]` Season and liturgical color use simple supported choices;
      plan language and Choir use selectors with explicit Not assigned where
      valid.
- [ ] `[UI-AUTO]` Presider and Location accept suggestions or immediate custom
      values; notes are unrestricted private multiline text.
- [ ] `[UI-AUTO]` Parish Settings owns Regular Presiders, optional Default
      Presider, Default Location, and Choir management links without becoming a
      clergy/location directory.
- [ ] `[AUTO]` Changing shared fields returns a review summary and recomputes
      conditional/readiness results without hidden content mutation.
- [ ] `[AUTO]` Default changes affect only later plan creation.

**E2E** `[UI-AUTO]` — Configure regular and guest Presiders plus default
location, create a new plan and verify prefill, enter a custom bishop and venue,
change Celebration season/color, review effects, and prove an older plan and
template remain unchanged.

### T3.11 — Mass Theme chooser and item override review

**Goal:** Apply shared visual identity coherently while keeping item exceptions
explicit.

**Changes:** `crates/ordo/src/masses/theme_chooser/`,
`crates/domain/src/mass_planning/theme_resolution.rs`, `tests/e2e/masses/theme.*`

**Depends on:** T3.3, T3.8, T3.9.

**AC**

- [ ] `[UI-AUTO]` Header Theme action opens a full-app floating chooser with
      canonical preview and staged selection.
- [ ] `[UI-AUTO]` Review opens a nested floating child slideover spanning app
      chrome; the parent recedes and restores on Back.
- [ ] `[UI-AUTO]` Override rows show current Theme → resulting inherited Theme,
      allow staged removal, and preserve plan/item context.
- [ ] `[AUTO]` One Apply action commits Mass Theme selection plus accepted
      override removals atomically.
- [ ] `[UI-AUTO]` Edit selected theme routes to the Theme Editor and returns to
      the still-staged chooser; Theme save propagation updates the preview.
- [ ] `[AUTO]` Clearing an override restores inheritance without copying the
      current Mass Theme onto the item.

**E2E** `[UI-AUTO]` — Give three items different override states, stage a new
Mass Theme, review current→resulting values, remove one override, edit the Theme
and return, apply, reload, and assert exact Theme resolution for all items.

### T3.12 — Inspector Cue navigation and owner-directed pagination fixes

**Goal:** Make every generated Cue inspectable while routing corrections back to
the content owner.

**Changes:** `crates/ordo/src/masses/inspector/`,
`crates/domain/src/mass_planning/cue_flattening.rs`, `tests/e2e/masses/inspector.*`

**Depends on:** T3.1, T3.9, T3.11.

**AC**

- [ ] `[AUTO]` The selected Mass item's canonical flattened Cue order is the
      single Inspector navigation source; navigation stops at item boundaries.
- [ ] `[UI-AUTO]` Projectable Cue selection shows the authoritative canonical
      frame; operator-only trigger shows black with private context outside it.
- [ ] `[UI-AUTO]` Inspector explicitly exposes previous/next, position,
      warnings, item Theme override, and contextual Fix pagination—no generic
      overflow menu.
- [ ] `[UI-AUTO]` Selecting a generated child opens its containing occurrence
      as necessary and keeps item/outline selection synchronized.
- [ ] `[UI-AUTO]` Fix pagination routes directly to the owning Arrangement or
      Mass-local Lyrics/response editor and returns to the same Cue context.
- [ ] `[AUTO]` Operator-only context is absent from the canonical frame and
      congregation renderer payload.

**E2E** `[UI-AUTO]` — Navigate every Cue in a Song and response item from both
occurrence rows and Inspector controls, invoke a pagination fix, change the
boundary, return, and verify corrected frames and preserved selection.

### T3.13 — Computed readiness and preparation preflight UI

**Goal:** Surface actionable problems without inventing an approval state or
blocking valid content unnecessarily.

**Changes:** `crates/domain/src/mass_planning/readiness.rs`,
`crates/ordo/src/masses/readiness/`, `tests/mass-planning/readiness.*`,
`tests/e2e/masses/readiness.*`

**Depends on:** T3.1–T3.12.

**AC**

- [ ] `[AUTO]` Readiness computes Incomplete, Ready with warnings, or Ready from
      unresolved slots, invalid/missing dependencies, pagination, archived
      sources, degraded Themes, conditional ambiguity, and review findings.
- [ ] `[AUTO]` Readiness is derived and never stored as approval/completion.
- [ ] `[UI-AUTO]` Header counts open one floating Readiness slideover grouped as
      Incomplete presentation and Review recommended.
- [ ] `[UI-AUTO]` Readiness states and counts use a compact semantic
      `StatusLabel` introduced with this first production caller; T0.3 does not
      add a generic Badge or ubiquitous pill primitive.
- [ ] `[UI-AUTO]` A jump preserves the panel; a corrective action closes it and
      focuses the owning editor control; fixed findings disappear automatically.
- [ ] `[AUTO]` An archived referenced Arrangement is a warning and stays
      presentable; a missing/unrenderable dependency makes affected Cues
      unavailable.
- [ ] `[UI-AUTO]` Present remains available behind acknowledgment for incomplete
      content; only inability to capture/open a coherent presentation candidate
      is fatal.

**E2E** `[UI-AUTO]` — Seed every finding category, follow each jump/correction,
verify automatic count changes, attempt Present with remaining incomplete Cues,
and assert the exact unavailable set and absence of an approval button.

### T3.14 — Source-neutral Liturgy import review

**Goal:** Accept externally prepared liturgical content without making any
language/source provider part of Ordo core.

**Changes:** `crates/domain/src/mass_planning/liturgy_import/`,
`crates/ordo/src/masses/liturgy_import/`, `contracts/liturgy-import/`,
`tests/fixtures/liturgy-import/`, `tests/e2e/masses/liturgy_import.*`

**Depends on:** T0.8, T3.9, T3.13.

**AC**

- [ ] `[AUTO]` Accept a `liturgy-import` `.ordo` artifact with language,
      Celebration evidence, readings, responses, roles, source references, and
      intended target slots/items.
- [ ] `[AUTO]` Provider identity such as Sulugoon is optional provenance only;
      imported parish content remains editable and source-independent.
- [ ] `[UI-AUTO]` Review previews imported text and target placement before any
      plan mutation; mismatched language/date/slot evidence requires a decision.
- [ ] `[AUTO]` Invalid inputs cannot be force-imported and do not block unrelated
      valid documents in a multi-document intake.
- [ ] `[AUTO]` Commit copies approved language-specific content into the Mass
      plan atomically and never synchronizes future source changes.
- [ ] `[UI-AUTO]` Manual entry remains available for every supported content
      type when no import exists.

**E2E** `[UI-AUTO]` — Review a Cebuano import with readings/responses, resolve
one ambiguous target, commit to a plan, edit the copied response, delete the
source artifact, restart offline, and verify complete independent authoring and
rendering.

### T3.15 — Reusable Responses, Prayers, and General Presentations

**Goal:** Complete the multi-view Library without flattening specialized Song,
exchange, and presentation models into one generic content record.

**Changes:** `crates/domain/src/reusable_content/`,
`crates/ordo/src/library/responses_prayers/`,
`crates/ordo/src/library/general_presentations/`,
`crates/storage/src/reusable_content/`, `tests/e2e/library/reusable-content.*`

**Depends on:** T0.3, T2.2, T3.9.

**AC**

- [ ] `[UI-AUTO]` Library local views are Songs, Responses & Prayers, and
      General Presentations with stable search, selection, History, and Include
      archived behavior while preserving type-specific editors.
- [ ] `[AUTO]` Response/Prayer sources own ordered exchange blocks; General
      Presentation sources own constrained Text or Image Slides plus an
      eight-second entry duration inherited by Slides with optional overrides.
- [ ] `[AUTO]` One Mass plan binds every still-linked occurrence of one source
      to one Edit revision. Reusing it adds that same bound revision; reviewed
      update advances linked occurrences atomically while local overrides and
      immutable snapshots remain unchanged.
- [ ] `[UI-AUTO]` Current, Update available, Mass-local override, Source updated,
      and Archived source states expose direct compare/update/restore routes.
- [ ] `[AUTO]` Promotion after upstream divergence never auto-merges or
      overwrites. It saves only an explicit reviewed new source revision and
      relinks the promoting occurrence only when content exactly matches.
- [ ] `[AUTO]` Archive preserves every retained binding and remains presentable;
      permanent deletion is limited to never-referenced entries with no retained
      history.

**E2E** `[UI-AUTO]` — Reuse one response and two announcements multiple times,
customize one occurrence, revise/archive/promote their sources, update one plan,
and assert plan-wide binding consistency, visible divergence, stable snapshots,
and exact reload behavior.

### Phase 3 gate

- [ ] `[UI-AUTO]` From first_run Parish setup, create Choirs, Songs, Themes, a
      language template, one Celebration, and a complete representative Sunday
      Mass with every adopted content and control item.
- [ ] `[AUTO]` Every projectable Cue renders through the same Canonical Slide
      Renderer Interface qualified in Phase 0/1.
- [ ] `[UI-AUTO]` Reload at several points and finish with exact durable content,
      selection context, readiness, revisions, and no internet access.
- [ ] `[MANUAL]` The paired 1366×768 and 1920×1080 Mass Planning acceptance
      hierarchy matches the approved responsive reference.

---

## Phase 4 — Immutable presentation, transfer, and data protection

The preparation-to-church round trip. **Ends with:** every plan presents from an
immutable snapshot, a complete Celebration moves safely between unrelated
installations and back, concurrent home/church edits are reviewed correctly,
and the Parish can be backed up and restored without merging.

### T4.1 — Dependency-closed Presentation snapshot capture and reuse

**Goal:** Freeze exactly what will render before Presenter, transfer, or export
can depend on mutable library content.

**Changes:** `crates/portability/src/snapshots/`, `contracts/snapshot/`,
`crates/storage/src/snapshots/`, `tests/portability/snapshots.*`

**Depends on:** T1.1–T1.3, T3.13.

**AC**

- [ ] `[AUTO]` Snapshot capture accepts one exact Mass-plan revision and closes
      over ordered sections/items/Cues, resolved content, Themes, fonts, assets,
      pagination, complete canonical frames, and availability diagnostics.
- [ ] `[AUTO]` Captured snapshots are immutable; no editing, Theme, library,
      asset, archive, or restoration action can rewrite them.
- [ ] `[AUTO]` Content hashing is deterministic and excludes installation-only
      UI state, cache files, display settings, and edit timestamps that do not
      affect presentation.
- [ ] `[AUTO]` Capture is atomic; any missing/inconsistent required dependency
      returns a structured blocker and exposes no partial snapshot.
- [ ] `[AUTO]` Re-entering Presenter or exporting with the same exact content
      reuses the snapshot; changing any presentation dependency creates a new
      one automatically.
- [ ] `[AUTO]` Snapshot verification can run without opening editable Parish
      records and detects tampering or missing frames/assets.

**E2E** `[AUTO]` — Capture a complete plan, enter/exit twice without edits and
assert one snapshot UUID, change only private plan notes and verify reuse, change
one lyric boundary and verify a new snapshot while the first remains byte-identical.

### T4.2 — Presenter preflight and automatic handoff

**Goal:** Turn a prepared plan into a safe Presenter session with automatic
validation and focused acknowledgment only when necessary.

**Changes:** `crates/presentation/src/preflight/`, `crates/ordo/src/masses/present_action/`,
`crates/ordo/src/preflight/`, `tests/preflight/`,
`tests/e2e/presenter/handoff.*`

**Depends on:** T1.6, T3.13, T4.1.

**AC**

- [ ] `[AUTO]` Preflight checks snapshot integrity, fonts/assets, Cue
      availability, renderer probe, output/display/calibration evidence, and
      interrupted-session state through one normalized Interface.
- [ ] `[AUTO]` Present flushes pending autosave and validates the exact current
      revision before snapshot reuse/capture.
- [ ] `[UI-AUTO]` Clean or review-only warnings enter Presenter directly with
      congregation output disabled.
- [ ] `[UI-AUTO]` Incomplete content opens the approved centered,
      viewport-bounded modal listing unavailable Cues by section/item; initial
      focus is Review and fix and Present available content is the explicit
      warning-toned acknowledgment.
- [ ] `[UI-AUTO]` Cancel/review creates no snapshot; proceeding reuses or
      captures one automatically without an approval concept.
- [ ] `[AUTO]` Only inability to capture or open one coherent snapshot is a
      Presenter-entry blocker.
- [ ] `[UI-AUTO]` Exit restores the selected plan, item, Cue, scroll, and
      Inspector context captured at handoff.

**E2E** `[UI-AUTO]` — Enter from Ready, Ready with warnings, Incomplete, and
fatal-capture fixtures; assert direct/modal/blocked behavior, snapshot counts,
initial output-disabled state, and exact preparation context after exit.

### T4.3 — Durable Presentation sessions and recovery journal

**Goal:** Connect the Phase 1 recovery model to durable snapshots and local
session records without exposing stale Program after restart.

**Changes:** `crates/presentation/src/sessions/`, `crates/presentation/src/journal/`,
`crates/storage/src/presentation_sessions/`, `tests/e2e/presenter/durable-recovery.*`

**Depends on:** T1.7, T4.1, T4.2.

**AC**

- [ ] `[AUTO]` A Presentation session references exactly one immutable snapshot
      and records start/end/recovery status without modifying the snapshot.
- [ ] `[AUTO]` Journal captures Preview, Program context, state intent, output
      evidence, Ordo/timeline disclosure, and later emergency changes through
      ordered checksummed entries.
- [ ] `[UI-AUTO]` Planned exit closes the journal and does not offer recovery on
      next launch.
- [ ] `[UI-AUTO]` Unexpected termination reopens the same snapshot directly in
      native-full-screen Presenter with external black and the adopted recovery
      strip.
- [ ] `[AUTO]` A journal for a missing/damaged snapshot cannot be resumed and
      produces a safe actionable diagnostic without external output.
- [ ] `[AUTO]` Recovery never depends on current library records having remained
      unchanged after snapshot capture.

**E2E** `[UI-AUTO]` — Start a real stored snapshot, navigate and enter each
state, terminate at scripted points, edit/archive its source library record
between runs, and verify recovered context still derives solely from the intact
snapshot with Program black.

### T4.4 — Celebration Package capture and verified export

**Goal:** Publish one self-contained Celebration artifact safely, with all plans
and no partial-export concept.

**Changes:** `crates/portability/src/celebration_package/export.rs`,
`crates/storage/src/package_history/`, `crates/ordo/src/transfer/export_celebration/`,
`tests/portability/celebration-export.*`

**Depends on:** T0.8, T4.1.

**AC**

- [ ] `[AUTO]` One Celebration Package contains the whole Celebration, an
      independent immutable snapshot for every current Mass plan, deduplicated
      offline dependencies, and editable-source lineage.
- [ ] `[UI-AUTO]` Export opens the same full-app floating slideover from a
      Celebration context menu or Transfer & Backup and lists every plan,
      readiness, blockers, size, destination, and collapsed contents.
- [ ] `[AUTO]` Incomplete plans may export with visible warnings; only inability
      to create a complete, internally consistent artifact blocks.
- [ ] `[AUTO]` Artifact filenames are human-readable date/title names while
      manifest UUIDs own identity and version.
- [ ] `[AUTO]` Content-equivalent export reuses the exact latest Package-version
      UUID and fingerprint; changed scope creates a new immutable version.
- [ ] `[AUTO]` Package History commits only after temporary publication,
      destination read-back, and checksum verification; failure/cancel leaves no
      new history or corrupted prior destination.
- [ ] `[UI-AUTO]` Completion state persists until dismissed and shows location,
      size, fingerprint/version, new/reused status, Show file, and Copy again.

**E2E** `[UI-AUTO]` — Export a three-plan Celebration with one incomplete plan,
verify the artifact independently, export unchanged and assert exact reuse,
then edit one plan, replace the destination with injected finalization failure,
and assert the old file/history remain intact.

### T4.5 — Two-layer Celebration Package import staging

**Goal:** Separate artifact trust from editable library decisions so immutable
presentation evidence can never be altered by reconciliation.

**Changes:** `crates/portability/src/celebration_package/import.rs`,
`crates/ordo/src/transfer/import_review/`, `crates/storage/src/import_staging/`,
`tests/portability/celebration-import.*`

**Depends on:** T4.4.

**AC**

- [ ] `[AUTO]` Layer 1 copies to local temporary staging and verifies container,
      kind/version, hashes, dependency closure, every snapshot, and artifact
      identity before any mutation.
- [ ] `[AUTO]` Layer 2 stages mappings and updates for editable Songs,
      Arrangements, Themes, templates, Celebrations, and Mass plans without
      changing package snapshots.
- [ ] `[UI-AUTO]` Parent full-app slideover summarizes source, artifact version,
      verification, plans, dependencies, and automatic decisions.
- [ ] `[UI-AUTO]` Possible Song/Arrangement matches open a nested side-by-side
      read-only comparison of incoming and local Overview/Lyrics/Sequence/Slides.
- [ ] `[AUTO]` Invalid artifacts are indivisible and cannot proceed; valid
      identity/reconciliation ambiguity is Needs decision and blocks only until
      resolved.
- [ ] `[AUTO]` One approved commit atomically installs immutable Package history
      and accepted editable local content; failure leaves all Parish state
      unchanged.
- [ ] `[AUTO]` Importing an identical Package version is a no-op; importing an
      older version retains history but never makes it Current automatically.
- [ ] `[UI-AUTO]` Imported content appears as ordinary local content with
      restrained provenance and no ongoing Imported state.

**E2E** `[UI-AUTO]` — Into an unrelated Parish library, import a Package with
new, unchanged, possible-match, and invalid-dependent cases; resolve the nested
comparison, inject commit failure, retry successfully, and assert exact snapshot
registration, Current version, ordinary content, and atomicity.

### T4.6 — Revision ancestry and symmetric round-trip reconciliation

**Goal:** Support the real home→church→home→church workflow while asking the
operator only about true overlapping or unmappable changes.

**Changes:** `crates/portability/src/reconciliation/`, `crates/storage/src/sync_anchors/`,
`crates/ordo/src/transfer/reconciliation/`,
`tests/portability/round-trip.*`

**Depends on:** T2.2, T4.5.

**AC**

- [ ] `[AUTO]` Stable UUID plus proven common Edit revision establishes a sync
      anchor; matching UUID alone does not prove content equivalence.
- [ ] `[AUTO]` Base/local/incoming comparison classifies unchanged, incoming-
      only, local-only, non-overlapping auto-merge, overlapping divergence, and
      unmappable identity.
- [ ] `[AUTO]` Non-overlapping field/structure changes carry forward
      automatically while overlapping changes require an explicit decision.
- [ ] `[UI-AUTO]` Reconciliation previews both values/content and the resulting
      local record before commit; wording is Update/Keep/Add, not sync jargon.
- [ ] `[AUTO]` Accepted revisions establish new anchors only after atomic import
      commit; export itself never advances an anchor.
- [ ] `[AUTO]` Church edits from any included Mass plan can return in the same
      complete Celebration Package type; there is no Return Package or delta.
- [ ] `[AUTO]` Immutable Package snapshots remain exact historical artifacts
      regardless of source-record reconciliation choices.

**E2E** `[AUTO]` `[UI-AUTO]` — Export at home, import at church, independently
edit different fields and one overlapping lyric at both installations, export
church, import home, resolve the overlap, make another home edit, and return to
church; assert anchors, auto-carry-forward, decisions, and all Package versions.

### T4.7 — Installed Package history and ordinary Masses integration

**Goal:** Keep portable provenance inspectable without creating a second Mass
navigation system.

**Changes:** `crates/ordo/src/transfer/package_history/`,
`crates/ordo/src/masses/package_provenance/`,
`crates/storage/src/package_history/`, `tests/e2e/transfer/package_history.*`

**Depends on:** T3.7, T4.5, T4.6.

**AC**

- [ ] `[UI-AUTO]` Current imported Celebration appears as one ordinary
      Celebration group with ordinary chronological plan rows in Masses.
- [ ] `[UI-AUTO]` Restrained provenance identifies package origin/current
      version without labeling editable content as a special imported mode.
- [ ] `[UI-AUTO]` Transfer & Backup owns Package artifact/version history,
      fingerprints, verification, and Current selection.
- [ ] `[AUTO]` Each Package version and its snapshots remain immutable; changing
      Current changes the managed installed lineage, not historical bytes.
- [ ] `[AUTO]` Plans remain available from intact independent snapshots when an
      editable reconciled source later changes.
- [ ] `[UI-AUTO]` Unavailable or Damaged retained versions remain visible with
      evidence and cannot become Current or present.

**E2E** `[UI-AUTO]` — Install three versions including an older one, inspect
history, verify only the newest accepted version is Current, open its ordinary
Mass rows, damage a managed artifact copy, and verify other intact snapshots and
versions remain independently usable.

### T4.8 — Portable Parish Backup creation and protection reminder

**Goal:** Protect the full authoritative Parish without confusing backups with
Celebration transfer or device cloning.

**Changes:** `crates/portability/src/parish_backup/create.rs`,
`crates/storage/src/backups/`, `crates/ordo/src/transfer/protect_parish/`,
`tests/portability/backup-create.*`

**Depends on:** T0.8, T2.1, T4.7.

**AC**

- [ ] `[AUTO]` Parish Backup closes over authoritative content, assets,
      revisions, Package provenance, mappings, anchors, settings, and selected
      device metadata, excluding caches, staging, logs, active recovery,
      binaries, and source installation UUID.
- [ ] `[UI-AUTO]` Protect this parish uses the shared export slideover with
      complete counts, size, integrity, destination, and collapsed contents and
      offers no selective controls.
- [ ] `[AUTO]` Portable Backup creation uses temporary publication, read-back
      verification, persistent completion, Show file, and Copy again.
- [ ] `[AUTO]` Automatic safety Backups run only before migration, data-changing
      upgrade, and restore; they are not substitutes for a portable copy.
- [ ] `[UI-AUTO]` When Parish content changed and last portable Backup is at
      least seven days old, a non-blocking reminder states the authoritative
      change count.
- [ ] `[AUTO]` No scheduled job writes to absent removable media and no runtime
      upload exists.

**E2E** `[UI-AUTO]` — Populate every authoritative data category, create and
independently verify a Backup, mutate Parish content and advance the test clock,
observe the reminder, copy the verified artifact again, and verify exclusions
and stable content hashes.

### T4.9 — Replace-only Parish restore and rollback

**Goal:** Recover or migrate a Parish predictably without partial merge or
misleading Undo semantics.

**Changes:** `crates/portability/src/parish_backup/restore.rs`,
`crates/ordo/src/transfer/restore/`, `crates/platform/src/restart_coordinator.rs`,
`tests/portability/backup-restore.*`

**Depends on:** T4.8.

**AC**

- [ ] `[UI-AUTO]` Restore is a dedicated full-page current-versus-Backup
      comparison; it never offers merge or selective restoration.
- [ ] `[AUTO]` Restore action stays disabled until an accessible exact-current
      Backup is recently verified with no later changes, or a fresh local safety
      Backup is created and verified.
- [ ] `[UI-AUTO]` The protection requirement is explicit; no Parish-name typing
      ceremony or Undo restore is offered.
- [ ] `[AUTO]` Candidate restore is fully verified into staging, atomically
      switched, application-restarted, and integrity-checked before success.
- [ ] `[AUTO]` Any validation/switch/restart-integrity failure rolls back to the
      exact prior Parish and retains actionable evidence.
- [ ] `[AUTO]` Previous state is retained as an ordinary Parish Backup selectable
      through the same Restore workflow.
- [ ] `[AUTO]` Restoring on another installation retains Parish content but
      generates/keeps local installation identity and resets unrecognized
      display calibration/Output Fit safely.

**E2E** `[NATIVE-AUTO]` — Restore a Backup into a differently seeded temporary
installation, force failure before and after atomic switch to verify rollback,
then complete successfully, restart, and compare every authoritative category
while asserting new local installation/display behavior.

### T4.10 — Backup retention and damaged-artifact handling

**Goal:** Keep protection evidence honest when users move, delete, or damage
artifact files.

**Changes:** `crates/portability/src/artifact_health/`,
`crates/ordo/src/transfer/artifact_health/`, `tests/portability/artifact_health.*`

**Depends on:** T4.7–T4.9.

**AC**

- [ ] `[AUTO]` Distinguish Unavailable path/access from Damaged verification
      failure; neither counts as current protection or usable input.
- [ ] `[UI-AUTO]` Retained artifacts remain visible with exact evidence and
      Locate another copy where applicable.
- [ ] `[AUTO]` Replacement succeeds only for exact artifact identity and full
      fingerprint and is atomic; Ordo never salvages or rewrites a damaged file.
- [ ] `[AUTO]` Artifact, extracted dependencies, reconciled records, and each
      independent snapshot have separate health results.
- [ ] `[UI-AUTO]` Safety Backups are retained until explicit permanent deletion;
      no automatic rotation exists in MVP.
- [ ] `[UI-AUTO]` Deletion discloses reason, time, size, and lost recoverability
      and never deletes exported artifacts outside app-managed storage.

**E2E** `[UI-AUTO]` — Move one retained artifact, tamper another, damage one
snapshot dependency while leaving siblings intact, locate an exact copy, reject
a same-name/different-fingerprint copy, and verify protection status after each
operation.

### Phase 4 gate

- [ ] `[NATIVE-AUTO]` A complete multi-plan Celebration prepared on the Windows
      preparation installation exports to `.ordo`, imports on an unrelated
      offline Windows installation, and every
      snapshot renders identically to source golden hashes.
- [ ] `[AUTO]` The same Package type completes church→home→church round trips
      with automatic non-overlap handling and reviewed divergence.
- [ ] `[NATIVE-AUTO]` Parish Backup restore survives injected failures and leaves
      either the exact old or exact verified new Parish—never a mixture.
- [ ] `[UI-AUTO]` Presenter recovery uses immutable snapshots and keeps external
      output black after every tested interruption.

---

## Phase 5 — Church operations, fallback, and diagnostics

The operational workflows surrounding a live Mass. **Ends with:** an operator
can configure and calibrate displays, run actionable preflight, make safe
emergency changes, export a validated fallback deck, diagnose the offline
installation, and recover from font/artifact faults.

### T5.1 — First-run Parish and installation setup

**Goal:** Collect only the information needed to start local preparation and
keep installation-specific choices separate from Parish content.

**Changes:** `crates/ordo/src/first_run/`, `crates/ordo/src/settings/parish/`,
`crates/ordo/src/settings/installation/`, `tests/e2e/first_run.*`

**Depends on:** T3.10, T4.9.

**AC**

- [ ] `[UI-AUTO]` First launch requires only Parish name and offers optional
      default location and regular Presiders/default Presider without requiring
      Choirs, internet, login, Theme creation, template creation, or display
      assignment.
- [ ] `[UI-AUTO]` Optional setup can be skipped and completed later through
      Settings; Masses opens as the default destination.
- [ ] `[AUTO]` Parish settings travel in Parish Backup as appropriate;
      installation identity, local paths, and recognized display evidence do not
      clone onto restore.
- [ ] `[UI-AUTO]` A fully restored Parish skips first_run Parish creation and
      enters with safe local installation defaults.

**E2E** `[UI-AUTO]` — Complete minimal setup, restart, verify defaults seed only
new plans, restore the Parish on a fresh installation, and confirm content
returns while display assignment remains Not assigned.

### T5.2 — Displays settings, identification, and calibration

**Goal:** Make installation output configuration observable and safe before
Presenter relies on real projector geometry.

**Changes:** `crates/ordo/src/settings/displays/`,
`crates/platform/src/display/calibration/`, `crates/ordo/src/congregation/calibration/`,
`tests/e2e/displays/`

**Depends on:** T0.5, T3.2, T5.1.

**AC**

- [ ] `[UI-AUTO]` Settings → Displays shows live topology, recognition evidence,
      operator and congregation assignment, Identify displays, calibration
      state, and Output Fit per recognized display.
- [ ] `[UI-AUTO]` Not assigned is a valid explicit congregation setting and is
      visually distinct from an assigned display becoming unavailable.
- [ ] `[UI-AUTO]` Calibration opens the congregation window black, then only the
      calibration pattern; exiting returns black before close.
- [ ] `[AUTO]` Pattern marks the 120×80 minimum safe area and Output Fit adjusts completed-frame
      uniform scale from 0–10% without recomposition.
- [ ] `[AUTO]` Ratios within 0.1% of 16:9 fill; other ratios letterbox/pillarbox
      complete frames and warn without cropping/stretching.
- [ ] `[UI-AUTO]` Canonical Preview mirrors the exact Output Fit result.
- [ ] `[MANUAL]` Identify, assignment, calibration, pointer behavior, and actual
      projector fit are verified on Windows two-display setups.

**E2E** `[UI-AUTO]` `[MANUAL]` — Identify two displays, assign congregation,
calibrate from 0% through 10%, simulate near/exceeding aspect tolerance,
disconnect/reconnect, and confirm no silent reassignment and black transitions.

### T5.3 — Emergency insert and emergency edit

**Goal:** Let the operator respond to an unplanned need without risking the
current Program or silently changing reusable library content.

**Changes:** `crates/presentation/src/emergency/`,
`crates/ordo/src/presenter/emergency_editor/`,
`crates/domain/src/mass_planning/local_overrides/`, `tests/e2e/presenter/emergency.*`

**Depends on:** T2.2, T3.9, T4.3.

**AC**

- [ ] `[UI-AUTO]` Insert immediately Freezes Program and opens a viewport-
      bounded chooser/editor for General text/image, Response sequence, or Song.
      The Song path is a centered, keyboard-first **Quick Insert** chooser whose
      field filters results and whose arrow keys move the active result; it
      composes the existing Field, selection-list, Dialog, focus, and scrolling
      behavior rather than adding a global Command Palette primitive. Cancel
      leaves Program held and creates no item.
- [ ] `[UI-AUTO]` Save inserts a Mass-local item after the current Mass item in
      the same section, selects its first projectable Cue in Preview, and leaves
      Program held until Space. A missing Song/Arrangement is Mass-local first.
- [ ] `[UI-AUTO]` Ctrl+E immediately Freezes and edits the selected projectable
      Cue in Preview only; operator-only/noneditable targets explain refusal.
- [ ] `[UI-AUTO]` Editing a reused linked section applies to every linked
      occurrence in this Mass by default, with an explicit selected-occurrence
      fork option.
- [ ] `[AUTO]` Journal includes emergency inserts/overrides so restart recovery
      restores them without mutating the immutable source snapshot or library.
- [ ] `[AUTO]` Post-Mass review may deliberately promote applicable content as
      a later library/Mass revision; promotion never mutates the used snapshot.
- [ ] `[UI-AUTO]` Back/close/exit flushes pending saves; failure offers Retry
      save, Keep editing, or destructive Leave without saving and successful
      retry resumes the original navigation.
- [ ] `[UI-AUTO]` Next Mass appears only for the next scheduled sibling under
      the Celebration, fades current Program black, closes safely, and opens
      sibling preflight without automatic exposure.

**E2E** `[UI-AUTO]` — While a Cue is live, Insert text, cancel, Insert/save an
image, Space it live, emergency-edit a linked Chorus for all occurrences and a
single occurrence, terminate/recover, and verify Program holds plus exact local
ownership throughout.

### T5.4 — Production emergency PowerPoint export

**Goal:** Turn any intact Presentation snapshot into a validated static fallback
that preserves Mass order and private notes.

**Changes:** `crates/powerpoint/`,
`crates/ordo/src/masses/emergency_export/`,
`tests/powerpoint/production.*`

**Depends on:** T0.9, T4.1.

**AC**

- [ ] `[AUTO]` Export consumes an immutable snapshot and its complete canonical
      frames; it never reflows text through PowerPoint.
- [ ] `[AUTO]` Arrangement occurrences expand fully in Cue order and every Mass
      item maps to an actual PowerPoint section.
- [ ] `[AUTO]` Operator-only trigger/Hold slides are black with speaker notes;
      Blackout and black lead-in/closing Safety sections are preserved.
- [ ] `[AUTO]` Reopen validation verifies all frames, hashes, order, sections,
      dimensions, and notes before atomic destination publication.
- [ ] `[UI-AUTO]` Completion clearly reports verified destination and offers
      Show file; failure leaves prior destination intact.
- [ ] `[MANUAL]` A representative exported deck runs correctly in the church
      PC's installed PowerPoint version.

**E2E** `[AUTO]` `[MANUAL]` — Export the sanitized Normal Mass snapshot,
automatically compare every deck frame and section against snapshot order, then
run the entire deck in church PowerPoint and confirm safety slides/private notes.

### T5.5 — Offline Diagnostics Module and issue-first workspace

**Goal:** Give operators actionable local evidence without exposing Parish
content or pretending diagnostics repair unrelated Modules.

**Changes:** `crates/ordo/src/diagnostics/`, `crates/ordo/src/settings/diagnostics/`,
`contracts/diagnostic-report/`, `tests/diagnostics/`

**Depends on:** T2.1, T4.10, T5.2.

**AC**

- [ ] `[AUTO]` One Diagnostics Interface returns normalized Healthy/Needs
      attention summary, timestamp, checks, environment facts, and issue-owned
      corrective routes.
- [ ] `[AUTO]` Checks cover build/OS/hardware/GPU, database/schema, assets,
      bundled fonts, storage, displays/calibration, recent transfer/Backup, and
      crash/recovery evidence.
- [ ] `[UI-AUTO]` Page shows latest result immediately, refreshes cheap facts on
      entry, and runs expensive full scan only from explicit Run diagnostics.
- [ ] `[AUTO]` Scan is read-only and cancellable/serialized; it never repairs,
      uploads, changes Program, or exposes findings to congregation output.
- [ ] `[UI-AUTO]` Issue-first single-column hierarchy leads with actionable
      failures; technical healthy evidence remains available without dominating.
- [ ] `[AUTO]` Copy summary and diagnostic-report `.ordo` export sanitize Parish
      name, titles, lyrics, readings, notes, media, usernames, serials, full
      paths, and file bodies.
- [ ] `[AUTO]` Report import/open is read-only and cannot mutate Parish data.

**E2E** `[UI-AUTO]` — Seed database, asset, font, storage, display, Backup, and
recovery findings, run/cancel/rerun diagnostics, follow each owning-workflow
link, export a report, and scan its extracted content for every prohibited field.

### T5.6 — Self-healing bundled-font integrity

**Goal:** Preserve canonical typography through local corruption while assigning
installer repair only to packaged resources.

**Changes:** `crates/rendering/src/fonts/integrity/`, `crates/ordo/src/diagnostics/font_check.rs`,
`crates/ordo/src/startup/font_recovery.rs`, `tests/fonts/integrity.*`

**Depends on:** T0.4, T5.5.

**AC**

- [ ] `[AUTO]` Verify runtime font cache and read-only factory pack against one
      versioned pinned manifest.
- [ ] `[AUTO]` Damaged runtime cache reconstructs atomically from a verified
      factory pack and passes a renderer probe before becoming active.
- [ ] `[AUTO]` Missing factory pack with intact runtime cache permits normal
      output and reports non-blocking Font recovery copy missing.
- [ ] `[AUTO]` If neither copy verifies, system fonts are never substituted;
      management authoring remains, but exact previews, new snapshots, and
      Celebration export are blocked.
- [ ] `[AUTO]` A previously validated snapshot with intact frames remains
      presentable under the both-copies-lost condition without recomposition.
- [ ] `[UI-AUTO]` Corrective guidance runs same-version installer Repair; full
      reinstall with Parish preservation is suggested only after Repair fails.

**E2E** `[NATIVE-AUTO]` — Corrupt runtime only, factory only, and both in separate
packaged copies; restart, assert reconstruction/degraded/blocking behavior,
render probes, diagnostics, old-snapshot availability, and absence of system
font fallback.

### T5.7 — Keyboard-shortcut reference and Help ownership

**Goal:** Make the fixed keyboard model discoverable in Preparation and
Presenter without duplicating command definitions.

**Changes:** `crates/ordo/src/help/shortcuts/`,
`crates/ordo/src/commands/registry.rs`, `tests/e2e/help/shortcuts.*`

**Depends on:** T1.5, T3.8.

**AC**

- [ ] `[UI-AUTO]` Help and Presenter Ordo-rail footer expose labeled Keyboard
      shortcuts; Ctrl+/ toggles the same floating right panel.
- [ ] `[AUTO]` Visible keys derive from the command registry and cannot drift
      from registered commands.
- [ ] `[UI-AUTO]` Commands group under current workspace first, then Global,
      Editing, and Presenter as applicable; no search/customization is present.
- [ ] `[UI-AUTO]` Presenter panel is passive, keeps F/B/Space active, and Esc
      restores prior Cue/control focus without changing Program.
- [ ] `[MANUAL]` Keycap wording and focus order remain usable at 200% Interface
      scale and with Windows screen reader.

**E2E** `[UI-AUTO]` — Open the reference from three workspaces and Presenter,
compare displayed bindings to registry output, activate safe commands, verify
contextual omissions, close with Esc, and assert focus/state restoration.

### Phase 5 gate

- [ ] `[UI-AUTO]` One operator can configure an installation, calibrate fake
      output, preflight and run a stored Mass, emergency-insert/edit, recover,
      exit, inspect Diagnostics, and export an emergency deck completely offline.
- [ ] `[NATIVE-AUTO]` Native display, font-fault, diagnostic-export, and PPTX
      validation suites pass on packaged Windows artifacts.
- [ ] `[MANUAL]` Actual church PowerPoint opens and presents the validated deck.

---

## Phase 6 — Packaging, hardening, and church release qualification

Production readiness. **Ends with:** native installers and offline artifacts
pass automated fault/performance gates plus both physical church qualification
passes, and the product owner completes the MVP acceptance scenario.

### T6.1 — Windows installer, repair, upgrade, downgrade, and uninstall

**Goal:** Ship and maintain the air-gapped church installation without placing
Parish data at risk.

**Changes:** `packaging/windows/`, `xtask/src/package_windows.rs`,
`crates/ordo/src/startup/version_gate.rs`, `tests/packaged-smoke/windows/`

**Depends on:** T4.9, T5.6.

**AC**

- [ ] `[NATIVE-AUTO]` Build a Windows x64 per-machine installer for supported
      Windows 10 22H2 with all runtime assets bundled offline.
- [ ] `[NATIVE-AUTO]` Same-version install repairs packaged resources, rebuilds
      font cache, and preserves Parish data/Backups outside install directory.
- [ ] `[NATIVE-AUTO]` Upgrade creates and verifies a safety Backup before any
      first-launch data migration, then verifies schema/assets after migration.
- [ ] `[NATIVE-AUTO]` Failed migration/verification rolls back application data
      to the prior usable state and retains evidence.
- [ ] `[NATIVE-AUTO]` In-place downgrade and opening a newer Parish schema are
      refused without mutation; fonts are never extracted across versions.
- [ ] `[MANUAL]` Uninstall defaults to Keep Parish data — Recommended and removes
      binaries/disposable caches while a compatible reinstall reopens data.
- [ ] `[MANUAL]` Remove all local Ordo data shows exact categories, portable-
      Backup evidence, and unprotected-change warning; it never deletes external
      `.ordo` files.

- [ ] `[NATIVE-AUTO]` Packaging includes license notices for GPUI, Rust crates,
      fonts, Lucide, and every other shipped dependency (moved from the
      retired T6.2; Decision 268).

**E2E** `[NATIVE-AUTO]` `[MANUAL]` — Install clean, create a seeded Parish,
repair same version, upgrade through one migration, inject upgrade failure,
attempt downgrade, uninstall while keeping data/reinstall, then exercise the
explicit local-data removal path against disposable test data.

### T6.2 — Retired: Linux Flatpak and diagnostic ZIP (deferred post-MVP)

**Status:** Retired by Decision 268. Linux is not an MVP platform: preparation
and presentation both run on Windows. This task is not an MVP gate and no
Phase 6 task depends on it. It keeps the original Linux criteria, wording
unchanged, so a future post-MVP Linux port starts from them rather than from
nothing.

**Original goal:** Provide a first-class preparation build and a supportable
unpacked diagnostic artifact on the developer's Linux platform.

**Retained criteria (not gated)**

- `[NATIVE-AUTO]` Build Linux x64 Flatpak with offline runtime assets,
      desktop entry, icon, file association for `.ordo`, and needed display/file
      portal permissions documented.
- `[NATIVE-AUTO]` Build a versioned diagnostic ZIP that launches without
      installation and uses an isolated documented data location unless
      explicitly pointed at a test copy.
- `[NATIVE-AUTO]` Flatpak can create/open/copy `.ordo` artifacts through user
      file selection and run the same canonical renderer/contract tests.
- `[NATIVE-AUTO]` Packaging includes license notices for GPUI, Rust crates,
      fonts, Lucide, and every other shipped dependency.
- `[MANUAL]` Wayland and X11 launch, fullscreen Presenter, display discovery,
      file dialogs, and font rendering pass on supported Linux environments.
- `[NATIVE-AUTO]` Phase 0 Linux checks retired here by Decision 268, wording
      kept: serialized canonical logical glyph positions, wrapping,
      pagination inputs, safe area, and element bounds are byte-for-byte
      invariant across packaged Windows/Linux runs, 100–200% Ordo Interface
      scale, representative system scale, and output densities; raster
      sharpness is qualified separately per platform (from T0.4).
- `[NATIVE-AUTO]` On minimum-profile Windows and Linux test hardware, p95
      Presenter commands are below 50ms and total memory never exceeds the
      1 GiB absolute release ceiling (from T0.6).
- `[MANUAL]` Atkinson operator fit and Noto fallback are legible and stable
      in packaged Windows and Linux builds at both qualified resolutions (from
      T0.4; the retired `T0.4-MANUAL-GATE.md` script B).
- `[MANUAL]` Windows 10 22H2 and supported Linux packaged checks are signed
      off with failures and reruns retained (from T0.10).

**Retained E2E (not gated)** — Install Flatpak in a clean runner/user profile, create
a fixture Parish, export/import `.ordo`, launch Presenter without output, run
Diagnostics, uninstall/reinstall, and verify intended data persistence.

### T6.3 — Native CI, release artifacts, and provenance

**Goal:** Produce repeatable OS-native candidates without cross-compilation or
manual file assembly.

**Changes:** `.github/workflows/`, `xtask/src/release/`, `docs/releasing.md`,
`tests/packaged-smoke/`

**Depends on:** T6.1.

**AC**

- [ ] `[NATIVE-AUTO]` GitHub Actions builds Windows on Windows
      from the pinned Rust toolchain and committed Cargo dependency graph.
- [ ] `[AUTO]` Every job runs verify, contract corpus, golden rendering,
      packaged smoke, and applicable performance/fault subsets before upload.
- [ ] `[AUTO]` Release outputs include installers/packages, diagnostic ZIP,
      emergency fixture deck, version manifest, third-party notices, SHA-256
      checksums, and retained test reports.
- [ ] `[AUTO]` Packaged artifacts include a runtime manifest generated from the
      packaged resource inventory; packaged smoke rejects missing resources and
      any network-required declaration.
- [ ] `[AUTO]` Artifacts identify exact commit, version, Rust toolchain,
      GPUI/platform revision, schema head, contract versions, and bundled-font
      manifest hash.
- [ ] `[AUTO]` Draft release creation is automated; public/manual publication is
      deliberate and no background updater is shipped.
- [ ] `[NATIVE-AUTO]` Clean packaged smoke creates a temporary Parish, runs
      migrations, renders known frames, completes Celebration/Backup round trips,
      and leaves no real user data.

**E2E** `[NATIVE-AUTO]` — Tag a release candidate, build both OS artifacts from
clean runners, verify checksums and manifests, install them into clean test
profiles, run packaged smoke, and produce one draft release with all evidence.

### T6.4 — Fault-injection and fail-closed safety matrix

**Goal:** Prove failures stay inside their owning Module and never become unsafe
Program or partial durable mutation.

**Changes:** `tests/fault-injection/`, `tests/fixtures/failures/`,
`scripts/run-fault-matrix.*`

**Depends on:** Phases 1–5 complete.

**AC**

- [ ] `[AUTO]` Rendering faults cover font, image, decode, worker, GPU/context,
      unavailable future Cue, and incomplete frame.
- [ ] `[AUTO]` Storage faults cover migration, transaction, storage-worker
      thread death, disk-full/write error, asset coordination, and integrity
      failure.
- [ ] `[AUTO]` Portability faults cover traversal, unknown version, tamper,
      missing asset, read-back failure, destination replacement, interrupted
      finalization, reconciliation conflict, restore switch, and rollback.
- [ ] `[AUTO]` Display faults cover absent/ambiguous display, disconnect,
      reconnect under changed identity/topology, failed output creation, and
      congregation renderer termination.
- [ ] `[AUTO]` Recovery faults cover torn/stale journal, missing snapshot,
      interruption during emergency edit, and repeated crash during recovery.
- [ ] `[AUTO]` Every case asserts complete old-or-new durable state and Program
      of black/disconnected/last-valid complete frame only.
- [ ] `[UI-AUTO]` Operator UI exposes one specific actionable result and no raw
      stack/local path/private content on the congregation renderer.

**E2E** `[UI-AUTO]` — Run the indexed fault matrix against packaged-like
processes, compare final storage and Presentation state to explicit oracles, and
fail the suite for any unclassified exception, partial mutation, or unsafe frame.

### T6.5 — Performance, memory, startup, and long-session hardening

**Goal:** Meet the adopted native GPUI latency and evidence-based memory budgets
under realistic content and session duration, not only in the Phase 0 fixture.

**Changes:** `tests/performance/`, `scripts/profile/`,
`docs/qualification/performance/`, targeted implementations found by profiling

**Depends on:** Phases 1–5 complete, T6.1, T6.3.

**AC**

- [ ] `[NATIVE-AUTO]` Measure cold/warm startup, idle CPU, keydown→Preview,
      keydown→Program, command bursts, timeline scroll, search, large editor
      navigation, snapshot capture, and artifact operations.
- [ ] `[NATIVE-AUTO]` Presenter command p95 remains below 50ms on minimum-profile
      and church PCs; p50/p95/p99 distributions and outliers are retained.
- [ ] `[NATIVE-AUTO]` Total process memory stays within the 512 MiB
      qualification budget adopted in T0.6 and never crosses the 1 GiB absolute
      ceiling.
- [ ] `[AUTO]` Process and GPU-resource evidence proves bounded full-frame/thumbnail/media
      caches, virtualization, obsolete-resource release, and no unbounded History
      or journal UI retention.
- [ ] `[NATIVE-AUTO]` Rapid input reaches the correct final state without lost
      commands, broad timeline rerenders, UI hangs, or Program corruption.
- [ ] `[NATIVE-AUTO]` A longer automated soak supplements physical testing and
      shows no monotonic memory/resource growth after warmup.
- [ ] `[MANUAL]` Any revised minimum hardware claim is explicit, evidence-based,
      and accepted before release; hard-gate failure blocks release.

**E2E** `[NATIVE-AUTO]` — On minimum-profile Windows machines, run a
two-hour mixed preparation/Presenter soak with the Normal Mass plus a 500-Cue
stress fixture, collect process, worker, frame, and GPU-resource traces, and assert latency, memory, rapid-
input, idle, and leak gates.

### T6.6 — Accessibility, responsive, and visual acceptance matrix

**Goal:** Verify the adopted design remains operable after all real content and
failure states are present.

**Changes:** `tests/e2e/accessibility/`, `tests/e2e/responsive/`,
`docs/qualification/accessibility/`, fixes in owning UI Modules

**Depends on:** Phases 1–5 complete.

**AC**

- [ ] `[UI-AUTO]` Test 1366×768 and 1920×1080 at 100–200% Interface scale with
      representative Windows system scales and deliberately long names.
- [ ] `[UI-AUTO]` No whole-workspace horizontal scroll, clipped primary action,
      missing high-stakes Presenter control, or state mutation during responsive
      transitions.
- [ ] `[AUTO]` Automated accessibility checks cover landmarks, names, roles,
      current/selected/expanded/unavailable state, contrast, and dialog focus.
- [ ] `[MANUAL]` Keyboard-only operation covers every primary journey, context
      menu, drag alternative, nested slideover, dialog, and Presenter command.
- [ ] `[MANUAL]` Windows screen-reader testing verifies concise Cue naming,
      deduplicated polite updates, urgent fail-closed announcements, and absence
      of operator-only congregation content.
- [ ] `[MANUAL]` Forced colors and reduced motion preserve meaning, focus,
      content cuts, bounded blank fades, and equivalent nested-overlay navigation.
- [ ] `[MANUAL]` Visual comparison against approved references confirms
      hierarchy/ownership, not pixel-copying of mockup artifacts.

**E2E** `[UI-AUTO]` `[MANUAL]` — Complete Song creation, Mass preparation,
transfer review, and Presenter Recovery Drill using keyboard only across the
scale/resolution matrix, then repeat the critical Presenter path with Windows
screen reader and forced colors.

### T6.7 — Physical church qualification

**Goal:** Validate the exact release candidate through the real Windows 10 PC,
cabling, display/projector, room, and human operating conditions.

**Changes:** completed local copies of
`docs/qualification/PHYSICAL-QUALIFICATION-TEMPLATE.md`; do not commit filled
reports or Parish content

**Depends on:** T6.1, T6.3–T6.6.

**AC**

- [ ] `[PHYSICAL]` Record exact build/checksums, PC, GPU, OS, system scale,
      Ordo scale, displays, topology, cabling, projector, lighting, fixture hash,
      and installed PowerPoint version.
- [ ] `[PHYSICAL]` Run the sanitized Normal Mass for at least 60 uninterrupted
      minutes with realistic pacing/idle periods; use the longer typical local
      Sunday duration when applicable.
- [ ] `[PHYSICAL]` Separate operator and observer assess operation plus comfortable
      readability from the farthest routinely occupied and an off-center seat.
- [ ] `[PHYSICAL]` Exercise both generic black and light Themes, Cebuano/Noto
      glyphs, long wraps, one-to-four-line lyrics, readings, responses, repeated
      Chorus, triggers, Hold, Blackout, and both timeline modes.
- [ ] `[PHYSICAL]` Run the shorter Recovery Drill with rapid navigation, Freeze,
      Blackout, cursor recovery, emergency insert/edit, display disconnect/
      reconnect, interruption/restart, and final Program verification.
- [ ] `[PHYSICAL]` Run the generated emergency PPTX in installed PowerPoint and
      verify sections, images, private notes, and Safety slides.
- [ ] `[PHYSICAL]` No release-blocking failure from Decision 248 remains; every
      rerun and any accepted nonblocking operator-chrome issue has evidence and
      rationale.
- [ ] `[PHYSICAL]` Completed reports remain local with the release bundle and
      contain no parish lyrics/readings/PII, congregation imagery, or unsanitized
      diagnostics.

**E2E** `[PHYSICAL]` — Complete both adopted physical passes from installation
and calibration through normal presentation, adversarial recovery, emergency
PowerPoint, safe exit, and post-run diagnostics on the actual church system.

### T6.8 — Final MVP acceptance and release handoff

**Goal:** Demonstrate the promised outcome from an empty installation without
developer intervention or hidden setup.

**Changes:** `docs/acceptance/MVP-ACCEPTANCE.md`, release notes, known issues,
operator quick-start/help content

**Depends on:** T6.1, T6.3–T6.7.

**AC**

- [ ] `[MANUAL]` On the connected Windows preparation installation, create a Parish, Choirs, Songs/Arrangements, Theme,
      template, Celebration, and several Mass plans using manual and `.ordo`
      import paths.
- [ ] `[MANUAL]` Resolve readiness and pagination, export one complete verified
      Celebration Package and portable Parish Backup, and copy them to removable
      media.
- [ ] `[MANUAL]` On a clean air-gapped Windows church installation, import the
      Package, review only genuine decisions, calibrate output, and present one
      plan end-to-end without PowerPoint.
- [ ] `[MANUAL]` Make an emergency local edit, complete the Mass, export the same
      Celebration Package type back, reconcile it at home, and return a later
      version to church.
- [ ] `[MANUAL]` Generate and validate the emergency deck, run Diagnostics, and
      successfully restore the Parish Backup into an isolated test installation.
- [ ] `[AUTO]` All CI, native, contract, golden, fault, performance, packaged,
      and artifact checks reference the exact release candidate.
- [ ] `[PHYSICAL]` Physical qualification is signed off and no blocker remains.
- [ ] `[MANUAL]` Release notes clearly state supported OS/hardware, offline
      behavior, known limitations, Backup responsibility, and deferred features.

**E2E** `[MANUAL]` `[PHYSICAL]` — Execute the complete Windows preparation →
air-gapped Windows presentation → church edits → home reconciliation loop from
the written quick-start alone, with the emergency PPTX and Parish restore as
independent fallback/protection demonstrations.

### Phase 6 gate — MVP complete

- [ ] The outcome and every Included item in `MVP-PLAN.md` is represented by a
      checked task AC or an explicitly linked acceptance case.
- [ ] Every Explicitly excluded/deferred feature remains absent or clearly
      labeled unavailable; none is half-implemented as a dangerous shortcut.
- [ ] The exact release candidate passes all automatic, native, manual, and
      physical gates required by its change risk.
- [ ] The product owner can operate the complete acceptance scenario without a
      developer, internet access, or PowerPoint except during deliberate fallback
      qualification.

---

## Dependency summary

```text
Phase 0: platform proofs and contracts
    ↓
Phase 1: fixture-driven safe Presenter
    ↓
Phase 2: durable Song Library ───────┐
    ↓                               │
Phase 3: complete Mass preparation  │
    ↓                               │
Phase 4: snapshots and transfer ←───┘
    ↓
Phase 5: church operations and fallback
    ↓
Phase 6: packaging, hardening, physical qualification
```

The phases are ordered by risk and dependency, not by screen-menu order. Inside
one phase, tasks may run concurrently only when their declared dependencies and
shared file ownership do not overlap. No phase gate is satisfied by screenshots
or disconnected UI alone.

## Release-blocking budgets and invariants

- Presenter command latency: `<50ms` p95 for keydown→Preview and keydown→Program
  on the supported minimum profile and actual church PC.
- Native memory: every representative-session run remains within the `512 MiB`
  qualification budget; `1 GiB` remains the absolute release ceiling.
- Canonical composition: fixed `1920x1080`; Interface/system scaling may not
  change wrapping, pagination, geometry, or safe area.
- Program safety: only black, Disconnected, or a complete validated canonical
  frame; never partial, stale-confidence, operator-only, or diagnostic output.
- Output starts disabled; enabling creates black first; restart recovery is
  black first.
- Portable mutation is verify-first and atomic; storage/restore results are
  exact old-or-new, never partial.
- Runtime remains fully offline and self-contained.

## Evidence locations

| Evidence | Source-controlled location | Completed evidence location |
|---|---|---|
| Automated/native test definitions | `tests/`, `scripts/` | CI artifacts for exact commit/release |
| Golden canonical fixtures | `tests/golden-frames/` | CI diff/report artifacts |
| Contract fixtures | `tests/fixtures/contracts/` | CI validation report |
| Performance scripts | `tests/performance/`, `scripts/profile/` | Local/CI release bundle with process, worker, frame, and GPU metrics |
| Accessibility/manual scripts | `docs/qualification/` | Local release bundle; sanitized summary only if published |
| Physical qualification template | `docs/qualification/PHYSICAL-QUALIFICATION-TEMPLATE.md` | Local release bundle only; never source control |
