---
id: 201
title: Safety Backups use ordinary manual retention in the MVP
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

# Safety Backups use ordinary manual retention in the MVP

- Keep automatic pre-migration, pre-upgrade, and pre-restore safety Backups as ordinary verified Parish Backups in **Transfer & Backup**.
- Do not create a separate recovery-point type, automatic retention schedule, or **Undo restore** interaction. Restoring a safety Backup uses the same replace-only Parish Backup workflow.
- Retain safety Backups until the operator deliberately deletes them. Show creation time, reason, size, and the consequence that deletion removes that recovery copy before confirming permanent removal.
- Defer age-based cleanup, count-based rotation, and configurable retention beyond the MVP. These operations are rare enough that manual cleanup is sufficient for initial deployment.
