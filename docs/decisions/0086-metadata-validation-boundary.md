---
id: 86
title: Metadata validation boundary
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

# Metadata validation boundary

- Creating a Celebration requires only its liturgical date and display title.
- Creating a Mass plan requires only its actual scheduled start and primary language. A parish may therefore defer Choir, Presider, Location, notes, and additional liturgical metadata without losing the draft.
- Season, color, and applicability tags are optional, but a conditional template rule that depends on missing metadata remains explicitly unresolved and contributes a readiness warning.
- Missing choir assignment leaves choir-dependent Song slots unresolved. Duplicate schedules warn without blocking deliberate creation.
- A scheduled date different from the Celebration date requires an explicit timing relationship such as **Anticipated**. Inconsistent relationships, such as an anticipated plan scheduled after its Celebration, are validation errors rather than readiness warnings.
