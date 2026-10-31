---
id: 224
title: Coherent degraded rendering for a missing Theme background
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [themes, rendering]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Coherent degraded rendering for a missing Theme background

- When a saved Theme background image is missing or corrupt, render that Theme's required fallback color immediately. Never expose a broken-image symbol, partial composition, or transparent accident to congregation output.
- Mark the reusable Theme **Degraded** and surface one deduplicated Mass Readiness warning wherever that Theme is inherited. Keep the warning advisory because the authored fallback color produces complete projectable output.
- In the Theme Editor, preserve the missing asset reference and show a clear placeholder with **Replace image** and **Remove image**. Never silently clear or rewrite the saved Theme merely because an asset lookup failed.
- Permit Presenter Mode after the ordinary warning acknowledgment. Permit a new local Presentation snapshot to capture the complete fallback-color result while recording the degradation in snapshot diagnostics and readiness evidence.
- Leave every previously validated immutable Presentation snapshot unchanged and available according to its own verified dependency state.
- Block Celebration Package export until the missing referenced image is replaced or deliberately removed. A locally presentable fallback does not satisfy the Package's promise to contain complete editable dependencies.
- Route detection and fallback rendering through the Theme and Slide Renderer Modules, while diagnostics and editor repair actions consume their normalized degraded-asset result rather than independently guessing asset health.
