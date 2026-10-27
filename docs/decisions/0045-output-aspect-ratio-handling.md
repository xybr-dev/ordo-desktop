---
id: 45
title: Output aspect-ratio handling
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Output aspect-ratio handling

- The canonical congregation-facing frame remains `16:9` and is never stretched or cropped to fill a mismatched physical output.
- On a `16:9` display, Program fills the available output.
- On any other display aspect ratio, Ordo centers the complete canonical frame and adds black letterboxing or pillarboxing as required.
- Presenter Mode warns the operator when the selected congregation display does not match `16:9`, but the mismatch does not prevent presentation.
- Preview includes the same fitted frame and black bars shown by Program, preserving the requirement that the operator preview and congregation output be visually identical.
- Keep display fitting inside the Presentation Engine's output Interface so every physical-display Adapter receives the same deterministic composition.
