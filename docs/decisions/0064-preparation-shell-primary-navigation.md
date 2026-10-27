---
id: 64
title: Preparation-shell primary navigation
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [ui, platform]
supersedes: []
superseded_by: null
amends: []
amended_by: [257]
sources: []
---

# Preparation-shell primary navigation

> **Superseded in part by Decision [257](0257-make-library-a-multi-view-destination-with-revision-bound.md).** Rename the global **Songs** destination
> to **Library**, whose local views are Songs, Responses & Prayers, and General
> Presentations. The remaining shell-navigation rules below remain in force.

- Preparation uses a persistent vertical application-navigation rail rather than opening on a generic dashboard of summary cards.
- **Masses** is the default workspace. **Upcoming**, **History**, and **Archive** are views within Masses rather than separate global destinations.
- **Songs** is the entry point for Song works, Arrangements, Choir repertoire, and Song import. These related library workflows remain locally navigable within the workspace.
- **Themes**, **Transfer & Backup**, and **Settings** are the other top-level preparation destinations. Transfer & Backup owns Celebration Package and Parish Backup operations; Settings owns parish, application, and display configuration.
- Presenter Mode is launched in the context of a selected Mass plan. It is not a disconnected top-level preparation destination with an ambiguous active Mass.
- At comfortable widths the rail shows icons and persistent text labels. At constrained preparation widths it may collapse to icons with accessible names and tooltips while keeping the active workspace unambiguous through structure and shape as well as color.
- The warm paper-neutral shell uses stable split-pane workspaces rather than nested generic cards. Exact pane geometry remains responsive and workspace-specific; changing navigation density never changes the selected domain object or discards editor state.
