---
id: 252
title: Replace Electron with a clean-slate Rust and GPUI implementation
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [platform]
supersedes: [29, 58, 249]
superseded_by: null
amends: [28, 30, 31, 33]
amended_by: [268]
sources: []
---

# Replace Electron with a clean-slate Rust and GPUI implementation

> **Superseded in part by Decision [268](0268-make-ordo-windows-only-for-the-mvp-and-defer-linux.md).** Supported Linux preparation and the Linux Flatpak/ZIP are deferred post-MVP. Its cross-platform glyph, wrap, pagination, and frame parity gate becomes Windows-only invariance across scale and density. The rest of this decision is unchanged.

- This supersedes Decisions [28](0028-desktop-shell-supported-hardware-and-performance-budgets.md)–[31](0031-windows-installation-scope.md), [33](0033-source-hosting-and-cross-platform-builds.md), [58](0058-react-ui-foundation-and-ownership.md), and [249](0249-permanently-select-electron-as-ordo-s-desktop-shell.md) to the exact full/partial
  extents recorded by the forward notice in each affected section.
- Lock Rust plus Zed's GPUI as Ordo's production desktop architecture for the
  MVP. Electron, React, TypeScript, Vite, Tailwind, shadcn/Base UI, Chromium,
  Node.js, preload/IPC, and Electron Forge are no longer part of the intended
  implementation.
- Treat this as a deliberate supersession of Decision [249](0249-permanently-select-electron-as-ordo-s-desktop-shell.md), not an additional
  shell experiment or a dual-frontend strategy. The implementation plan will
  contain no Electron fallback application and no ongoing GPUI-versus-Electron
  bake-off.
- Restart development from a clean repository and source tree after this
  planning baseline. The committed production repository and its current
  `MVP-PLAN.md`, `CONTEXT.md`, `DESIGN.md`, and upstream pins are authoritative.
- Use a Cargo workspace with pure Rust domain Modules and explicit native
  Adapters. Keep Song Library, Mass Planning, Pagination, Presentation Engine,
  Canonical Slide Renderer, Storage, Portability, PowerPoint export, Display
  Control, and Presenter Preflight behind the already adopted small,
  domain-shaped Interfaces.
- Pin the Rust toolchain, Cargo dependency graph, GPUI/gpui-platform revision,
  and any optional GPUI behavior library used by a release. GPUI is pre-1.0;
  upgrades are scheduled compatibility work with qualification evidence, not
  an automatic move to upstream `main`.
- Build an Ordo-owned native UI foundation directly on GPUI. Translate the
  accepted `DESIGN.md` tokens and HTML acceptance references conceptually;
  HTML, CSS, Tailwind classes, React components, and shadcn source are visual or
  behavioral references only and are not production dependencies.
- Third-party GPUI libraries such as `gpui-base` or `gpui-component` may be
  evaluated for difficult low-level behavior, but adoption requires an exact
  pinned revision, license review, native accessibility/focus tests, and a
  contained Adapter that does not expose library properties through Ordo's
  visible workflow Interfaces. No such library is assumed until qualified.
- Use native/server operating-system window decorations for the Preparation
  window in the MVP. Presenter Mode remains borderless full-screen. Defer a
  custom client-side titlebar unless native qualification demonstrates an
  operational need.
- Run the application as one native process while keeping the Presenter hot
  path in memory and free from database, filesystem, pagination, image decode,
  autosave, or other blocking work. Move SQLite ownership and CPU-heavy jobs to
  dedicated Rust worker threads/tasks behind asynchronous, typed Module
  Interfaces; do not reproduce IPC merely to mimic Electron's topology.
- Use SQLite through a pinned Rust driver behind the Storage Module. Qualify the
  concrete driver in Phase 0; the planned baseline is `rusqlite` with a single
  owning worker thread, explicit SQL, versioned migrations, WAL/recovery policy,
  and domain-shaped requests. Callers never receive raw connections or SQL.
- Keep one shared Canonical Slide Renderer for timeline thumbnails, Inspector,
  Preview, Program, congregation output, snapshots, and PowerPoint frames. Its
  `1920x1080` logical composition and bundled-font shaping must be independent
  of GPUI operator layout and operating-system scaling. Cross-platform glyph,
  wrap, pagination, and frame parity is the principal Phase 0 technical gate.
- Replace browser/Electron UI testing with pure Rust Module tests, GPUI
  `TestAppContext` and internal action/render harnesses, golden-frame tests,
  native packaged smoke tests, scripted accessibility checks, and physical
  projector qualification. Do not pretend external desktop automation is
  reliable where GPUI/AccessKit cannot yet expose stable automation identity.
- Preserve the existing release outcomes: native x64 Windows 10/11 support,
  supported Linux preparation, conventional offline Windows installation,
  Linux Flatpak plus diagnostic ZIP, no runtime internet requirement, and
  native CI builds. Replace Electron Forge/npm packaging with pinned Cargo and
  platform-native packaging scripts.
- Retain the adopted `<50 ms` p95 Presenter command goal, rapid-input safety,
  idle-CPU gate, bounded frame caches, and `1 GiB` absolute memory ceiling.
  Adopt a `512 MiB` total-process qualification budget for every representative
  Phase 0 run; exceeding either limit blocks Phase 1 until an explicit budget
  and native-memory rationale revision is adopted.
- Keep the existing `8 GB` church-PC profile as the conservative initial
  qualification machine, not as proof that GPUI requires 8 GB. Decision 252
  removes Chromium as the reason for rejecting 4 GB; support for a lower-memory
  machine may be added only after the same representative Presenter, recovery,
  and physical safety gates pass with adequate operating-system and GPU
  headroom.
