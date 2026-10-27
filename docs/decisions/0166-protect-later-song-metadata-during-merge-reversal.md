---
id: 166
title: Protect later Song metadata during merge reversal
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

# Protect later Song metadata during merge reversal

- Detect a reversal conflict whenever a Song metadata value changed by the merge has since been edited, removed, repurposed, or otherwise diverged from the merge event's recorded result.
- Show only the affected fields in a three-state comparison: **Before merge → Current → Reversal choice**. Preserve access to provenance without forcing the operator through unchanged fields.
- Require an explicit resolution for every overlapping field before enabling **Reverse merge**. Default each resolution to **Keep current** so a historical inverse never silently overwrites newer work; let the operator deliberately restore the pre-merge survivor value instead.
- Restore the retired Song's own original metadata from its sealed lineage independently of the survivor-side choice. Preserve source evidence and rejected attribution claims according to their recorded provenance.
- Apply the same conflict rule to canonical attribution, aliases, usual liturgical moments, occasion-applicability tags, and any later Song metadata introduced into the merge model.
