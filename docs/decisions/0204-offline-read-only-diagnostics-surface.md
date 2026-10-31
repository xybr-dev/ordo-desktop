---
id: 204
title: Offline read-only Diagnostics surface
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Offline read-only Diagnostics surface

- Provide one **Settings → Diagnostics** page that works completely offline and summarizes installation health as **Healthy** or **Needs attention** without requiring telemetry, an account, or internet access.
- Report the Ordo application version and build; operating system, CPU, memory, GPU, and rendering environment; database and schema version with integrity status; managed-asset availability and corruption findings; bundled Atkinson and Noto font availability; storage location and free space; congregation-display assignment and calibration state; latest verified Parish Backup and Celebration Package activity; and evidence of a recent crash or interrupted Presentation session.
- Make **Run diagnostics** a fresh, non-mutating inspection. The Diagnostics page must not silently repair, rewrite, migrate, prune, or otherwise alter parish data.
- Keep findings operator-readable and grouped by owning concern. Where an ordinary corrective workflow exists, link the finding to its proper owner such as **Displays**, **Transfer & Backup**, or the affected library entry rather than duplicating repair controls inside Diagnostics.
- Keep Diagnostics operator-only. Running a check must never open congregation output or expose diagnostic content on Program.
- Centralize collection and normalization behind a deep **Diagnostics Module** whose small result interface supplies the Settings page and any later diagnostic-report exporter. Individual UI panels must not inspect storage, assets, fonts, displays, or runtime internals independently.
