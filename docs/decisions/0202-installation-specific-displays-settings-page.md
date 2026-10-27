---
id: 202
title: Installation-specific Displays settings page
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, platform]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Installation-specific Displays settings page

- Provide one **Settings → Displays** page with a live topology diagram matching the operating system's actual left/right/top/bottom display arrangement and one detected-display tile showing number, name, resolution, aspect ratio, position, and recognition status.
- Mark the current operator display explicitly. Let the congregation display be one selected detected display or **Not assigned**, and save that choice immediately for the installation.
- Keep **Identify displays** as the primary diagnostic action and reuse the same display-picker interaction inside Presenter preflight.
- For the selected congregation display, show a **Calibration** section containing current Output Fit, the adopted `0–10%` uniform slider, canonical fitted Preview, **Show calibration pattern**, and the last calibration-check time.
- Never expose Mass content from Settings. Calibration opens the assigned output black first, shows only the calibration frame after explicit action, and returns to black before closing.
- A disconnected selected display becomes **Unavailable** without silent reassignment. With only one display, keep congregation display **Not assigned** while Presenter Mode remains available with congregation output disabled.
