---
id: 232
title: Discoverable contextual keyboard-shortcut reference
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Discoverable contextual keyboard-shortcut reference

- Expose a labeled **Keyboard shortcuts** entry under Help in Preparation and in the Presenter Ordo-rail footer. Do not make discovery depend on memorizing a shortcut or interpreting an unlabeled icon.
- Use `Ctrl+/` to toggle the shortcut reference across Ordo. Do not use a bare `?`, which conflicts with text entry and varies awkwardly across keyboard layouts.
- Open the reference as a viewport-bounded floating right panel rather than a centered modal so the current workspace and operational context remain recognizable.
- Group commands under **Global**, the current workspace, **Editing**, and **Presenter**, placing the current context first. Display exact keycaps with concise stable action wording; omit shortcut search and customization from the MVP.
- In Presenter Mode, keep the shortcut reference passive: opening it never changes Program, and `F`, `B`, and `Space` retain their adopted presentation meanings. A text editor, emergency editor, or true modal continues suspending every single-key Presenter command.
- Let `Esc` close the reference and restore focus to the previously active Cue or control. Reuse one Shortcut Reference Module result across Help and Presenter so visible bindings cannot drift from registered commands.
