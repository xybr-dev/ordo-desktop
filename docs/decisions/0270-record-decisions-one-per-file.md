---
id: 270
title: Record decisions one per file
status: accepted
date: 2026-10-27
recorded: 2026-10-27
tags: [repo, decisions]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [clockout:docs/decisions/README.md, clockout:docs/decisions/adr.py, clockout:docs/decisions/0001-record-architecture-decisions.md]
---

# Record decisions one per file

## Context

Ordo's decisions lived in `decision.md`, one 386 KB file with 269 numbered sections. Supersession
was prose: a forward notice at the top of the affected section ("Superseded in part by Decision 268"),
so finding what is still in force meant reading notices across the whole file, and cites by line
number (`decision.md:2697-2700`) broke on every edit. clockout keeps one record per file with status
in frontmatter, checked by `adr.py`, and the owner wants one format across repos. Ordo is early in
development, so the move is cheap now.

## Decision

- Record decisions in `docs/decisions/`, one per file, named `NNNN-kebab-title.md`, with the
  frontmatter in `TEMPLATE.md`. `docs/decisions/adr.py` is a copy of clockout's, extended with
  `amends` / `amended_by` for partial supersession, which the ledger uses far more than full
  supersession (24 amended records against 5 superseded).
- Split the ledger mechanically and keep its numbers: Decision N becomes record N, so existing cites
  stay valid. Decision 12a stays inside record 12. Bodies are moved verbatim apart from links added to
  "Decision N" cites; status and both directions of every link come from the forward notices.
- The ledger's non-decision sections (preamble, design-status summary, interview record, working
  glossary, deferred list, grilling outcome) move unchanged to `docs/decision-ledger-archive.md`, which
  is not normative. `CONTEXT.md` owns domain terms and `MVP-PLAN.md` owns scope.
- `decision.md` is removed. References to it point at `docs/decisions/` or at a record.
- `adr.py check` runs in `cargo xtask verify` and in its own CI workflow, because the main verify
  workflow skips Markdown-only changes.
- New records use the Context / Decision / Alternatives considered / Consequences sections.

## Alternatives considered

- **Freeze `decision.md` and start new records at 0270.** Cheaper, but two systems to search, and
  status for 0001–0269 would stay buried in prose.
- **Split and renumber or consolidate.** Breaks every "Decision N" cite in `MILESTONES.md`,
  `MVP-PLAN.md`, `DESIGN.md` and the code for no gain.
- **Port the checker to Rust inside xtask.** Keeps `cargo xtask verify` Python-free, but forks the
  shared tool. Python is already on the development machine and on GitHub's Windows runners.

## Consequences

- The index in `docs/decisions/README.md` shows each record's status, including what amends it.
- `cargo xtask verify` now needs Python 3 on the path (`python`, `python3` or `py`).
- Imported records keep the ledger's bullet form and import-day dates; their tags are a first pass
  from the titles, and 36 have none yet.
- The split found four partial supersessions stated only in the later decision's text: 262, 263,
  264 and 265 narrowing 255, and 265 narrowing 264. Records 255 and 264 gained the missing forward
  notices, marked as added at the split, and the amend links now point both ways.
