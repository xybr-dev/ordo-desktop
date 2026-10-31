---
id: 154
title: Song-merge metadata resolution
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

# Song-merge metadata resolution

- Keep the surviving Song work's primary title unchanged. Convert the retiring Song work's primary title into an alias of the survivor.
- Automatically take the union of aliases, usual liturgical moments, and occasion-applicability tags, normalizing and deduplicating textual values case-insensitively.
- Collapse exact metadata matches without asking the operator to resolve a false conflict.
- Preserve source and provenance records as distinct entries rather than overwriting or flattening one source into another.
- Treat conflicting composer or original-author values as blocking identity conflicts. Require an explicit choice of the canonical value before merge commit; never automatically concatenate attribution facts or silently prefer the survivor.
- Keep all metadata work staged and reversible during review. Automatic resolutions are visible before commit and may not mutate either Song merely because the review was opened.
