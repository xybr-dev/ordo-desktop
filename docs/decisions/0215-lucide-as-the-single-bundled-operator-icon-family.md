---
id: 215
title: Lucide as the single bundled operator icon family
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

# Lucide as the single bundled operator icon family

- Use Lucide as Ordo's sole general operator-interface icon family. Bundle the selected SVG modules locally, import only used icons, pin the dependency with the rest of the implementation toolchain, and preserve the applicable ISC/MIT notices in distributed third-party notices.
- Use `16px` icons inside ordinary controls and `20px` icons for navigation and prominent actions, with a consistent `2px` stroke across Preparation and Presenter appearances.
- Pair icons with visible labels for high-stakes or unfamiliar actions, explicitly including Live, Freeze, Blackout, Present, Restore, Delete, and congregation-display control. An icon never replaces the stable wording of an operational command.
- Limit icon-only controls to familiar compact actions such as Close, More, disclosure chevrons, and drag handles. Give every icon-only action an accessible name and a pointer tooltip that repeats the same name without becoming the sole accessible label.
- Keep ordinary icons outlined. Reserve filled shapes for status markers where the shape helps communicate non-color state.
- Allow a custom Ordo SVG only when Lucide cannot clearly express a domain-specific concept. Custom icons must follow Lucide's grid, stroke, size, and accessibility language; never mix Material, Font Awesome, emoji, or another general icon family into the interface.
