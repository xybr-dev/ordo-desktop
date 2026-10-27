---
id: 111
title: Response-sequence exchange blocks
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

# Response-sequence exchange blocks

- Build a Response-sequence item from ordered **exchange blocks** rather than an unstructured mixture of Cue rows.
- Each exchange block contains an optional operator-only trigger followed by one required congregation response. Priest (`P`) and congregation (`T`) are the defaults, while both roles remain explicitly editable for deacon, lector, cantor, commentator, and similar exchanges.
- Flatten each block into Presenter order as `operator-only trigger → generated response slide Cue(s)`. When no trigger is authored, the response Cue(s) enter the sequence directly.
- Selecting an operator-only trigger uses the established black Preview while showing its private prompt and role to the operator; advancing selects the paired projectable response.
- Show the response's generated canonical slide Cue or Cues beneath its exchange block, using the same Inspector selection and owner-directed pagination rules as other projectable content.
- Keep exceptional workflows that require arbitrary mixtures, control Cues, or non-paired content in **Custom sequence** instead of weakening the Response-sequence editor's focused model.
