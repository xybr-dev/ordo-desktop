---
id: 28
title: "Desktop shell, supported hardware, and performance budgets"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [platform]
supersedes: []
superseded_by: null
amends: []
amended_by: [252, 268]
sources: []
---

# Desktop shell, supported hardware, and performance budgets

> **Superseded in part by Decision [268](0268-make-ordo-windows-only-for-the-mvp-and-defer-linux.md).** Linux is no longer a supported preparation or development platform. The hardware and performance budgets remain.

> **Superseded in part by Decision [252](0252-replace-electron-with-a-clean-slate-rust-and-gpui.md).** The Electron selection, process model,
> and Electron-specific rationale below are retained as decision history. The
> hardware qualification and safety/performance outcomes remain applicable,
> except that Decision [252](0252-replace-electron-with-a-clean-slate-rust-and-gpui.md) replaces the Electron-era 750 MB preferred memory
> target with the native 512 MiB qualification budget and restates the
> historical `1 GB` ceiling as the current `1 GiB` absolute ceiling.

- Use Electron as Ordo's cross-platform desktop shell.
- Electron's bundled Chromium renderer is preferred over a system-webview shell because Ordo must minimize layout and rendering differences between Linux preparation and Windows presentation.
- The larger application and installer size is an accepted trade-off for a consistent bundled rendering engine, offline deployment, and mature multi-window/display control.
- Windows 10 22H2, 64-bit, is the minimum supported Windows installation for the MVP. The initial church PC runs Windows 10.
- Windows 11, 64-bit, is the recommended Windows platform for newly provisioned computers. Windows 10 reached Microsoft end of support on October 14, 2025, so Ordo must pin and qualify a tested Electron release for the air-gapped church PC instead of assuming all future Electron releases will continue to support it.
- Ordo targets x64 Windows packages. A machine merely capable of launching Electron is not necessarily an Ordo-supported live-presentation machine.
- The supported minimum church-PC hardware profile is:
  - Four logical CPU cores, approximately an Intel Core i3 fourth generation or equivalent.
  - `8 GB` RAM.
  - A hardware-accelerated DirectX 11-class integrated or discrete GPU capable of driving two simultaneous 16:9 displays.
  - An SSD with at least `2 GB` free for Ordo in addition to the parish content library.
- The recommended profile is an Intel Core i3/i5 eighth generation, AMD Ryzen 3, or newer equivalent; `8 GB` RAM for Ordo alone; and `16 GB` RAM when Ordo will share the machine with OBS or similarly demanding software.
- `4 GB` RAM is not an officially supported configuration even if Ordo happens to launch, because the operating system, Chromium processes, security software, drivers, and decoded media leave insufficient live-presentation safety margin.
- MVP performance is measured on a minimum-profile reference computer and must satisfy these release budgets:
  - Keep total Ordo memory below a `750 MB` target during a representative large Mass, with `1 GB` as the hard acceptance ceiling.
  - Average below `5%` CPU while idle on a static slide over a 30-second sample.
  - Make Left/Right, Freeze, Blackout, and Space visibly take effect within `50 ms` at the 95th percentile.
  - Lose no navigation input during rapid sequential operation.
  - Enter Presenter Mode from an existing Presentation snapshot within `5 seconds`.
  - Keep Program stable while search, autosave, pagination, thumbnail generation, or future-slide decoding occurs.
  - Scroll an approximately `800`-cue timeline smoothly at both required operator-screen test sizes.
  - Never disturb the current Program frame when a future slide fails to render or decode.
- Ordo must not retain an entire Mass as decoded full-resolution slide bitmaps. A single decoded `1920x1080` RGBA frame is approximately `7.9 MiB`, so an unbounded 500-slide cache would approach `4 GB` before application overhead.
- Presentation rendering therefore uses a bounded cache: only Program, Preview, and a small set of nearby slides remain presentation-ready; the cue timeline uses virtualized, lower-resolution canonical thumbnails and evicts offscreen decoded resources.
- The main Electron process must not perform blocking disk I/O, synchronous IPC, pagination, thumbnail generation, or other long-running work. CPU-heavy work moves off the UI path when measurement shows it is necessary.
- Electron is an adapter around framework-independent presentation-state and canonical-slide-composition modules. Song pagination, Mass-plan logic, snapshots, and live presentation semantics must not depend directly on Electron; this separation exists for Depth, Locality, deterministic tests, and safe process ownership, not because another production shell is planned.
- Linux remains a supported preparation and development platform.
- The local database implementation, installer formats, signing, and offline update workflow remain to be selected.
