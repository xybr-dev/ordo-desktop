---
id: 126
title: Atomic Sequence-occurrence reordering
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Atomic Sequence-occurrence reordering

- Reorder Sequence occurrences through visible drag handles or `Alt+Up` / `Alt+Down` while an occurrence is focused.
- Treat one occurrence as the smallest movable unit. Moving it carries every generated slide from its referenced section together and never changes their internal order.
- Keep generated-slide ordering owned by the reusable section in **Lyrics**. Do not expose individual slide reordering from Sequence.
- Autosave occurrence moves and make them recoverable through Undo while preserving the selected occurrence after the move.
- Keep multi-selection and bulk occurrence movement out of the MVP.
