---
id: 244
title: Keep completed qualification evidence local and privacy-safe
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [repo]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Keep completed qualification evidence local and privacy-safe

- Commit only a blank, reusable physical-qualification report template to the source repository. Do not commit completed parish qualification reports.
- Store a completed report locally beside the exact qualified release bundle and its checksums so the evidence and tested binaries remain traceable together on the air-gapped deployment path.
- Capture the build identity/hash, published checksum, date, operator, relevant PC/Windows specifications, operating-system scale, operator and congregation display modes/topology, cable or adapter path, qualification fixture or Package hash, performance observations, both pass results, failures, corrections, reruns, and final sign-off.
- Exclude parish-authored content, congregation photographs or recordings, personal data beyond the deliberately named tester, and unsanitized diagnostics. Evidence screenshots, if useful, must contain only the operator's qualification fixture or sanitized diagnostic/calibration material.
- If Ordo later becomes open source, publish at most a sanitized qualification summary. Retain the full local report and church-specific topology outside source hosting.
