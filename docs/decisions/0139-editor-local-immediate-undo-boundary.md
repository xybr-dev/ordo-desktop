---
id: 139
title: Editor-local immediate Undo boundary
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Editor-local immediate Undo boundary

- Scope immediate Undo and Redo to the currently open Song or Arrangement editor session. Never let an ordinary `Ctrl+Z` reach into a different Song or Arrangement that the operator is not viewing.
- While a text field owns focus, use that field's native text Undo and Redo first so character- and selection-level corrections behave normally during the active editing session.
- Add persisted text-edit sessions and structural Arrangement operations to the current editor's application-level Undo stack. Temporary **Undo** feedback and `Ctrl+Z` address the same editor-local history after an operation commits.
- Clear the immediate editor-session Undo stack when the operator navigates away from that Song or Arrangement or restarts Ordo. Do not make an unseen prior entity the target of a later shortcut.
- Use durable Edit revisions and the explicit **History** surface for recovery after navigation, restart, or older changes. Durable restoration creates a new latest revision rather than rewriting historical states.
