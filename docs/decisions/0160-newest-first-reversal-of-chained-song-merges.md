---
id: 160
title: Newest-first reversal of chained Song merges
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Newest-first reversal of chained Song merges

- Allow merge redirects to chain historically while flattening their runtime resolution to the newest surviving Song.
- If merge `A → B` is followed by `B → C`, block reversal of `A → B` while `B → C` remains applied. Explain **Reverse the newer merge first** and link to that newer merge-history event.
- Reversing `B → C` atomically restores Song B and reconstructs the prior `A → B` redirect relationship from stored merge provenance. The earlier merge may then be reversed independently.
- Apply the same newest-first rule to longer chains. Never split, fork, or partially redirect an applied identity chain merely to reverse an older event out of order.
