# T0.9 fixture plan

The E2E deck is `powerpoint_spike::fixture_plan()`. The Safety lead-in and
closing slides are added by the generator, not listed here.

| Mass item | Cues in order |
|-----------|---------------|
| Identification | Image (frame seed 1) |
| Entrance Song | Image (seed 2), Image (seed 3), operator-only Cue "Hold: wait for the procession" |
| Penitential Act response | Image (seed 4) |
| First Reading | Image (seed 5), operator-only Cue "Blackout: reader at ambo" |

Image frames are generated deterministically by `frame(seed)` (1920x1080 PNG),
so no binaries are stored. Operator-only Cues become black slides whose text
lives in the speaker notes.

`expected-outline.txt` is the inspector's listing of the exported deck: nine
slides, the Safety lead-in first, one section per item, and the Safety closing
last.
