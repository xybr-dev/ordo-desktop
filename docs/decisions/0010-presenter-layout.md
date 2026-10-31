---
id: 10
title: Presenter layout
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: [203]
sources: []
---

# Presenter layout

> **Superseded in part by Decision [203](0203-same-window-native-full-screen-presenter-mode.md).** Presenter now transforms the existing
> operator window into native full-screen Live Control rather than merely
> maximizing it. The three-column hierarchy and remaining layout rules below
> stay adopted.

Every projectable slide uses a fixed `1920x1080` logical canvas:

- Slide content is laid out and text is wrapped once in canonical canvas coordinates. Changing the physical output resolution must never reflow text, alter line breaks, move elements, or repaginate the content.
- The completed 16:9 composition is uniformly scaled to the physical output. `1366x768`, `1920x1080`, `2560x1440`, `3840x2160`, and other 16:9 outputs therefore preserve identical geometry.
- A display within the adopted `0.1%` tolerance of 16:9, including `1366x768`, fills completely through final-frame scaling; materially different aspect ratios fit and center the complete frame against black rather than cropping or reflowing it.
- Higher-resolution outputs rerender the same canonical geometry at the target pixel density so text and vector elements remain sharp; they do not create a different responsive slide layout.
- The operator application shell remains responsive. Its window size may alter the number and size of thumbnails, but the slide canvas shown inside those thumbnails is fixed.
- Typography and element dimensions are stored in canonical canvas units. Physical monitor DPI, operating-system scaling, and device pixel ratio must not change wrapping or pagination.
- The Celebration Package carries the approved rendering inputs and required fonts when licensing permits so another workstation can reproduce the canonical frames without font substitution.

The operator presenter uses three columns:

1. **Ordo / entire Mass**
   - Shows the full ordered Mass.
   - Shows titles only: songs, responses, readings, rites, and other top-level items.
2. **Cue timeline**
   - Scrollable detailed sequence grouped under the titles from column 1.
   - Projected content appears as actual 16:9 slide thumbnails rather than generic content cards.
   - Every representation of a projectable slide uses one canonical rendered frame. Column-2 thumbnails, column-3 Preview and Program, and the congregation display must be visually identical apart from scaling, as in PowerPoint. Thumbnail rendering must not reflow, abbreviate, restyle, or otherwise approximate the slide.
   - Operator-only priest/minister triggers appear as compact cue rows because they have no congregation slide.
3. **Output monitors and state**
   - Shows Program: the congregation's current output.
   - Shows Preview: the operator's current selection/upcoming output.
   - Shows state indicators such as live/unfrozen, frozen, and blackout.
   - Shows the relevant private operator trigger/context.

Thumbnail density in column 2 is responsive rather than fixed:

- Ordo chooses the largest practical canonical 16:9 thumbnails for the available column width.
- In **Visual Slide Grid** mode, the canonical frame within each thumbnail is never narrower than `300px` (`300x168.75` at 16:9) at supported Presenter sizes. Labels and state treatment sit outside this fixed-ratio frame.
- The grid targets approximately three slides per row at `1366x768` and up to four at `1920x1080`. When available width contracts, it removes a column before it would squeeze a frame below the minimum.
- `1366x768` and `1920x1080` are required operator-screen design and test targets.
- Operator-only cue rows continue to span the available timeline width.

Ordo chooses an automatic initial layout from the available operator-screen space:

- At `1366x768`, the Ordo rail starts collapsed, the visual slide grid targets approximately three columns, and Program and Preview are stacked.
- At `1920x1080`, the Ordo rail starts pinned, the visual slide grid may reach four columns, and the output-monitor column is given more room while preserving usable timeline thumbnails.
- Manual rail pinning/collapse and Visual Slide Grid/Compact Cue List choices persist per workstation.
- Window resizing may reflow controls and change thumbnail density, but it must not change the selected cue, Program, Preview, or presentation state.

The cue timeline provides two operator-selectable modes:

- **Visual Slide Grid** is the default and shows canonical rendered slide frames.
- **Compact Cue List** shows numbered rows with role/section and a lyric or cue excerpt for denser navigation.
- Both modes select the same underlying ordered cues and use column 3 for the canonical Preview.

Column 1 is a collapsible/pinnable Ordo rail:

- When collapsed, a narrow labeled **ORDO** tab remains visible. Hover alone never reveals the rail in Presenter Mode.
- Explicitly activating the tab or its keyboard command opens the unpinned rail as an overlay without resizing the Cue timeline beneath it.
- Clicking its pin control keeps it open until unpinned.
- Clicking outside the unpinned rail or pressing `Esc` closes it and returns focus to the Cue timeline.
- Collapsing it gives the cue timeline more space for the visual slide grid.

Column 1 behaves as a table of contents rather than a filter:

- Selecting a title scrolls column 2 to the corresponding group while preserving the rest of the Mass timeline.
- Groups may be collapsed manually.
- The active cue remains highlighted and is automatically kept in view during keyboard navigation.
- Surrounding groups remain available for context and emergency jumps.

The two application windows have different display behavior:

- The congregation window is borderless full-screen on its assigned output display.
- The operator window is ordinarily windowed and resizable for preparation, rehearsal, troubleshooting, and screen sharing.
- Presenter Mode retains normal operating-system recovery mechanisms so the operator is not trapped if display assignment or projection fails.
