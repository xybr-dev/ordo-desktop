---
id: 56
title: Passive congregation window and cursor recovery
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Passive congregation window and cursor recovery

- The congregation window is a passive borderless output surface that stays above ordinary windows on its assigned display without taking keyboard focus from the operator workspace.
- It accepts no congregation-side controls or mouse interactions and presents no title bar, notifications, diagnostics, or error dialogs; failures remain on the operator screen and external output fails to black.
- The system pointer is hidden only while it is over the congregation window. It is neither hidden system-wide nor trapped, and it becomes visible normally after returning to the operator display.
- While the pointer is over the congregation display, the operator workspace immediately shows a prominent operator-only cursor locator.
- The locator derives return guidance from the operating system's actual display topology rather than assuming the projector is on the right. It may direct left, right, up, down, or diagonally as appropriate.
- The locator includes a compact display-layout diagram identifying the operator display, congregation display, current pointer location, and shortest direction back to the operator display; it disappears immediately when the pointer returns.
