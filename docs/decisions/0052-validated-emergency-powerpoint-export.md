---
id: 52
title: Validated emergency PowerPoint export
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

# Validated emergency PowerPoint export

- Emergency PowerPoint export operates on an immutable Presentation snapshot and uses the same canonical renderer as Presenter Mode.
- Ordo generates the `.pptx` in a temporary location, then reopens and validates the completed deck before reporting export success.
- Validation checks slide count and order, canonical `1920x1080` full-frame images, speaker-note mapping for operator-only Cues, embedded-image integrity, and expected intentional black slides.
- If any projectable Cue cannot render or the completed deck fails validation, Ordo does not expose the new file as a successful export and does not overwrite a previous valid export. It identifies the affected Cues for correction.
- Renderable warnings may still be exported after acknowledgment, following the existing Mass-plan readiness policy.
- The deck contains actual PowerPoint sections in Mass order, with one section for each top-level Mass item. Its generated congregation slides and operator-only black slides remain grouped within their parent Mass-item section.
