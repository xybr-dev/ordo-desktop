# Ordo coding-agent guide

Before changing source, read only the authoritative material relevant to the
active task:

Read documentation selectively: use headings and search to locate relevant
sections. Do not read an entire file unless explicitly required, the relevant
section depends on broader context, or targeted reading leaves behavior
ambiguous. Always read the complete selected milestone task and its acceptance
criterion.

1. Use `MILESTONES.md` to select exactly one active task and its acceptance
   criterion.
2. Use `CONTEXT.md` for domain terms and invariants.
3. Use `DESIGN.md` and its approved reference for visible behavior.
4. Use `MVP-PLAN.md` for strategic architecture and scope.
5. Use `LEARNING-ROADMAP.md` when explaining unfamiliar Rust or GPUI code.

This list is a reading route, not a precedence stack. Authority is by subject:
`MILESTONES.md` owns task order, status, and acceptance gates; `CONTEXT.md` owns
domain terms and invariants; `DESIGN.md` owns visible behavior and visual
tokens; and `MVP-PLAN.md` owns product scope and strategic architecture.
`decision.md` is the detailed decision ledger. Consult it when the applicable
shorter document is silent; if normative documents conflict, reconcile them
before implementation rather than choosing one silently.

## Source authority

This repository is the clean-slate production implementation. Only its
committed source, documentation, migrations, and instructions are
authoritative.

## Working rules

- Keep changes within the active milestone task; do not import deferred scope.
- Treat crates as dependency-enforcement seams, not folders for every feature.
- Keep `domain` free of GPUI, SQLite, filesystem, network, and OS-window types.
- Keep domain rules out of GPUI views and storage adapters.
- Use the pinned GPUI revision's source/examples; do not guess pre-1.0 APIs.
- Do not block the GPUI thread with storage, filesystem, rendering, hashing, or
  export work.
- Prefer borrowing over unexplained cloning. Every long-lived task must have a
  named owner and shutdown behavior.
- Test behavior through the owning Module Interface. Use temporary locations;
  tests must never read or mutate real Parish data.
- Do not add a runtime internet dependency, CDN, telemetry, or login.
- Run `cargo xtask verify` before declaring a task complete.

Detailed agent-skill policy lives in `docs/agent-development.md`.
