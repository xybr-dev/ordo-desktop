---
id: 38
title: Stable entity identity
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Stable entity identity

- A Song title is not a unique identifier. Distinct Song works may share a title, and one Song work may be known under several spellings, translations, or local names.
- Durable Ordo entities use UUID version 4 stable identities generated with Node's built-in cryptographically random `crypto.randomUUID()` implementation.
- Store UUIDs in their canonical lowercase textual representation for transparent SQL inspection and portable JSON interchange; parish-scale storage does not justify a more opaque binary representation.
- Keep creation timestamps as explicit fields rather than attempting to infer chronology from identifiers.
- Enforce UUID uniqueness in storage and regenerate if insertion ever reports a collision.
- Stable identities are suitable for offline Celebration Packages, import conflict detection, and possible future same-parish synchronization, while remaining invisible to ordinary operators.
- Creating or importing a likely duplicate never merges automatically. Ordo presents possible matches so the operator can attach a new Arrangement to an existing Song work or deliberately create a separate Song work.
