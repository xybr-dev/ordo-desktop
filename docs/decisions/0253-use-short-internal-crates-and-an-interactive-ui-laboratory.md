---
id: 253
title: Use short internal crates and an interactive UI laboratory
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [platform]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Use short internal crates and an interactive UI laboratory

- Organize the initial production workspace into seven internal crates named
  `domain`, `storage`, `rendering`, `presentation`, `ui`, `platform`, and
  `ordo`. Do not repeat the `ordo-` product prefix on internal crate names;
  the workspace already supplies that context.
- Use `domain`, not `core`, for the pure Song, Mass Planning, Pagination, and
  related domain Modules. This avoids collision and ambiguity with Rust's
  built-in `core` crate and makes the crate's responsibility explicit.
- Make `ordo` the composition-root crate and the user-facing executable. It
  owns feature views and application wiring but no domain, persistence,
  canonical-rendering, or platform rules.
- Treat crates as dependency-enforcement tools rather than a folder for every
  future feature. Begin Portability and PowerPoint export as Modules or create
  their dedicated `portability` and `powerpoint` crates only at the milestone
  where their deep Interfaces justify the extraction. Otherwise keep the Module
  in the closest existing production crate that preserves dependency direction
  and update the affected milestone Changes paths before implementation. Do not
  scaffold pass-through crates merely to match a diagram.
- Keep the native `ui` Module internal to Ordo. Build its visual language from
  `DESIGN.md` on GPUI while using a pinned and qualified `gpui-base` for useful
  unstyled behavior. Do not adopt the styled `gpui-component` layer wholesale,
  and do not expose `gpui-base` properties through Ordo workflow Interfaces.
- Add a development-only `tools/ui-lab` binary that depends on `ui`, not on the
  complete application. It is the interactive proving ground for primitive
  states, themes, viewport presets, Interface scale, keyboard/focus behavior,
  overlays, accessibility semantics, long and multilingual content, and
  constrained layouts.
- Define UI-lab scenarios as reusable fixtures that can be opened manually and
  exercised by the GPUI automated UI harness. The lab is not shipped to parish
  users and must not become a second implementation of Ordo workflows or a
  general-purpose published component library.
- Keep workspace package names and directory names aligned. If a crate is ever
  intentionally published outside the workspace, reconsider its public package
  name at that time instead of burdening every internal import now.
