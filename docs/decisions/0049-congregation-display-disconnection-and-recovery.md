---
id: 49
title: Congregation-display disconnection and recovery
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, storage]
supersedes: []
superseded_by: null
amends: []
amended_by: [272]
sources: []
---

# Congregation-display disconnection and recovery

> **Amended by Decision [272](0272-space-resumes-output-after-a-disconnect.md).** After a disconnect, `Space` resumes output in one step once the same recognized congregation display is back.

- If the active congregation display disappears during a Presentation session, Ordo immediately marks congregation output **Disconnected** and prevents its output window from being relocated onto the remaining operator display.
- The Presentation session, Preview selection, and navigation context remain active so the operator can continue preparing the next Cue.
- The Program monitor shows an unmistakable disconnected placeholder rather than claiming that the last Program frame is still visible to the congregation.
- Reconnecting the same display or attaching another display never resumes visible output automatically.
- The operator explicitly enables the congregation display after reconnection; external Program starts black.
- Pressing `Space` then restores the currently selected Preview to Program, following the established Blackout recovery workflow.
