---
id: 15
title: Emergency PowerPoint export
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Emergency PowerPoint export

- MVP must export the current Mass plan as a static `.pptx` emergency deck.
- Congregation-facing slides preserve the exact canonical Ordo rendering.
- Each congregation slide is exported as a full-frame image rather than editable PowerPoint text boxes, preventing font substitution, text reflow, and background/layout differences.
- Operator-only cues become black slides with private cue text in speaker notes.
- Generated arrangements are fully expanded, including repeated song sections.
- The deck is an immutable fallback snapshot; later Ordo changes require a new export.
