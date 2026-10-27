---
id: 95
title: Mass Theme chooser and nested Theme Overrides review
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, themes, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Mass Theme chooser and nested Theme Overrides review

- Activating **Theme · Parish Dark** in the Mass-plan header opens a floating right **Mass Theme** slide-over that spans the complete application window, including its header. It is an app-level planning task rather than an Inspector-confined panel.
- The chooser previews each parish Theme through the selected canonical `16:9` slide so typography, pagination, background, contrast, and other presentation consequences can be compared on real Mass content.
- Choosing a Theme is staged. The canonical preview updates immediately, while the Mass plan remains unchanged until the sticky header action commits the selection. Closing the panel or pressing `Esc` discards the staged choice.
- The chooser identifies the currently applied Theme and explicitly discloses Mass items that keep item-level Theme overrides. Applying a different Mass Theme never silently replaces those overrides.
- **Review** opens a child **Theme Overrides** slide-over above the Mass Theme chooser. The parent recedes slightly but remains spatially visible; Back returns to it without losing the staged Theme or override decisions.
- Override review follows Mass-outline order and names each owning item. Every row shows the current item override and the Theme it would inherit after the staged change as an explicit `current override → inherited result` comparison, even when both happen to render similarly.
- **Use Mass Theme** stages removal of one item's override; the all-items action stages the same change for every overridden item. These actions remain reversible within the review and do not partially mutate the plan.
- The parent commit action summarizes and applies the staged Mass Theme selection and staged override removals as one coherent change set. Autosave and Undo follow the ordinary Mass-plan editing rules; existing Presentation snapshots remain immutable.
- **Edit selected theme** routes to the full Theme Editor for the highlighted Theme without implicitly applying that Theme to the Mass plan.
