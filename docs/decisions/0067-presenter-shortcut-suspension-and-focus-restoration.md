---
id: 67
title: Presenter shortcut suspension and focus restoration
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Presenter shortcut suspension and focus restoration

- Passive Presenter surfaces such as the Ordo rail and shortcut-reference panel do not suspend the live `F`, `B`, or `Space` commands. Merely inspecting navigation or help must not make these essential controls unavailable.
- While a text field, emergency editor, or modal dialog owns focus, every single-key Presenter command is suspended, including Cue navigation, `F`, `B`, `O`, and `Space`. Typed content can therefore never trigger a presentation transition.
- `Esc` is handled by the topmost editor, dialog, or overlay first. It closes or begins that surface's cancel flow without altering Program.
- Emergency Insert and Emergency Edit Freeze Program before their editing surface accepts input, so shortcut suspension cannot cause the current output to advance inadvertently.
- Saving, cancelling, or closing the surface restores focus to the previously active Cue unless a deliberate action selected a different Cue.
- The shortcut-reference surface explains when live commands are suspended; a visible editing/modal state must not leave the operator guessing why a shortcut is inactive.
