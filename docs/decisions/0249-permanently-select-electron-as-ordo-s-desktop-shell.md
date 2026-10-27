---
id: 249
title: "Permanently select Electron as Ordo's desktop shell"
status: superseded
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, platform]
supersedes: []
superseded_by: 252
amends: []
amended_by: []
sources: []
---

# Permanently select Electron as Ordo's desktop shell

> **Superseded by Decision [252](0252-replace-electron-with-a-clean-slate-rust-and-gpui.md).** Retained to explain the previous trade-off
> and why it was deliberately reopened after stronger evidence became
> available.

- Lock Electron + React as Ordo's production desktop architecture. Electron is no longer a provisional default and the implementation plan will not include a Tauri, GPUI, NativePHP, Wails, or other shell bake-off.
- Accept Electron's structural Chromium/Node memory and distribution overhead in exchange for one pinned Chromium generation across Windows and Linux, mature multi-window/display/fullscreen control, an established React toolchain, offline packaging, process/utility isolation, and the strongest low-risk path to future Chromium media support.
- Retain the `750 MB` preferred target, `1 GB` hard memory ceiling, `<50 ms` p95 Presenter-command target, rapid-input, idle-CPU, startup, and physical qualification gates. Framework selection does not excuse missing a performance budget; failure requires profiling, bounded caches, renderer-local hot paths, process correction, or an honestly revised supported hardware profile—not automatic framework reconsideration.
- Keep keydown-to-Preview and keydown-to-Program paint measurement in Milestone 0. The live command path remains an in-memory Presentation Engine transition plus narrowly subscribed operator and congregation updates, with no SQLite, filesystem, autosave, pagination, image decode, synchronous IPC, or broad timeline rerender on the path.
- Bound decoded full-resolution frames, keep operator thumbnails genuinely low-resolution and virtualized, preload only the nearby presentation set, release obsolete media, and measure memory by Electron process. Keep hardware acceleration enabled by default and treat software rendering only as a diagnosed compatibility fallback.
- Treat Discord as evidence that a heavily interactive React/Electron desktop product can be responsive, not as an Ordo benchmark. Ordo's own packaged measurements on the minimum-profile and actual church PC remain authoritative.
- Reject Tauri because its likely footprint advantage is unproven for Ordo's two-WebView workload and its WebView2-on-Windows/WebKitGTK-on-Linux split directly weakens canonical rendering parity. Reject GPUI because it replaces React with a pre-1.0 Rust UI rewrite and transfers far more cross-platform rendering/media ownership to Ordo. Reject NativePHP because its Desktop product still bundles Electron while adding Laravel/PHP runtime layers. Keep Wails outside the roadmap while its required multi-window generation remains prerelease and cross-engine.
- Preserve the Display Control, Storage, Presentation Engine, and Canonical Slide Renderer seams for Depth, Locality, process isolation, and testability. They do not signal a planned future shell migration.
- Primary research references: [Electron process model](https://www.electronjs.org/docs/latest/tutorial/process-model), [Electron performance guidance](https://www.electronjs.org/docs/latest/tutorial/performance), [Electron screen Interface](https://www.electronjs.org/docs/latest/api/screen/), [Tauri WebView versions](https://v2.tauri.app/reference/webview-versions/), [Tauri Windows installer/WebView2 choices](https://v2.tauri.app/distribute/windows-installer/), [GPUI status](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md), and [NativePHP Desktop building](https://nativephp.com/docs/desktop/2/publishing/building).
