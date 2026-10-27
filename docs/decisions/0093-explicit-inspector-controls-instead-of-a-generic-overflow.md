---
id: 93
title: Explicit Inspector controls instead of a generic overflow
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Explicit Inspector controls instead of a generic overflow

- The Inspector has no generic `...` menu in the MVP. Mixing Mass-item operations, Cue operations, view preferences, and navigation in one unlabeled overflow would make action ownership ambiguous.
- High-value Inspector actions remain visible and scoped: previous/next canonical slide with position, the inspected slide's warning state, item-level Theme override, and a contextual **Fix pagination** route to the owning editor.
- Mass-item actions remain in the Mass Outline row menu, while content editing remains in the selected-item editor. The Inspector does not duplicate either surface.
- If a later release gains several coherent slide-specific commands, it may introduce an accessibly named **Slide actions** menu. View preferences such as safe-area guides should likewise use an identifiable view control rather than a miscellaneous overflow.
