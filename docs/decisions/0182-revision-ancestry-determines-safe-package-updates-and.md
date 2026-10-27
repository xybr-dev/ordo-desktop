---
id: 182
title: Revision ancestry determines safe package updates and divergence
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

# Revision ancestry determines safe package updates and divergence

- Never infer cross-computer freshness from a visible revision number or timestamp alone. Give each Edit revision its own stable UUID, parent revision UUID, deterministic content hash, creation time, and origin-installation UUID in addition to the durable entity UUID.
- Record the latest revision known to be shared after a reviewed import as that mapped entity's **sync anchor**. Include compact revision identifiers, hashes, and parent relationships in the Celebration Package so the receiving installation can locate a common ancestor without carrying every historical revision's complete content.
- Treat equal revision UUIDs as unchanged. Treat different revision UUIDs with equal canonical content hashes as content-equivalent rather than manufacturing a conflict.
- Permit an automatic safe update only when the incoming state changed after the common revision and the local current content remains effectively equal to that common state. Preserve a locally changed state when the incoming side did not change.
- Require review when both current states changed after their common revision, when the same entity UUID has no provable common revision, or when lineage is missing or inconsistent. The latter is an identity/integrity concern, not an ordinary fast-forward.
- Keep every displaced or rejected local branch recoverable in History. Choosing an incoming state changes the current editable state through a new/imported restorable revision; it does not erase the local branch.
- If a reviewed decision keeps an incoming record as a separate local record, generate a new local UUID and retain its source-to-local mapping for succeeding imports.
