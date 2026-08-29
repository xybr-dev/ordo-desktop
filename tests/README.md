# Cross-crate and native test locality

Pure Module tests live beside their owning Interface. This directory is
reserved for fixtures and cross-crate/native harnesses that cannot be owned by
one crate. Every filesystem or database test must allocate a temporary
location; no test may discover or mutate a developer's Parish data.
