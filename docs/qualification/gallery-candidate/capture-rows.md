# Gallery capture provenance

- Pipeline: the live `LabView` (`open` plus its `spawn_worker` raster thread): `load_fixtures` order → production canonical worker (`CanonicalCommand::Compose`, default Theme, 100% density) → `rendering::rasterize` at 100% (solid-gray stand-in for GeneralImage foregrounds) → `ordo::canonical_views::finish_frame`. Each row's `Rendered.frame` plus `diagnostics` below are those exact live-lab results.
- Visible frame: finished pre-chrome slide pixels, `1920x1080` logical, `1920x1080` raster at 100% density — slide pixels paired with wrapper/diagnostics metadata, not a compositor screenshot (`TestAppContext` supplies no headless renderer on any OS, so none is attempted here).
- Wrapper: `ui::canonical_frame::canonical_frame` through `ordo::canonical_views` (`timeline_thumbnail` / `inspector_frame` / `preview_frame` / `program_frame`, 320 / 480 / 720 / 960 before Interface scale, `4px` operator-only chrome); the congregation surface reserves the same sampling edge-to-edge with square corners (black until its background encode lands).
- Diagnostics: the Warnings column below rides beside each capture; overflow fixtures warn beside complete frames.
- Live review: `cargo run -p ordo --example canonical-slide-lab` (`F2` fixture · `F3` viewport · `F4` scale · `F5` density; diagnostics beside the scaled frame).
- Status: candidate for review only — not in `design-references/`, never an Approved Reference until the live output and its captured reference are approved together in `T0.4-MANUAL-GATE.md`.

| Fixture | Template | Header | Body lines (authored/rendered) | Footer lines (rendered) | Warnings | PNG |
| --- | --- | --- | --- | --- | --- | --- |
| black | Black | absent | 0/0 | 0 | none | `gallery-candidate/black.png` |
| celebration-title-overflow | CelebrationTitle | absent | 4/4 | 1 | celebration title has 4 rendered lines against a three-line ceiling; split before shrinking | `gallery-candidate/celebration-title-overflow.png` |
| celebration-title | CelebrationTitle | `National Catechetical Day` | 1/4 | 1 | none | `gallery-candidate/celebration-title.png` |
| general-image | GeneralImage | `Image header` | 0/0 | 1 | none | `gallery-candidate/general-image.png` |
| general-text-mixed | GeneralText | `Pahibalo` | 1/2 | 1 | none | `gallery-candidate/general-text-mixed.png` |
| lyrics-cebuano-one-line | Lyrics | `Bersikulo` | 1/1 | 0 | none | `gallery-candidate/lyrics-cebuano-one-line.png` |
| lyrics-english-two-line | Lyrics | `Verse` | 2/2 | 0 | none | `gallery-candidate/lyrics-english-two-line.png` |
| lyrics-label-off | Lyrics | absent | 1/1 | 0 | none | `gallery-candidate/lyrics-label-off.png` |
| lyrics-long-word | Lyrics | `Verse` | 1/3 | 0 | none | `gallery-candidate/lyrics-long-word.png` |
| lyrics-missing-glyph | Lyrics | `Verse` | 1/1 | 0 | none | `gallery-candidate/lyrics-missing-glyph.png` |
| lyrics-mixed-four-line | Lyrics | `Mixed` | 4/4 | 0 | lyric body carries 4 phrases against a two-phrase target; split before shrinking | `gallery-candidate/lyrics-mixed-four-line.png` |
| lyrics-overflow-warning | Lyrics | `Verse` | 20/20 | 0 | body content needs 2080px but the body zone offers 862px at the approved minimum; split before shrinking<br>lyric body carries 20 phrases against a two-phrase target; split before shrinking<br>lyric body has 20 rendered lines against a four-line ceiling; split before shrinking | `gallery-candidate/lyrics-overflow-warning.png` |
| lyrics-punctuation | Lyrics | `Verse` | 1/2 | 0 | none | `gallery-candidate/lyrics-punctuation.png` |
| lyrics-tagalog-three-line | Lyrics | `Taludtod` | 3/3 | 0 | lyric body carries 3 phrases against a two-phrase target; split before shrinking | `gallery-candidate/lyrics-tagalog-three-line.png` |
| psalm-header-empty | Psalm | empty | 1/1 | 1 | none | `gallery-candidate/psalm-header-empty.png` |
| psalm-tagalog | Psalm | `SALMO RESPONSORYO: Awit 95` | 1/2 | 1 | none | `gallery-candidate/psalm-tagalog.png` |
| reading-identification | ReadingIdentification | absent | 1/2 | 1 | none | `gallery-candidate/reading-identification.png` |
| reading-passage-cebuano | ReadingPassage | absent | 1/2 | 1 | none | `gallery-candidate/reading-passage-cebuano.png` |
| reading-passage-english | ReadingPassage | absent | 1/2 | 1 | none | `gallery-candidate/reading-passage-english.png` |
| response-cebuano | Response | `TUBAG:` | 1/1 | 0 | none | `gallery-candidate/response-cebuano.png` |
| response-header-absent | Response | absent | 1/1 | 0 | none | `gallery-candidate/response-header-absent.png` |
| response-header-empty | Response | empty | 1/1 | 0 | none | `gallery-candidate/response-header-empty.png` |
| song-identification | SongIdentification | absent | 1/0 | 3 | none | `gallery-candidate/song-identification.png` |
