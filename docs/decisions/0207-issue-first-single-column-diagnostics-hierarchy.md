---
id: 207
title: Issue-first single-column Diagnostics hierarchy
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

# Issue-first single-column Diagnostics hierarchy

- Present **Settings → Diagnostics** as an ordinary full Settings page, not a slide-over and not a dashboard-like mosaic of cards.
- Keep the page header compact: **Diagnostics**, the latest completed time, and **Run full diagnostics**. While scanning, replace that action with **Cancel** and place one thin overall progress indicator directly beneath the header.
- Lead the page body with a restrained full-width **Healthy** or **Needs attention** strip and a plain-language summary. When findings need action, place **Issues requiring attention** immediately below it; each issue explains the consequence and links to its owning workflow.
- Organize the remaining detail as collapsible bordered rows for **Data & assets**, **Fonts & rendering**, **Displays**, **Transfer & backups**, **Application & system**, and **Recent problems**. Every collapsed row exposes its status and finding count without requiring expansion.
- Place **Copy summary** and **Export diagnostic report** together after the diagnostic detail as secondary troubleshooting actions rather than competing with the primary scan command.
- Retain one readable column at `1366x768`. Wider layouts may increase usable detail within a row but must not convert the page into a generic multi-card dashboard.
