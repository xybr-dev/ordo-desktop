# Agent-assisted Rust and GPUI development

Ordo uses a layered agent strategy inspired by Laravel Boost: small always-on
guidelines, focused on-demand skills, exact framework context, and executable
repository inspection. Skills supplement deterministic tooling; they never
replace compiler, lint, test, policy, or qualification evidence.

## Adopted external skills

Start with a deliberately small set:

- `apollographql/skills@rust-best-practices` for idiomatic ownership, error
  handling, Clippy, testing, traits, pointer safety, and performance habits.
- Focused Longbridge GPUI skills for `gpui-context`, `gpui-entity`,
  `gpui-action`, `gpui-focus-handle`, `gpui-event`,
  `gpui-layout-and-style`, `gpui-async`, and `gpui-test`.
- The existing workflow skills `tdd`, `diagnosing-bugs`, `codebase-design`, and
  `domain-modeling` when their trigger applies.

Do not install generic Rust mega-packs merely for volume. Defer Tokio-specific
async guidance until Ordo actually adopts Tokio at a justified seam. Do not use
the styled `gpui-component` skill to bypass Ordo's internal `ui` Module:
Longbridge's unstyled `gpui-base` is qualified selectively, while Ordo owns its
visual language and workflow Interfaces.

## Repository-owned Ordo skills

Create these lazily after repeated implementation work proves their content:

- `ordo-development` routes source work through the active milestone,
  dependency direction, domain vocabulary, and clean-slate scope rules.
- `ordo-gpui` captures exact patterns from the pinned GPUI revision for
  entities, contexts, actions, focus, windows, tasks, and tests.
- `ordo-verification` maps acceptance criteria to automatic, native, manual,
  and physical evidence without overstating automation.

Initially, `AGENTS.md` plus this document are sufficient. A skill must remain
focused and on-demand; do not build one giant prompt that duplicates
`CONTEXT.md`, `DESIGN.md`, `MVP-PLAN.md`, or `MILESTONES.md`.

## Ordo Boost path

`cargo xtask` is the first project-aware tool surface. Its commands should
eventually expose exact, offline repository facts such as:

```text
cargo xtask context       workspace graph, revisions, active milestone
cargo xtask verify        ordinary deterministic verification gates
cargo xtask architecture  crate-dependency contract
```

Prefer this portable CLI over an MCP server while these few deterministic
commands are enough. Consider a local MCP server only when several agents need
structured queries that cannot be served cleanly by `xtask` output.

## Learning discipline

For each generated change, the owner should be able to explain:

1. Which Module owns the state.
2. Which callers cross its Interface.
3. Which thread or GPUI context executes it.
4. How failures travel to the caller.
5. Which test proves the behavior.

Unknown clones, trait bounds, lifetimes, spawned tasks, globals, or `unsafe`
blocks stop review until explained. Work one milestone acceptance criterion at
a time and deliberately observe the relevant test fail before trusting its
green result.

