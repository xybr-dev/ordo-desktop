---
id: 101
title: Dirty Theme session and navigation-away behavior
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [themes, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Dirty Theme session and navigation-away behavior

- Any staged Theme change that differs from the loaded revision places the Theme Editor in a visible **Unsaved changes** state.
- **Save** and `Ctrl+S` use the same impact-warning, revision creation, propagation, feedback, and Undo behavior; the shortcut does not bypass shared-Theme safeguards.
- Back, close, or any navigation away from a dirty Theme session opens a compact confirmation with exactly three outcomes: **Save changes**, **Discard changes**, or **Keep editing**.
- **Save changes** completes the ordinary Save flow before navigating. **Discard changes** abandons only the staged editor state and leaves the saved Theme and inheriting plans unchanged. **Keep editing** cancels navigation and returns focus to the Theme Editor.
- Successfully using **Duplicate Theme** publishes the staged result as a new independent Theme and resolves the dirty source-editing session. Returning to the source or Mass Theme chooser must not then ask the operator to discard the same changes.
- A clean Theme Editor session exits without confirmation.
