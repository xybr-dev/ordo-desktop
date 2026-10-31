---
id: 155
title: Song merge rehomes but never combines Arrangements
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

# Song merge rehomes but never combines Arrangements

- Rehome every Arrangement owned by the retiring Song beneath the surviving Song while preserving the Arrangement's UUID, full Edit revision history, Choir associations, applicability metadata, archival state, and other identity.
- Preserve all Mass-plan, Presentation-snapshot, import, and other durable references to those Arrangements because their UUIDs and content remain intact; only their parent Song changes.
- Detect and flag exact or likely duplicate Arrangements for operator comparison, but do not line-merge or consolidate their lyrics, sections, sequence, pagination, metadata, or histories as part of Song merge.
- Keep compared Arrangements separate even when their current content appears identical. Song merge therefore means collecting independent Arrangements beneath one canonical Song work, not fusing Arrangement identities.
- Defer any dedicated Arrangement-merge operation beyond the MVP. This conservative boundary prevents accidental loss and keeps Song-merge reversal structurally reliable.
