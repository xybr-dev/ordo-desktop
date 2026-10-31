---
id: 42
title: Merging duplicate Song works
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

# Merging duplicate Song works

- Provide a reviewed **Merge Song works** action for duplicates discovered after import or ordinary library use.
- The operator explicitly chooses the surviving Song work; Ordo consolidates Arrangements, aliases, metadata, and library references while surfacing conflicting values for review.
- Existing Mass plans, history, and imported references continue to resolve after the merge.
- The retired UUID becomes a permanent identity redirect to the surviving Song work rather than being reused or discarded.
- Resolving retired identities is local to the Song-library Module, so callers do not each implement compatibility behavior.
- A merge creates a restorable library edit revision.
