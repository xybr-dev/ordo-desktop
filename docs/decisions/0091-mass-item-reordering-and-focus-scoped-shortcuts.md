---
id: 91
title: Mass-item reordering and focus-scoped shortcuts
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Mass-item reordering and focus-scoped shortcuts

- Every Mass item is freely movable. A template provides a suitable starting order, but Ordo does not lock operators into that order or pretend to enforce complete liturgical ordering rules.
- Pointer reordering begins only from a dedicated drag handle; clicking the remainder of the outline row continues to select the item. Dragging shows a precise insertion line, supports scrolling through the outline, and leaves the moved item selected and visible.
- With focus on a Mass-item row or its handle inside the **Mass Outline**, `Alt+Up` and `Alt+Down` move that single item by one position. Focus follows the moved row so repeated presses remain predictable.
- The shortcut is inactive while focus is in a text field, rich-text editor, dropdown, dialog, slide-over, middle editor, Inspector, or Presenter Mode. It is not a global command acting merely on whichever item happens to be selected.
- Other reorderable editors may later use the same gesture only when focus is inside their own list. The focused list therefore determines the entity being moved.
- Reordering autosaves, participates in Undo, and never moves multiple items at once in the MVP.
