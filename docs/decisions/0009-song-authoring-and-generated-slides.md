---
id: 9
title: Song authoring and generated slides
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, rendering]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Song authoring and generated slides

- Use a hybrid model:
  - Within one Arrangement entry, the editor stores reusable sections such as verses, chorus, refrain, bridge, and coda once.
  - That Arrangement defines a performance sequence such as `V1 -> C -> V2 -> C`.
  - Ordo expands the arrangement into an ordinary sequential slide queue before presentation.
- Repeating a chorus must not require re-encoding or independently editing the same text several times.
- Exact musical punctuation, syllable division, repeated vowels, and intentional line breaks must be preserved.
- Separate Arrangement entries do not share line-level pagination maps. When the choir, seasonal form, wording, phrasing, or slide division is materially different, the operator creates a separate Arrangement entry with its own sections, sequence, and approved pagination.
- This permits some deliberate duplication across distinct Arrangements in exchange for simpler authoring and fewer hidden cross-arrangement dependencies.
