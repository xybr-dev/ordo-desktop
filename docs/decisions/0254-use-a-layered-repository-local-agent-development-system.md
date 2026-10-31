---
id: 254
title: "Use a layered, repository-local agent-development system"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [repo]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Use a layered, repository-local agent-development system

- Keep authoritative implementation documentation, agent guidance, approved
  visual references, and milestone state inside the production repository so
  they version with the source they govern. Source material may remain outside
  the repository, but duplicate planning documentation does not.
- Use a small external skill baseline rather than recreating general Rust and
  GPUI knowledge: Apollo's Rust best-practices skill, focused Longbridge GPUI
  concept/test skills, and the existing TDD, diagnosis, deep-Module design, and
  domain-modeling workflows.
- Do not accumulate generic Rust mega-packs or apply Tokio-specific guidance
  before the codebase adopts Tokio at a justified seam. Do not let the styled
  `gpui-component` skill bypass the adopted Ordo-owned `ui` Module and
  selectively qualified `gpui-base` behavior.
- Keep short repository-wide guidance in `AGENTS.md`. Create focused
  `ordo-development`, `ordo-gpui`, and `ordo-verification` skills lazily only
  after repeated work proves what each on-demand skill must know. Skills must
  reference authoritative documents rather than duplicating them into one
  permanent mega-prompt.
- Make `cargo xtask` the first Ordo-aware inspection and verification surface,
  beginning with deterministic `context`, `architecture`, token, offline/pin,
  and `verify` commands. Consider a local MCP server only when structured
  queries materially exceed what this portable offline CLI can provide.
