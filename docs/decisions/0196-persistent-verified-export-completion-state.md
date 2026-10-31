---
id: 196
title: Persistent verified-export completion state
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage, transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Persistent verified-export completion state

- After successful destination read-back verification, keep the export slide-over open in a dedicated **Export complete** state rather than relying on a temporary toast or closing automatically.
- Show the filename, destination drive/path, size, Package version with an abbreviated fingerprint, **Verified** status, included Mass-plan count, and whether Ordo exported a new Package version or copied an existing content-equivalent version.
- Provide **Done** as the primary action, **Show in folder** as a secondary action, and **Copy to another location** as a reuse action that republishes the already verified artifact without rebuilding or creating another Package version.
- When the destination is removable, close all Ordo file handles and instruct the operator to use the operating system's Eject command before removal. Do not claim that closing the Ordo write is equivalent to a complete operating-system unmount or cache flush.
- Defer a custom cross-platform **Eject drive** action beyond the MVP. It is not required for artifact correctness and must not be approximated with an unreliable implementation.
