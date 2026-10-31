---
id: 140
title: Fine-grained durable revisions with grouped History
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Fine-grained durable revisions with grouped History

- Persist every committed Song or Arrangement Undo point as a durable Edit revision, including each coalesced text-edit session and each discrete structural operation. Do not reserve recoverable history only for manually named checkpoints.
- Preserve the fine-grained revisions underneath the interface so recovery can reach a specific intermediate state after navigation, restart, or interruption.
- Group consecutive related revisions from one editing session into a readable History entry, such as **Edited lyrics · 8 changes**, rather than presenting a flat row for every small edit.
- Let the operator expand a grouped session to inspect and restore its individual revisions when finer recovery is necessary.
- Show creation, import, whole-Arrangement replacement, and historical restoration as explicit standalone History events because they represent meaningful provenance or safety boundaries.
- Retain all Song and Arrangement Edit revisions in the MVP. Do not add automatic pruning or revision-retention settings before real parish usage demonstrates a storage problem.
