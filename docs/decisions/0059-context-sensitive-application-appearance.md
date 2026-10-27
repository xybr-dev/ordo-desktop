---
id: 59
title: Context-sensitive application appearance
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: [256]
sources: []
---

# Context-sensitive application appearance

> **Superseded in part by Decision [256](0256-refine-operator-appearance-overlays-and-bounded-output.md).** Preparation now offers System
> (default), Light, and Dark instead of deferring manual selection. Presenter
> remains forced graphite-dark and operator appearance stays independent from
> congregation Themes.

- The MVP uses two deliberate appearance polarities selected by workspace rather than one appearance across the entire application.
- Preparation workspaces use a warm, paper-neutral light appearance suited to prolonged library management, planning, arrangement editing, pagination correction, and Theme preparation.
- Presenter Mode uses a graphite-dark appearance suited to a dim church and designed to keep attention on canonical Preview and Program frames.
- Both polarities share one visual system: the same operator typography, geometry, spacing rhythm, control hierarchy, icon rules, focus treatment, and operational state semantics. They are working environments within one product, not unrelated Themes.
- Liturgical colors belong to congregation Themes and their canonical previews. They do not tint the global operator shell, navigation, or ordinary controls.
- Entering or leaving Presenter Mode selects the corresponding appearance automatically. Manual application-appearance selection is deferred beyond the MVP.
- This application appearance decision is independent of congregation slide Themes; changing one never mutates or visually reinterprets the other.
