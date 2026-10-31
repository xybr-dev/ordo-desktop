---
id: 200
title: Replace-only restore workspace with an exact protection gate
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Replace-only restore workspace with an exact protection gate

- Run **Restore parish backup** as a dedicated full-page workspace rather than a slide-over. Block entry while Presenter Mode or congregation output is active.
- Copy the selected `.ordo` to staging and verify its artifact kind, schema compatibility, checksums, asset closure, and stored-data integrity before exposing any destructive action or mutating current parish data.
- Compare **Current parish** and **Backup** side by side using parish name/UUID, data size, authoritative entity counts, latest activity versus Backup creation time, current installation, and source installation. Prominently warn when Parish UUIDs differ, while still permitting an intentional machine migration or parish replacement.
- State plainly that restore replaces the complete current Parish scope and never merges it. Use a centered final confirmation with **Keep current parish** initially focused and **Restore parish** as the destructive action; do not require typing the parish name.
- Keep **Restore parish** disabled until Ordo proves the current Parish scope is recoverable. Reverify an accessible Backup whose content fingerprint exactly matches the current authoritative state, or automatically create and verify a fresh locally managed safety Backup when any content differs.
- Do not accept age or a historical record of an unavailable portable Backup as protection evidence. A Backup must be currently accessible, content-equivalent, and pass verification immediately before replacement.
- If protection capture or verification fails, block restore. After protection succeeds, show **Current parish protected** with the Backup time and location.
- Restore into a new managed location, verify it completely, atomically switch Ordo to it, and restart into the restored Parish scope. Preserve the pre-restore safety Backup as an ordinary verified Parish Backup; returning to that state uses the same Restore workflow as any other Backup.
- Do not introduce **Undo restore**, a restore-reversal mode, parallel restored branches, or a special recovery artifact. A Backup is a Backup.
