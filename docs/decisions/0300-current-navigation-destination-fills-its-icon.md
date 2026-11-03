---
id: 300
title: "The current navigation destination fills its icon and marks it with a short pill"
status: accepted
date: 2026-11-03
recorded: 2026-11-03
tags: [design, navigation, icons]
supersedes: []
superseded_by: null
amends: [215]
amended_by: []
sources: [DESIGN.md, docs/mockups/ordo.css, docs/mockups/ordo-kit.js]
---

# The current navigation destination fills its icon and marks it with a short pill

## Context

The global navigation rail marked the current destination with a tint, a bold label and a
full-height 3px bar on the entry's left edge. The entry is rounded, so the bar ran into the top-left
and bottom-left corners. Windows NavigationView marks the current destination with a short, rounded
pill and a filled icon instead. Decision [215](0215-lucide-as-the-single-bundled-operator-icon-family.md)
kept every ordinary icon outlined with a consistent 2px stroke, and Lucide has no filled set.

## Decision

- The current destination in the global navigation rail shows a filled form of its Lucide icon.
  Other destinations stay outlined.
- The filled form comes from the same Lucide icon: its outer shape is filled and its inner details are
  cut out in the selection color. An icon made only of lines (Library, Activity) has no shape to
  fill, so its filled form is the same lines at a 2.75px stroke. These filled forms are Ordo SVGs on
  Lucide's grid. They are the only filled icons outside status markers.
- The current rail entry shows its icon alone, centred, as the Microsoft Store rail does. Its
  label stays as the accessible name and as a pointer tooltip. Every rail entry is at least one
  label tall, so the current one keeps that height and its icon sits in the middle. This is the one
  navigation entry shown icon-only, which Decision 215 otherwise keeps for compact familiar actions.
- Rounded navigation entries mark the current one with a 3×16px pill with rounded ends, instead of a
  full-height bar. In the rail the pill centres on the entry, so it lines up with the icon. In the
  Settings and Help navigation lists it centres on the row. Square-cornered list rows keep the
  full-height bar.
- A two-line rail label ("Transfer & Backup") is centred under its icon.

## Alternatives considered

- **Keep the outlined icon.** The tint, pill and bold label already mark the destination. The filled
  icon is a quicker cue at a glance, and that cue is familiar from Windows.
- **A filled icon from another family.** Decision 215 forbids mixing icon families.
- **Keep the label and set the pill beside the icon.** The pill cleared long labels such as
  "Diagnostics", but it sat high on the entry and looked off-centre.
- **Keep the label and centre the pill on the entry.** Long bold labels reach the edge of the entry
  and ran into the pill.

## Consequences

- The GPUI rail needs one filled SVG for each of the seven destinations, alongside the outlined set.
  Both forms are listed in the runtime icon manifest.
- When "Transfer & Backup" is current it loses its second label line and gets shorter. It is the
  last of the primary destinations and the anchored group sits at the bottom, so no other entry
  moves.
- Status markers are no longer the only filled shapes. The filled rail icon appears only on the
  current destination, where a tint and a pill always go with it, so it is never mistaken for a
  status marker.
