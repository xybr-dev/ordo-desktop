---
id: 278
title: Manage Choirs as a Library view
status: accepted
date: 2026-10-28
recorded: 2026-10-28
tags: [songs]
supersedes: []
superseded_by: null
amends: [257]
amended_by: []
sources: [docs/mockups/arrangement-details-v2.html, docs/mockups/library-choirs.html]
---

# Manage Choirs as a Library view

## Context

Decisions [87](0087-simplified-celebration-metadata-and-choir-management.md) and
[221](0221-optional-choir-setup-with-inline-lightweight-creation.md) put the Choir management
workspace under **Songs → Choirs**. Decision [257](0257-make-library-a-multi-view-destination-with-revision-bound.md)
renamed the Songs destination to **Library** with three views (Songs, Responses & Prayers, General
Presentations) and named no place for Choirs. A choir created in Arrangement details is told to get
its label and preferred languages "later in Choirs", which pointed nowhere.

## Decision

- The Library has a fourth view, **Choirs**, after General Presentations. It is the Choir management
  workspace Decisions 87 and 221 describe, including its **No choirs yet** state and **Add choir**.
- Quick creation from a Choir selector (Decision 221) points there for a choir's remaining details.

## Alternatives considered

- **Settings → Choirs.** Treats choirs as parish setup, but separates them from the repertoire Decision
  87 has each Choir show.
- **A Choirs destination in the left rail.** Most visible, but adds a rail entry for something edited
  rarely.

## Consequences

This amends Decision 257's list of Library views; Decision 221's **Songs → Choirs**
reads as **Library → Choirs**. The view's layout and behaviour are in Decision [279](0279-library-views-beside-songs-share-its-master-detail.md) and `library-choirs`.
