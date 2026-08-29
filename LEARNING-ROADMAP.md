# Ordo — Learning-first development roadmap

This guide is the runway into Ordo for a developer who is still learning Rust
and GPUI. It does not replace [`MILESTONES.md`](./MILESTONES.md), add MVP scope,
or track implementation status. The milestones remain authoritative; this guide
explains the knowledge needed to execute them without treating generated code as
a black box.

## The outcome

Do not wait until you have “mastered Rust” before starting Ordo. Become
comfortable with the next concept, prove it in a tiny exercise, and then apply
it to one narrow Ordo behavior.

Before accepting a change, you should be able to answer:

1. Which Module owns the state?
2. Which callers cross its Interface?
3. Which thread or GPUI context executes it?
4. How can it fail, and where does the failure go?
5. Which test proves the observable behavior?

If any answer is unclear, the change is too large or its seam is misplaced.

## What you need—and what you do not

You need working knowledge of:

- Cargo packages, crates, Modules, workspaces, dependencies, and features.
- Structs, enums, pattern matching, collections, `Option`, and `Result`.
- Ownership, borrowing, references, `String` versus `&str`, and cloning by
  deliberate choice.
- Traits and generics at the level needed to read an Interface.
- Tests, structured errors, threads, channels, and `Send`/`Sync` basics.
- GPUI application startup, windows, entities, contexts, rendering, actions,
  focus, events, subscriptions, and background tasks.
- Reading framework examples and source when pre-1.0 documentation is incomplete.

You do **not** need to master these before beginning:

- `unsafe` Rust, procedural macros, custom allocators, or compiler internals.
- Advanced lifetime tricks or hand-written futures.
- GPU shaders or low-level graphics APIs.
- Every GPUI feature or every `gpui-base` primitive.
- General-purpose framework or component-library design.

## Track A — Rust foundations

Use [The Rust Programming Language](https://doc.rust-lang.org/book/) and
[Rustlings](https://github.com/rust-lang/rustlings) together. Read only the
chapters relevant to the current exercise; write code immediately afterward.

### A1. Types that express the domain

Learn variables, functions, structs, enums, `Vec`, `String`, modules, and
pattern matching.

Build a disposable, non-GPUI exercise containing:

```rust
enum CueBehavior {
    Project,
    Wait,
    Hold,
    Blackout,
}

struct Cue {
    title: String,
    behavior: CueBehavior,
}
```

Add functions that count projectable Cues and reject an empty Mass section.
The point is not the eventual Ordo model; it is learning to make invalid cases
visible in types.

Exit check: explain why an enum is safer here than several booleans and write
tests for every variant without assistance.

### A2. Ownership and borrowing

Study ownership, moves, references, mutable borrowing, slices, `String`, and
`&str`. Repeat the A1 exercise while removing unnecessary clones.

Exit check: for every function parameter, explain why it takes `T`, `&T`, or
`&mut T`. Never add `.clone()` merely to silence the compiler without knowing
the lifetime and ownership consequence.

### A3. Recoverable failures

Study `Option`, `Result`, the `?` operator, error enums, and the distinction
between a recoverable failure and a programming invariant violation.

Build a parser for a tiny Cue text format. Return a typed error for an unknown
behavior or blank title. Do not use `unwrap()` in the library path.

Exit check: trace one successful value and one failure from caller to result,
and test both through the parser's Interface.

### A4. Traits, generics, and lifetimes

Learn enough traits and generics to understand an Interface and its Adapter.
Learn what lifetime annotations describe, without trying to become a lifetime
specialist.

Build an in-memory Cue repository and a file-backed toy Adapter behind one
small trait. This exercise demonstrates why two Adapters make a real seam.

Exit check: replace one Adapter in a test without changing the calling logic.

### A5. Tests and Cargo workspaces

Study unit tests, integration tests, visibility, packages, crates, workspace
members, shared dependencies, and `Cargo.lock`.

Build a disposable two-crate workspace where one pure crate is consumed by one
binary. Run:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Exit check: explain the difference among a package, crate, Module, workspace,
and binary target. This prepares you to understand Ordo's seven initial crates
rather than merely accepting the scaffold.

### A6. Threads and channels

Study scoped ownership across threads, message passing, `Send`, `Sync`, and
orderly shutdown. Shared mutable state behind `Arc<Mutex<_>>` is not the default
answer for Ordo.

Build one worker that owns a collection, accepts typed commands through a
channel, returns typed results, and exits when its owner shuts down.

Exit check: identify the sole state owner, prove that callers cannot mutate it
directly, and test shutdown without sleeping or hanging. This is the mental
model for Ordo's SQLite owner worker.

## Track B — GPUI foundations

GPUI is pre-1.0. Learn from the exact pinned revision used by Ordo, not snippets
copied from an unrelated commit. Upstream's
[GPUI example index](https://github.com/zed-industries/zed/blob/main/crates/gpui/examples/README.md)
currently identifies `hello_world`, `input`, `uniform_list`, and `testing` as
the starting examples.

### B1. Application, window, view, and element

Run and then re-create the `hello_world` example. Trace:

```text
Application
  -> open window
    -> create root Entity<ViewState>
      -> Render
        -> element tree
```

Change text, layout, color, spacing, and window size yourself.

Exit check: explain the difference between retained state in an `Entity<T>` and
the element tree returned for one render.

### B2. Entity and context ownership

Build a counter with state stored in an Entity. Update it through the proper
context and cause a rerender. Then add a child entity observed by a parent.

Learn `Entity<T>`, `Context<T>`, `App`, `Window`, `cx.new`, `read`, `update`,
`notify`, observation, subscription lifetimes, and weak handles.

Exit check: draw the ownership graph and explain why a callback can or cannot
retain each handle it captures.

### B3. Actions, key bindings, and focus

Re-create a small Cue list navigated by actions rather than raw key checks. Add
stable element IDs, a `FocusHandle`, Tab traversal, and visible keyboard focus.

Exit check: trigger the same action from keyboard and pointer without
duplicating the state transition. Move focus without changing selection.

### B4. Text input and events

Study the GPUI input example, then the unstyled
[`gpui-base` input](https://github.com/longbridge/gpui-component/blob/main/website/base/primitives/input.md).
Build one labeled field in `ui-lab`, with validation, change events, Enter,
focus, and blur.

Exit check: explain which entity owns the editable value, when the parent is
notified, and how IME/text behavior stays inside the base primitive.

### B5. Overlays, scrolling, and virtualization

Build a menu, a viewport-bounded dialog, one nested slideover, and a long
virtualized list. Test dismissal, focus trapping, focus return, scrolling, and
resizing.

Exit check: keep the overlay header and actions reachable at 1366×768 and 200%
Interface scale, with only the body scrolling.

### B6. GPUI tests

Use `#[gpui::test]`, `TestAppContext`, actions, simulated input, focus, and
window-based assertions. Upstream maintains a concrete
[`TestAppContext` input example](https://github.com/zed-industries/zed/blob/main/crates/gpui/examples/view_example/example_tests.rs).

Make every B3–B5 scenario runnable both interactively in `ui-lab` and through
the UI harness.

Exit check: reproduce a focus or state bug with a failing test before fixing
it.

## Track C — Turn exercises into Ordo

Tracks A and B do not need to be entirely complete before Track C starts. Begin
real project work when A1–A5 and B1–B3 are comfortable; learn the remaining
topics just before their milestone needs them.

### C1. Execute T0.1 by hand

Create the virtual Cargo workspace and each initial crate deliberately:

```text
domain -> no GPUI, SQLite, filesystem, or OS window types
storage -> depends on domain
rendering -> depends on domain
presentation -> depends on domain and completed rendering values
ui -> depends on GPUI and qualified gpui-base
platform -> owns native window/display behavior
ordo -> composition root depending on the required crates
ui-lab -> development binary depending on ui and fixtures
```

Do not generate all manifests and move on. Inspect each dependency edge and
explain why the reverse dependency is forbidden.

### C2. Grow `ui-lab` before feature screens

Implement tokens, text, Button, Field, focus treatment, and primitive states in
that order. Each addition gets:

- A small Ordo-owned Interface.
- Curated interactive scenarios.
- Keyboard and focus behavior.
- At least one automated state or interaction test.
- Manual inspection at both target viewport presets.

Only add a primitive when the next accepted Ordo screen needs it. Do not build
an abstract general-purpose library in anticipation of unknown applications.

### C3. Complete the rest of Phase 0 in risk order

Continue with the permanent runtime ownership, canonical renderer, passive
congregation window, Presenter performance harness, SQLite worker, `.ordo`
container, and PowerPoint spike exactly as ordered in `MILESTONES.md`.

Phase 0 is not throwaway prototyping. Its qualified code becomes the foundation
for feature work, but every spike must still be small enough for you to explain
from its Interface to its platform interaction.

## Working with an AI agent without losing the codebase

Use the agent as a tutor, pair programmer, and reviewer—not as an opaque source
generator.

Before asking for implementation:

- Name one milestone task and preferably one acceptance criterion.
- Ask for the intended ownership, dependency direction, failure model, and test
  before accepting a broad patch.
- Ask the agent to locate the current upstream example matching the pinned GPUI
  revision rather than guessing an interface.

While reviewing a patch:

- Read the Interface first, then its tests, then the Implementation.
- Stop at every unfamiliar ownership conversion, clone, trait bound, lifetime,
  spawned task, global, or unsafe block.
- Ask what invariant the type system is enforcing and what simpler design was
  considered.
- Reject speculative abstractions and dependencies not required by the active
  acceptance criterion.

Before merging:

- Explain the state owner and thread owner in your own words.
- Deliberately break the behavior and watch the relevant test fail.
- Run formatting, Clippy, tests, and the applicable native/manual scenario.
- Keep the change narrow enough that you can later diagnose it without relying
  on the original agent conversation.

Useful prompts include:

```text
Explain this diff from the owning state outward. Do not change it yet.

Show me the ownership graph and tell me why every clone exists.

Which behavior is enforced by Rust's types, and which still requires a test?

Give me the smallest failing test for this bug before proposing a fix.

What current pinned GPUI example demonstrates this interface?
```

## Readiness to begin feature work

You are ready to leave the foundation runway when you can:

- Read ordinary borrow-checker errors without reflexively cloning everything.
- Model state with structs and enums and return typed failures with `Result`.
- Write and run pure Rust and GPUI interaction tests.
- Trace `action -> domain transition -> state -> render` in Ordo.
- Identify every long-lived task and the owner responsible for cancelling it.
- Explain why the UI thread never performs blocking storage or rendering work.
- Add or modify a `ui` primitive through `ui-lab` without opening `ordo`.
- Explain every crate dependency in the initial workspace graph.

This is sufficient competence to build Ordo responsibly. Rust mastery will
continue during the project.
