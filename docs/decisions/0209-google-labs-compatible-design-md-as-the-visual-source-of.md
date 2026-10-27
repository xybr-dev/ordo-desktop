---
id: 209
title: Google Labs-compatible DESIGN.md as the visual source of truth
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Google Labs-compatible DESIGN.md as the visual source of truth

- Maintain root-level `DESIGN.md` using the Google Labs `design.md` alpha format: normative machine-readable YAML design tokens followed by human-readable Markdown rationale and usage guidance.
- Make `DESIGN.md` the canonical contract for reusable visual identity, appearances, tokens, layout language, elevation, shapes, component styling, and visual do/don't guidance. Keep domain vocabulary in `CONTEXT.md`, historical rationale in `decision.md`, and product behavior plus implementation scope in `MVP-PLAN.md`.
- Keep `design-references/` as approved interactive evidence rather than production source or a competing specification. Link references from `DESIGN.md`; when a visual reference conflicts with the written contracts, the written contract wins.
- Use the format's canonical sections first and add Ordo-specific sections only where necessary. Unknown-section extensibility permits future iconography, interaction-state, motion, accessibility, responsive-qualification, Presenter-safety, and reference guidance without abandoning the format.
- Do not invent normative token values for unresolved branches. Declare intentional omissions in the YAML front matter and fill them as focus, geometry, iconography, spacing/density, and motion decisions are adopted.
- Treat the external format and CLI as alpha. When implementation begins, pin an exact `@google/design.md` version, run lint and contrast validation through the cross-platform `designmd` command, and review format upgrades deliberately rather than tracking latest automatically.
