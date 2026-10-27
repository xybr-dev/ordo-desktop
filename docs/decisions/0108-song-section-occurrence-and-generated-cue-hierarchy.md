---
id: 108
title: Song section-occurrence and generated-Cue hierarchy
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Song section-occurrence and generated-Cue hierarchy

- The Song item's section sequence is presented as ordered **section occurrences**, such as Verse 1, Chorus, Verse 2, Chorus. These remain the primary rows for understanding and changing the performance sequence.
- Expanding a section occurrence reveals its generated congregation-facing slide Cues in their exact playback order. Each Cue uses the same canonical rendered frame as the Inspector, scaled without recomposition.
- Two occurrences of a repeated section remain distinct positions in the sequence and therefore generate distinct ordered Cue occurrences, while visibly referencing the same linked section source unless one occurrence was explicitly forked.
- Reordering moves an entire section occurrence together with its generated Cues. Operators do not drag individual generated slides across section boundaries; text splitting and slide composition remain owned by the applicable reusable or Mass-local Lyrics editor.
- Selecting a generated Cue focuses that exact frame in the Inspector. Inspector previous/next navigation traverses the flattened Song Cue order and reveals the containing section occurrence when necessary.
- Provide one compact context-sensitive **Expand all / Collapse all** control in the section-sequence header. It changes only editor disclosure state, never the Arrangement, local override, Cue order, or saved pagination.
