# Presentation snapshot fixtures

Two sanitized snapshots read by `crates/presentation/src/snapshot_reader.rs`
and asserted byte-for-byte by `tests/presentation/tests/snapshot.rs`. They
contain no parish, person, or place names and no copyrighted text; the verse
and prose are original placeholders (English with Cebuano-looking words).

Each directory is one snapshot: `snapshot.txt` (the manifest; its grammar is
the module doc of the reader), `frames/<name>.txt` (`ordo-fixture/1` frames),
and `assets/<name>` (raw bytes). Bytes are SHA-256 hashed exactly as stored, so
`.gitattributes` marks this tree `-text`: never let an editor or checkout
convert line endings.

## normal-mass

A realistic Sunday structure: six sections (Before Mass, Introductory Rites,
Liturgy of the Word, Liturgy of the Eucharist, Concluding Rites, After Mass),
15 items, 33 Cues. Covers a general presentation with auto-play (8 s) and a
per-Cue duration override, a GeneralImage frame with a declared asset, a song
whose chorus frame is reused by three Cues, response items with operator-only
trigger Cues (private phrase), Hold and Wait Cues, a psalm refrain reused, a
long wrapping reading passage and communion lyric, both Themes (Custom Mass
Theme; the dismissal item overrides to the fallback). No unavailable Cue.

## recovery-drill

Adversarial controls for the Presenter recovery tests (T1.2): six sections,
7 items, 18 Cues. Contains exactly one unavailable Cue (flat 2) between two
projectable Cues, a trigger, a Hold, a Wait, an item whose only Cues are
operator-only (trigger plus Hold), an auto-play item with several Cues, frame
reuse, a long wrapping lyric, a Black frame, an image frame with its asset,
and both Themes with an override.

`commands.txt` is the canonical command script T1.2 replays: one command per
line from `next previous next-item previous-item first last take freeze
blackout enable-output disable-output disconnect autoplay`. In T1.1 it is only
data; the E2E test checks every line is a known command.

## Changing a fixture and re-hashing

1. Edit the frame, asset, or manifest. Keep LF endings and no trailing blank
   line (the frame and manifest parsers are strict).
2. For every changed file, set its digest: `sha256sum <file>` into the
   manifest's `frame-sha256` or `asset-sha256` (and `asset-size` for an asset).
3. Recompute `fingerprint`: the SHA-256 of every frame and asset digest (hex),
   sorted ascending, each followed by LF:

       grep -h -e '^frame-sha256 ' -e '^asset-sha256 ' snapshot.txt \
         | cut -d' ' -f2 | sort | sha256sum

   (or run the example below and copy the actual fingerprint from the error).
4. Check it loads and read the flattened order:
   `cargo run -p presentation --example snapshot-inspect -- tests/fixtures/presentations/<name>`
5. Update the hard-coded fingerprint, frame hashes, and order table in
   `tests/presentation/tests/snapshot.rs`. These constants are intentionally
   duplicated so an accidental fixture edit cannot pass by agreement.

Identities (UUIDs) are stable forever: never reuse one for different content.
