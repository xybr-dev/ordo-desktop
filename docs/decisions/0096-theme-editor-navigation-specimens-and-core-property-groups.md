---
id: 96
title: "Theme Editor navigation, specimens, and core property groups"
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

# Theme Editor navigation, specimens, and core property groups

- The full Theme Editor is a dedicated preparation workspace rather than another nested slide-over. One dominant canonical `16:9` frame remains authoritative beside the controls.
- From a Mass plan, **Edit selected theme** opens the highlighted reusable Theme without applying it. **Back to Mass Theme** returns to the same plan and restores the chooser with its staged Theme and Theme-override decisions intact.
- Opening the same editor from top-level **Themes** has no Mass-plan return context and no staged Mass Theme selection.
- A specimen bar changes only the content used to test the Theme; it does not change the Theme's formatting scope, edit Mass content, or create per-slide styling. Stable specimens cover **Lyrics**, **Response**, **Identification**, and **Reading**. **Current Mass slide** is additionally available only when the editor was opened from a Mass plan; the library entry begins with a stable specimen instead.
- Every specimen is rendered through the same canonical `1920x1080` pipeline used by the Mass planner and Presenter surfaces, allowing the operator to expose wrapping, contrast, density, and placement consequences before using the Theme.
- Theme properties are separated into **Typography**, **Colors**, and **Layout** rather than mixing unrelated controls under Type. **Typography** owns text-role selection, bundled typeface, preferred and minimum size, weight, and line spacing. **Colors** owns the text/background pair, picker and editable hexadecimal input, reusable parish colors, immediate canonical-preview feedback, and computed contrast feedback. **Layout** owns horizontal alignment, vertical placement, safe area, and maximum text width.
- Identification titles, credits, and labels may use their established secondary text role rather than inheriting lyric-body weight blindly. Specimen selection and text-role selection remain distinct: the former chooses test content, while the latter chooses which reusable Theme rule is edited.
- Background-image and other decorative-layer controls must remain within the Theme's visual-appearance responsibility rather than Typography; their exact MVP controls and whether **Colors** is later relabeled **Appearance** remain to be finalized without changing the three-way separation.
- Shared-Theme change semantics are extended by Decision [97](0097-staged-shared-theme-editing-and-safe-duplication.md); Theme revision/Undo and affected-plan rerender review remain unresolved.
