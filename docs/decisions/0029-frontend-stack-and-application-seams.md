---
id: 29
title: Frontend stack and application seams
status: superseded
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: 252
amends: []
amended_by: []
sources: []
---

# Frontend stack and application seams

> **Superseded by Decision [252](0252-replace-electron-with-a-clean-slate-rust-and-gpui.md).** Retained as the historical React/Electron
> design considered before the clean-slate GPUI decision.

- Use React with TypeScript for the operator and congregation renderer interfaces.
- Use Vite to build the Electron renderer applications.
- Do not add Next.js, server-side rendering, or another web-server application framework. Ordo is an offline desktop application whose renderer assets are bundled locally.
- React's larger ecosystem for virtualized timelines and grids, accessible controls, keyboard interaction, arrangement editing, testing, and profiling outweighs Svelte's smaller UI runtime for this product.
- Electron's Chromium processes, decoded media policy, bounded caches, timeline virtualization, and update locality have materially greater performance impact than the difference between React and Svelte.
- React Compiler may be enabled after its compatibility with the chosen Electron/Vite toolchain is verified; the MVP architecture and performance budgets must not depend on it.
- React owns view composition and operator interaction only. It must not own the business rules for Songs, Arrangements, Mass plans, pagination, snapshots, or live presentation state.
- The Presentation Engine is a framework-independent TypeScript module behind a small command-and-state interface covering navigation, Freeze, Blackout, taking Preview live, and recovery.
- The Canonical Slide Renderer is shared by the timeline, Preview, Program, congregation window, and fallback export paths so all consumers receive the same canonical composition rather than independently approximating it.
- Electron main/preload code supplies operating-system adapters for window/display control, durable storage, filesystem access, shortcuts, and process coordination through narrow IPC interfaces.
- Keeping these seams independent of React and Electron preserves testability and prevents desktop-framework concerns from leaking into Ordo's domain and presentation semantics. It is not a commitment to support or migrate to another shell.
