---
id: 236
title: Editor-first adaptation for constrained Preparation workspaces
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

# Editor-first adaptation for constrained Preparation workspaces

- Retain the full primary-list-or-outline, editor, and Inspector composition at the qualified `1366x768` desktop size under ordinary `100%` Interface scale. Wider space may increase useful preview and editing area but does not change ownership among those regions.
- Base adaptation on effective available workspace dimensions after operating-system scaling, Ordo Interface scale, window resizing, navigation chrome, and minimum usable logical pane widths of `240px` for primary list/outline, `520px` for editor, and `300px` for Inspector—not on physical monitor resolution alone.
- As horizontal space first becomes constrained, preserve the primary list or Mass outline beside the editor and turn the Inspector into a labeled right-side slideover. At narrower effective widths, preserve only the main editor and expose the primary list/outline and Inspector as opposing labeled slideovers.
- Permit only one temporary side pane to be open at a time in the constrained layout. Opening the other replaces it without losing selection, scroll position, editor state, or autosaved content; dismissal returns focus to its opener or the selected relevant entity.
- Never introduce whole-workspace horizontal scrolling, squeeze the editor beneath its usable content minimum, clip primary actions, or hide selection context merely to retain nominal three-pane geometry. Keep the current entity/title visible in compact workspace chrome.
- Keep the workspace's primary action visible and labeled where practical. Lower-priority actions may move into a clearly labeled overflow menu while preserving their wording, availability reason, keyboard access, and focus restoration.
- Treat layout mode as responsive presentation state, not persisted Parish content and not a separate editor mode. Returning to sufficient width restores fixed panes and their prior meaningful focus/scroll targets without reopening temporary slideovers.
