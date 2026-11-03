# Cross-crate and native test locality

Pure Module tests live beside their owning Interface. This directory is
reserved for fixtures and cross-crate/native harnesses that cannot be owned by
one crate. Every filesystem or database test must allocate a temporary
location; no test may discover or mutate a developer's Parish data.

- `native-runtime/` (`native-runtime-tests`): runtime lifecycle and packaged
  canonical invariance.
- `display/` (`display-tests`): T0.5 Display Control acceptance. The
  in-memory display Adapter drives the congregation-output E2E and the
  topology fixtures in `fixtures/display-topologies/` (documented line format
  in each file); `native_display.rs` enumerates the real Windows displays and
  runs in CI as `windows-display-control`.
- `artifact-container/` (`artifact-container-tests`): T0.8 `.ordo` container acceptance. Every kind round-trips through `create` and `inspect`, and a malformed corpus is refused with no file escaping its staging directory.
