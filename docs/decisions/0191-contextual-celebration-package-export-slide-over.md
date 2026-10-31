---
id: 191
title: Contextual Celebration Package export slide-over
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, rendering, transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Contextual Celebration Package export slide-over

- Expose **Export Celebration Package** in each Celebration group's contextual `…` menu in **Masses**. Also expose **Export celebration** in **Transfer & Backup**, where the operator first selects a Celebration. Both entry points open the same export workflow.
- Use the adopted viewport-spanning floating right-side slide-over rather than a centered modal or dedicated full workspace. Import keeps its dedicated review workspace because reconciliation is substantially more complex than export.
- Identify the Celebration by title and date, then summarize either **New version — N changes** or **No changes — existing version will be copied**. Show every included Mass plan grouped chronologically, readiness findings, fatal blockers, estimated package size, and the selected destination.
- Keep the included plan list informational. Do not add plan checkboxes or partial-package controls: the artifact boundary remains the complete Celebration and every current Mass plan.
- Place detailed snapshots and dependencies under a collapsed **Package contents** disclosure so routine export remains scannable without hiding evidence needed for troubleshooting.
- Keep **Export package** as the persistent bottom primary action. Invoke the native file/folder chooser as part of that action, then publish the verified artifact atomically to the selected destination.
- Keep routine export close to the owning Celebration in Masses while **Transfer & Backup** remains the administrative home for Package history, verification details, restoration, removal, and global export/import entry points.
