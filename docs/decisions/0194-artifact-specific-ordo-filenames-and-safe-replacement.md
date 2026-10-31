---
id: 194
title: "Artifact-specific `.ordo` filenames and safe replacement"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Artifact-specific `.ordo` filenames and safe replacement

- Generate human-readable default `.ordo` filenames according to artifact kind rather than encoding technical manifest data into every name. Examples include `2026-08-23_21st-Sunday-in-Ordinary-Time.ordo`, `St-Joseph-Parish_Backup_2026-08-24_1930.ordo`, `Amahan-Namo_Walter-Arrangement.ordo`, and `2026-08-23_Cebuano-Readings.ordo`.
- Sanitize generated names for supported Windows, Linux, and common removable-drive filesystems. Keep durable identity, artifact kind, schema version, and immutable Package-version identity inside the verified manifest; never infer them from the visible filename.
- For repeated Celebration exports, propose the same date-and-title filename so the current file remains easy to locate on removable media. Do not append UUIDs, revision numbers, or timestamps by default.
- When the destination already exists, rely on the native chooser's explicit replacement confirmation and let the operator rename the new file when both copies are desired.
- Build and verify a temporary sibling artifact before replacing the destination. Publish the replacement atomically where the destination filesystem permits and otherwise use a recoverable replacement sequence; any failure must leave the previous destination file intact or restored.
- A content-equivalent export copies the exact retained immutable Package artifact under the chosen filename; it does not regenerate payload bytes or create another Package version.
