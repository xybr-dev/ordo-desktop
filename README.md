# Ordo Desktop

Ordo is an offline-first Catholic liturgical presenter for parish audiovisual
operators. This repository is the clean-slate Rust + GPUI production
implementation.

## Start here

- [`CONTEXT.md`](./CONTEXT.md) — canonical domain language.
- [`DESIGN.md`](./DESIGN.md) — visual and interaction contract.
- [`MVP-PLAN.md`](./MVP-PLAN.md) — strategic product and architecture baseline.
- [`MILESTONES.md`](./MILESTONES.md) — ordered implementation tasks and gates.
- [`LEARNING-ROADMAP.md`](./LEARNING-ROADMAP.md) — Rust/GPUI learning runway.
- [`docs/agent-development.md`](./docs/agent-development.md) — agent-assisted
  development strategy.

## Run the app

From the repository root:

```sh
cargo run --locked
```

This opens the current operator window. Close it to stop the app; the
congregation window is not implemented yet.

## Verify

Install `cargo-deny`, then run:

```sh
cargo xtask verify
```

Ordo must build and operate without runtime internet access.
