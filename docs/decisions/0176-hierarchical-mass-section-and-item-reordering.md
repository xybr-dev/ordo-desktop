---
id: 176
title: Hierarchical Mass-section and item reordering
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

# Hierarchical Mass-section and item reordering

- Give each Mass-section header a dedicated drag handle. Moving a section moves the complete section and every Mass item it contains as one ordered unit.
- With focus on a section handle in the Mass Outline, let `Alt+Up` and `Alt+Down` move that whole section by one section; retain the existing focus-scoping and text-input exclusions.
- Let a Mass item move across section boundaries by pointer or keyboard. Permit every section, including empty or collapsed sections, to be a destination; pointer hover over a collapsed destination expands it after a short delay.
- Define one keyboard boundary step explicitly: `Alt+Down` on a section's last item transfers it to the beginning of the next section, while `Alt+Up` on a section's first item transfers it to the end of the previous section. Later presses continue within the destination section.
- Make the Mass-item row's **Move up** and **Move down** actions follow the same boundary semantics rather than introducing a separate **Move to section** command in the MVP.
- After any section or item move, keep the moved entity visible and focused, autosave the operation, and make it available to the owning plan or template's Undo history.
