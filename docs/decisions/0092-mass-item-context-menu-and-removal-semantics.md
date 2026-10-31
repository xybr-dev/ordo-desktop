---
id: 92
title: Mass-item context menu and removal semantics
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Mass-item context menu and removal semantics

- Right-clicking a Mass-item row and activating that row's hover-or-focus `...` affordance open the same scoped menu. The two entry points never expose different capabilities.
- The MVP menu contains **Add item above**, **Add item below**, **Duplicate item**, **Move up**, **Move down**, and a separated destructive-looking **Remove from this Mass** action. Boundary-inapplicable movement actions are disabled.
- **Remove from this Mass** removes only that plan's placement and plan-local content. It never deletes a referenced Song work, Arrangement, template, reading-library entry, or other reusable source content.
- Ordinary removal applies immediately without a confirmation dialog, autosaves, and presents a clearly named Undo action. Existing Presentation snapshots remain unchanged.
- The interface reserves **Delete** for actions that actually delete an owned record rather than using it for removal of a Mass-plan placement.
