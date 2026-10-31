---
id: 185
title: Immutable versions under one Installed Celebration Package
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, transfer, platform]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Immutable versions under one Installed Celebration Package

- Use the Celebration UUID to group repeated exports of the same Celebration under one Installed Celebration Package. Give every export its own package-version UUID and complete artifact fingerprint; a package version is immutable after export.
- Treat a re-import whose package-version UUID and fingerprint are already installed as **Already installed** and make no duplicate package, version, snapshot, asset, or library change.
- After successful verification and library reconciliation, install a different export of the same Celebration as a new immutable package version and make it the **Current version**. Do not create a second Celebration merely because its package contents changed.
- Retain previous package versions in **Package history** with their exact plan set and per-plan snapshots. Deduplicate shared immutable assets by content checksum, and normally expose only the Current version in Masses so old packages do not appear as duplicate scheduled Celebrations.
- Permit deliberate inspection and restoration of a previous package version. Importing a version older than the Current version records it in Package history but never makes it current automatically.
- Treat a different Celebration UUID as a separate Installed Celebration Package even when date, title, metadata, or included Mass schedules appear similar; similarity may warn but does not redefine durable identity.
