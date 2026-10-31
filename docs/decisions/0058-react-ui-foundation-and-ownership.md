---
id: 58
title: React UI foundation and ownership
status: superseded
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: 252
amends: []
amended_by: []
sources: []
---

# React UI foundation and ownership

> **Superseded by Decision [252](0252-replace-electron-with-a-clean-slate-rust-and-gpui.md).** The visual and interaction ownership
> principles remain, but the shadcn/Base UI/React implementation does not.

- Adopt shadcn/ui as a source-owned primitive foundation, using its Base UI-based React implementations rather than treating shadcn as Ordo's visual identity.
- Commit selected generated primitive source into the Ordo repository and customize it deliberately. Do not import stock dashboard blocks or allow future regeneration to overwrite reviewed Ordo changes blindly.
- Use Tailwind CSS for compiled application styling, with Ordo-owned semantic tokens as the canonical source of color, typography, spacing, density, geometry, focus, and motion decisions.
- Base UI supplies low-level accessible behavior for appropriate controls such as dialogs, menus, tooltips, popovers, comboboxes, collapsibles, and form inputs. All assets, CSS, icons, and fonts are bundled locally; the installed application has no runtime CDN or registry dependency.
- Visible operator workflows remain Ordo-owned deep Modules. In particular, Presenter Workspace, Ordo Rail, Cue Timeline, Output Dock, Mass Planning Workspace, Song Library Workspace, Arrangement Editor, Theme Editor, Display Setup, and emergency-change workflows expose domain-shaped Interfaces rather than raw shadcn or Base UI properties.
- Canonical slide rendering, virtualized canonical thumbnails, pagination visualization, arrangement sequencing, display topology, cursor recovery, calibration, and Theme composition remain custom rendering or interaction surfaces. Wrapping these surfaces in generic cards would add no Depth.
- Avoid the stock visual grammar associated with generic generated dashboards: excessive rounded cards, nested panels, decorative gradients, ambient glows, oversized pills, broad gray-on-gray surfaces, and arbitrary product accents.
- Base UI's keyboard, focus, portal, overlay, and nested-control behavior must be qualified in Milestone 0. Radix remains a possible replacement Adapter beneath the source-owned primitive layer if Base UI fails those packaged Windows/Linux interaction tests; this must not change Ordo-owned Module Interfaces.
- Do not add shallow Ordo wrappers around every primitive. Create an Ordo Module only when it hides meaningful domain or interaction complexity and therefore earns Leverage and Locality.
