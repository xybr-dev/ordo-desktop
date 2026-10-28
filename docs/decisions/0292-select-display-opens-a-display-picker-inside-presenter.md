---
id: 292
title: Select display opens a display picker inside Presenter
status: proposed
date: 2026-10-28
recorded: 2026-10-28
tags: [presenter, displays]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/presenter-faults.html, docs/mockups/settings.html]
---

# Select display opens a display picker inside Presenter

## Context

When the congregation display disconnects, Decisions [49](0049-congregation-display-disconnection-and-recovery.md)
and [272](0272-space-resumes-output-after-a-disconnect.md) keep the Presentation running. Output
never moves to another display by itself, and recovery onto a display other than the recognized
one goes through **Select display** / **Enable Output**. Decision
[202](0202-installation-specific-displays-settings-page.md) defines the Settings → Displays page
and says its display picker is reused in Presenter preflight. Nothing says where **Select display…**
leads during Mass. Sending the operator to Settings would mean leaving Presenter and losing sight of
Program and Preview.

## Decision

- **Where.** **Select display…** in the Output Dock opens a slide-over in the Insert panel's place
  (Decisions [290](0290-emergency-insert-chooser-and-quick-insert-dialog.md),
  [291](0291-emergency-edit-opens-the-selected-slide-in-the-insert-panel.md)). Presenter stays
  open; Program, Preview and the Output Dock stay in view; the Presentation keeps running.
- **When.** The control appears while output is Disconnected and the recognized display isn't back.
  It becomes the primary control when another display is connected.
- **What it borrows from Settings → Displays:**
  - The layout tiles: the operator display (not selectable), the assigned display marked
    **Unavailable** while missing, and any other connected display, marked **Recognised** or **New
    to Ordo**.
  - **Identify displays**, which shows numbers only and never Mass content.
  - **Output Fit** (0–10%), for a display new to Ordo.
  The calibration pattern stays in Settings, outside Mass.
- **Choosing a display.** **Use Display N** assigns it as the congregation display and saves the
  choice for this computer, as Settings does. Its window opens black, and a strip says so. Only
  Space or **Take Preview Live** then puts Preview on it.
- **No display to choose.** If only the operator display is present, **Use display** is disabled
  and the panel explains why. **Turn congregation output off** ends the Disconnected state on purpose;
  **Enable Output** brings output back later, black first.
- **Keys.** While the panel is open, single-key Presenter commands are suspended, and `Esc` closes
  it without changing anything.

## Alternatives considered

- **Opening Settings → Displays.** It leaves Presenter in the middle of Mass, and the operator
  loses sight of Program and Preview.
- **A centred dialog.** It hides the Output Dock, as in Decision 290, and has too little room for the
  display layout.
- **Switching to the new display automatically when one appears.** Ruled out by Decision 49: output
  never moves without the operator's choice.

## Consequences

The Settings → Displays tiles and Identify become one shared picker, used in Settings, in Presenter
preflight, and here. Choosing a display during Mass changes the installation's saved congregation
display, exactly as Settings would.
