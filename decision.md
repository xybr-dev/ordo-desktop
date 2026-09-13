# Ordo Decision Log

Status: MVP design baseline finalized; Rust/GPUI architecture locked; T0.1 complete; T0.2 active
Recorded: 2026-08-23  
Checkpoint: Functional, visual, operational, architecture, navigation, and execution-plan branches are finalized through Decision 260
Next interview branch: None required for the MVP baseline; execute `MILESTONES.md` T0.2 or deliberately reopen a named design branch
Planning deliverables: `MVP-PLAN.md` for strategy and `MILESTONES.md` for task execution
Supersession rule: Every superseding decision names the affected earlier decision; each affected decision's own section carries a forward notice stating whether replacement is full or partial and what remains authoritative.

## Product intent

Ordo is liturgical presenter software for Catholic audio-visual operators. Its first release is not merely a song database or a slide player: it must let one parish prepare and run a Sunday or solemnity Mass end-to-end without PowerPoint.

The central product idea is to keep the operator's liturgical context separate from what the congregation sees. A plan contains songs, spoken cues, congregation responses, readings, refrains, holds, blackouts, and other ordered items. During presentation, the operator sees the full order and upcoming context while the projector receives only congregation-facing output.

## Source evidence reviewed

- Thirteen PowerPoint files were inspected read-only: 796 slides total.
- The twelve numbered song decks encode a partial Mass sequence and contain roughly 83 song or setting candidates.
- A follow-up pagination analysis examined 502 congregation-facing lyric slides across the twelve song decks, excluding titles, blanks, notes, and the prayer/response deck:
  - 21% contain one explicit/logical lyric line, 54% contain two, 12% contain three, 12% contain four, and only 1.4% contain five or more.
  - The median and mode are two logical lines; 98.6% contain four or fewer.
  - 500 of 502 slides inherit 54-point Arial Black; the two exceptions use 48-point text.
  - Long logical lines often soft-wrap, producing an estimated median of about four rendered lines; roughly 14% likely exceed four rendered lines.
  - Prose-like settings such as the Our Father and Memorial Acclamation need stronger automatic splitting than ordinary song verses and choruses.
- `2025 Prayers and Tubag Bisaya.pptx` contains congregation responses, private priest cues in speaker notes, choir-dependent instructions, meaningful blank/hold slides, and non-liturgical announcements.
- The only dated Markdown file is `2026-08-23.md`. The prompt's `2026-08-03.md` was a filename mismatch; no such file currently exists.
- `2026-08-23.md` is a Cebuano Liturgy of the Word source containing the first reading, responsorial Psalm, second reading, and Gospel. It is not a complete Mass plan.
- Sulugoon's dated readings and Order of the Mass were reviewed as references:
  - <https://sulugoon.com/readings/2026/08/23/>
  - <https://sulugoon.com/missal/>
- Sulugoon uses role labels including `P` for `Pari` (Priest) and `T` for `Tanan` (the whole congregation), and includes conditional rites, alternatives, rubrics, and non-projected priest text.
- Canon 1248 §1 was reviewed as official support for distinguishing a Celebration's liturgical date from a Mass held on the evening of the preceding civil day; Ordo still does not infer the correct texts or local scheduling rule from canon law.

## Confirmed MVP decisions

### 1. Product slice

- Build one complete vertical slice: song/content library, Mass planner, and live dual-screen presenter.
- First success target: prepare and run a regular Sunday or solemnity Mass end-to-end without PowerPoint.
- The measurable MVP acceptance scenario is that a parish operator can:
  - Create and maintain song records.
  - Encode reusable song sections and line/slide structure.
  - Assign songs to choir repertoires and build choir-specific arrangement sequences.
  - Generate the resulting presentation slides without duplicating repeated sections such as choruses.
  - Import and review the liturgy, select choir arrangements, and prepare the complete Mass.
  - Connect separate operator and congregation displays, rehearse the plan, close Ordo, and reopen it offline.
  - Present the complete Mass using responses, songs, Freeze, Blackout, prepared jumps, and an emergency insert without opening PowerPoint.
  - Avoid unintended congregation output during rehearsal/presentation and recover the active presentation after an application restart.
- Optimize for one parish AV team first, while avoiding a data model that prevents future multi-parish support.

### 2. Platform and displays

- Use a cross-platform desktop architecture.
- Development will occur on Linux.
- Initial parish deployment will be on Windows.
- Support two physical displays:
  - Operator/control display.
  - Congregation/projector display.
- Approved imported plans and presentation content must remain usable offline.

### 3. Celebration scope

- MVP celebration types: Sundays and solemnities.
- Future support must be possible for funerals, weddings, baptisms, novenas, and other special rites without redesigning the core ordered-item model.

### 4. Mass-plan structure

- Create Mass plans from parish templates.
- Templates contain ordered and conditional item types rather than a rigid twelve-slot sequence.
- Items may be omitted, substituted, inserted, or reordered when the celebration requires it.

### 5. Sulugoon integration

- Sulugoon integration is not a publicly distributed Ordo feature or a public MVP acceptance requirement.
- A private, locally installed Sulugoon extension may be maintained for the product owner's personal preparation convenience, subject to Sulugoon's terms and any applicable source permissions.
- Sulugoon is one Cebuano content source, not Ordo's canonical authority or the basis of its language model.
- Imported content becomes an ordinary editable parish copy; English, Tagalog, and other language content follows the same library structure whether entered manually or obtained from another source.
- Source URL, retrieval date, and attribution may be retained as informational source references without locking the parish copy.
- Missing fields, changed text, alternatives, or suspicious content are surfaced for operator review, but ordinary correction does not require a formal override or justification workflow.
- An upstream change never silently alters a previously captured Presentation snapshot or exported Celebration Package.

### 6. Projection policy

- During spoken dialogues, project only the congregation's response by default.
- Priest or minister text remains operator-only context.
- During Scripture readings, the parish default is to project only congregation responses, Psalm refrains, and acclamations.
- A planner may enable full-reading projection per individual reading.

### 7. Speaker roles and cue visibility

- `P` means `Pari` (Priest).
- `T` means `Tanan` (the entire congregation).
- The model must later support other explicit roles such as deacon, lector, cantor, choir, commentator, and operator.
- Every cue needs at least:
  - Speaker/actor role.
  - Congregation visibility: projected, operator-only, or both.
  - Ordered parent Mass item.
  - Operational behavior such as project, wait/trigger, hold, or blackout.
- The live timeline normally shows only operator-relevant cues, not every missal line and rubric.
- Full canonical missal context may be expandable without cluttering the normal live timeline.

### 8. Song identity and choir repertoire

- A song is one canonical work even when used in several liturgical moments.
- Numbered Ordinary movements are separate Song works in the MVP. For example, `Kyrie 8th Mass` and `Kyrie 13th Mass` are two independently managed Songs rather than movements resolved through a required parent Mass-setting record.
- The number is part of each Song's identifying title/aliases. Composer, collection, and provenance should still be recorded when known so similarly numbered Gregorian, Cebuano, or other settings can be distinguished.
- Songs may carry manually assigned applicability tags for occasions such as Ordinary Time, Lent, Easter, Advent, and special celebrations. These tags do not imply that the ordinal Mass number itself encodes a season.
- During Mass preparation, matching occasion tags may filter or rank Songs to make likely choices easier to find.
- Occasion tags are advisory rather than restrictive: an operator may always find and select an untagged or differently tagged Song, without Ordo treating that choice as a liturgical error.
- A first-class Mass-setting family that groups Kyrie, Gloria, Sanctus, Agnus Dei, and other related movements is deferred. The future model must be addable without changing the identity of existing Songs.
- A work may have multiple arrangements, lyric versions, translations, and eligible Mass moments.
- The underlying musical composition or setting is the Song-work identity boundary. Shared title or liturgical text alone does not make two different musical settings the same Song work.
- A materially different choir or seasonal rendition is represented as its own Arrangement entry under the same Song work rather than as a second pagination map layered over another arrangement.
- Arrangement entries may carry applicability metadata such as choir and liturgical season so operators can choose the correct rendition for the Celebration.
- Choir repertoire in the MVP is simple membership: a choir either has a song in its repertoire or it does not.
- Rehearsal readiness, proficiency, attendance, and similar rehearsal management are deferred.
- Song-usage history and analytics are not part of the MVP.

### 9. Song authoring and generated slides

- Use a hybrid model:
  - Within one Arrangement entry, the editor stores reusable sections such as verses, chorus, refrain, bridge, and coda once.
  - That Arrangement defines a performance sequence such as `V1 -> C -> V2 -> C`.
  - Ordo expands the arrangement into an ordinary sequential slide queue before presentation.
- Repeating a chorus must not require re-encoding or independently editing the same text several times.
- Exact musical punctuation, syllable division, repeated vowels, and intentional line breaks must be preserved.
- Separate Arrangement entries do not share line-level pagination maps. When the choir, seasonal form, wording, phrasing, or slide division is materially different, the operator creates a separate Arrangement entry with its own sections, sequence, and approved pagination.
- This permits some deliberate duplication across distinct Arrangements in exchange for simpler authoring and fewer hidden cross-arrangement dependencies.

### 10. Presenter layout

> **Superseded in part by Decision 203.** Presenter now transforms the existing
> operator window into native full-screen Live Control rather than merely
> maximizing it. The three-column hierarchy and remaining layout rules below
> stay adopted.

Every projectable slide uses a fixed `1920x1080` logical canvas:

- Slide content is laid out and text is wrapped once in canonical canvas coordinates. Changing the physical output resolution must never reflow text, alter line breaks, move elements, or repaginate the content.
- The completed 16:9 composition is uniformly scaled to the physical output. `1366x768`, `1920x1080`, `2560x1440`, `3840x2160`, and other 16:9 outputs therefore preserve identical geometry.
- A display within the adopted `0.1%` tolerance of 16:9, including `1366x768`, fills completely through final-frame scaling; materially different aspect ratios fit and center the complete frame against black rather than cropping or reflowing it.
- Higher-resolution outputs rerender the same canonical geometry at the target pixel density so text and vector elements remain sharp; they do not create a different responsive slide layout.
- The operator application shell remains responsive. Its window size may alter the number and size of thumbnails, but the slide canvas shown inside those thumbnails is fixed.
- Typography and element dimensions are stored in canonical canvas units. Physical monitor DPI, operating-system scaling, and device pixel ratio must not change wrapping or pagination.
- The Celebration Package carries the approved rendering inputs and required fonts when licensing permits so another workstation can reproduce the canonical frames without font substitution.

The operator presenter uses three columns:

1. **Ordo / entire Mass**
   - Shows the full ordered Mass.
   - Shows titles only: songs, responses, readings, rites, and other top-level items.
2. **Cue timeline**
   - Scrollable detailed sequence grouped under the titles from column 1.
   - Projected content appears as actual 16:9 slide thumbnails rather than generic content cards.
   - Every representation of a projectable slide uses one canonical rendered frame. Column-2 thumbnails, column-3 Preview and Program, and the congregation display must be visually identical apart from scaling, as in PowerPoint. Thumbnail rendering must not reflow, abbreviate, restyle, or otherwise approximate the slide.
   - Operator-only priest/minister triggers appear as compact cue rows because they have no congregation slide.
3. **Output monitors and state**
   - Shows Program: the congregation's current output.
   - Shows Preview: the operator's current selection/upcoming output.
   - Shows state indicators such as live/unfrozen, frozen, and blackout.
   - Shows the relevant private operator trigger/context.

Thumbnail density in column 2 is responsive rather than fixed:

- Ordo chooses the largest practical canonical 16:9 thumbnails for the available column width.
- In **Visual Slide Grid** mode, the canonical frame within each thumbnail is never narrower than `300px` (`300x168.75` at 16:9) at supported Presenter sizes. Labels and state treatment sit outside this fixed-ratio frame.
- The grid targets approximately three slides per row at `1366x768` and up to four at `1920x1080`. When available width contracts, it removes a column before it would squeeze a frame below the minimum.
- `1366x768` and `1920x1080` are required operator-screen design and test targets.
- Operator-only cue rows continue to span the available timeline width.

Ordo chooses an automatic initial layout from the available operator-screen space:

- At `1366x768`, the Ordo rail starts collapsed, the visual slide grid targets approximately three columns, and Program and Preview are stacked.
- At `1920x1080`, the Ordo rail starts pinned, the visual slide grid may reach four columns, and the output-monitor column is given more room while preserving usable timeline thumbnails.
- Manual rail pinning/collapse and Visual Slide Grid/Compact Cue List choices persist per workstation.
- Window resizing may reflow controls and change thumbnail density, but it must not change the selected cue, Program, Preview, or presentation state.

The cue timeline provides two operator-selectable modes:

- **Visual Slide Grid** is the default and shows canonical rendered slide frames.
- **Compact Cue List** shows numbered rows with role/section and a lyric or cue excerpt for denser navigation.
- Both modes select the same underlying ordered cues and use column 3 for the canonical Preview.

Column 1 is a collapsible/pinnable Ordo rail:

- When collapsed, a narrow labeled **ORDO** tab remains visible. Hover alone never reveals the rail in Presenter Mode.
- Explicitly activating the tab or its keyboard command opens the unpinned rail as an overlay without resizing the Cue timeline beneath it.
- Clicking its pin control keeps it open until unpinned.
- Clicking outside the unpinned rail or pressing `Esc` closes it and returns focus to the Cue timeline.
- Collapsing it gives the cue timeline more space for the visual slide grid.

Column 1 behaves as a table of contents rather than a filter:

- Selecting a title scrolls column 2 to the corresponding group while preserving the rest of the Mass timeline.
- Groups may be collapsed manually.
- The active cue remains highlighted and is automatically kept in view during keyboard navigation.
- Surrounding groups remain available for context and emergency jumps.

The two application windows have different display behavior:

- The congregation window is borderless full-screen on its assigned output display.
- The operator window is ordinarily windowed and resizable for preparation, rehearsal, troubleshooting, and screen sharing.
- Presenter Mode retains normal operating-system recovery mechanisms so the operator is not trapped if display assignment or projection fails.

### 11. Program, Preview, Freeze, and Blackout semantics

> **Superseded in part by Decision 169.** Operator-only triggers use a black
> canonical Preview/Program frame; their private prompt and related response
> remain adjacent operator context rather than entering the Preview frame. The
> body below is reconciled to that distinction; the remaining linked/frozen
> state rules stay adopted.

Ordo uses a linked/frozen model rather than a traditional manual OBS transition model.

#### Live / unfrozen

- Preview and Program are effectively linked.
- Left/Right navigates the timeline.
- The selected projectable output immediately appears in both Preview and Program.
- Entering an operator-only priest cue makes the canonical Preview and Program frames black because the cue has no congregation output.
- The private prompt and related upcoming congregation response remain visible to the operator as adjacent context outside the Preview frame.

#### Frozen

- Program holds the last congregation frame.
- Left/Right continues navigating Preview independently.
- This is the mechanism for retaining previous congregation output while preparing another slide.
- Pressing Space takes the currently selected Preview live and returns to the unfrozen linked state.

#### Blackout

- Pressing `B` immediately blackouts Program.
- Blackout does not prevent Left/Right navigation in Preview.
- Pressing Space takes the current Preview live and returns to the unfrozen linked state.
- Blackout and Freeze are distinct concepts even though both temporarily decouple Program from Preview.

#### Priest-to-congregation example

1. The operator is positioned at `P: Ang Ginoo maanaa kaninyo.`
2. Program is black unless Freeze intentionally retains a previous output.
3. The operator sees the upcoming `T` response.
4. When the priest speaks the trigger, Right advances to `T: MAANAA USAB KANIMO`.
5. In the normal unfrozen state, that response immediately appears on Program and Preview.

Operator-only cues remain clickable/selectable positions in both timeline modes:

- Their canonical congregation-facing output is black.
- Selecting one makes Preview black.
- When Live/unfrozen, Program also becomes black.
- When Frozen, Program retains its held frame while Preview becomes black.
- The private cue text remains visible in the operator-context area, not on the slide frame.

### 12. Safe live operations

While presenting, operators may:

- Navigate forward/back.
- Freeze and unfreeze.
- Blackout and restore.
- Jump to another prepared Mass item.
- Select a prepared alternate.
- Insert emergency text or an image.

Ordinary editing of lyrics, canonical responses, and plan structure is locked during live presentation.

Emergency text/image insertion is the exception:

- Opening the tool, including through the `Insert` key, automatically Freezes Program so unfinished content cannot leak to the congregation.
- Saving places the new cue immediately after the current cue and selects its canonical rendered frame in Preview.
- `Space` takes the inserted cue live using the ordinary Freeze-to-Live behavior.
- The insert is autosaved inside the current Mass plan for crash recovery.
- It does not enter the permanent song/content library unless someone explicitly promotes it after Mass.

Emergency editing is a second controlled exception:

- `Ctrl+E` opens the selected projectable cue for editing and automatically Freezes Program.
- Program retains its previous frame while edits update only the canonical Preview.
- The edit is autosaved as a Mass-local override and never silently overwrites the song library or imported/official liturgical text.
- `Space` takes the corrected Preview live.
- When the slide comes from a reused song section, the override updates every linked occurrence of that section in the current Mass by default.
- The operator may explicitly limit an override to the selected occurrence.
- After Mass, Ordo may offer the override for reviewed promotion into the canonical library/source.

### 12a. MVP keyboard map

- `Left` or `Up`: select the previous cue.
- `Right` or `Down`: select the next cue.
- `Page Up` or `Page Down`: jump to the previous or next Mass-item group.
- `Home` or `End`: jump to the first or last cue in the Mass plan.
- `F`: toggle Freeze.
- `B`: activate Blackout.
- `Space`: take the selected Preview live and clear Freeze or Blackout.
- `Esc`: close the current dialog or overlay without altering Program.
- `Insert`: open the emergency text/image insert workflow and automatically Freeze Program.
- `Ctrl+E`: emergency-edit the selected projectable cue and automatically Freeze Program.
- Navigation always follows the established state semantics: it updates Preview and Program while Live/unfrozen, but only Preview while Frozen or Blacked out.
- Shortcut customization is deferred until real operator usage provides evidence for it.

### 13. MVP permissions

- Any parish AV operator may prepare and present a Mass for now.
- Separate administrator, planner, approver, library-editor, and presenter roles are deferred.

### 14. Interrupted-session recovery

- During presentation, Ordo continuously journals the active Mass, selected cue, Program frame, Preview frame, Freeze/Blackout/Live state, and emergency inserts/overrides.
- After an unexpected termination, Ordo immediately reopens the interrupted Mass in Presenter Mode and restores whether congregation output had been enabled.
- If congregation output had been enabled, Ordo recreates its window on a confidently recognized assigned display but restarts Program black; if it had been disabled, no congregation window is created. Ordo never automatically reprojects stale or unintended content after a crash.
- The operator screen restores the last Program frame as context, the Preview selection, and the interrupted state.
- Pressing `Space` takes the recovered Preview live and resumes the ordinary linked state.

### 15. Emergency PowerPoint export

- MVP must export the current Mass plan as a static `.pptx` emergency deck.
- Congregation-facing slides preserve the exact canonical Ordo rendering.
- Each congregation slide is exported as a full-frame image rather than editable PowerPoint text boxes, preventing font substitution, text reflow, and background/layout differences.
- Operator-only cues become black slides with private cue text in speaker notes.
- Generated arrangements are fully expanded, including repeated song sections.
- The deck is an immutable fallback snapshot; later Ordo changes require a new export.

### 16. Structured song import boundary

- Ordo will not import or interpret arbitrary PowerPoint files.
- Migration of the existing parish decks will be performed separately, potentially using AI agents and external extraction tooling.
- Ordo exposes a documented, versioned JSON import shape for structured song data.
- The application validates and previews this structured data before adding it to the library; it does not own arbitrary deck interpretation.
- One JSON document represents one canonical Song work and contains:
  - Schema version and stable import ID.
  - Title, aliases, language information, authorship/source, and rights metadata.
  - Eligible liturgical moments.
  - Optional manually reviewed liturgical-season and special-occasion applicability tags.
  - For numbered Ordinary movements, the identifying number in the title/aliases plus composer, collection, or source metadata when known; no parent Mass-setting record is required in the MVP.
  - One or more independent choir- or season-specific Arrangements.
  - Each Arrangement's applicability metadata, including choir and eligible liturgical seasons when known.
  - Each Arrangement's own reusable sections with explicit line and slide breaks, plus a performance sequence referencing those section IDs.
  - Optional theme overrides and importer notes.
- Ordo may accept multiple documents as one batch, but validates, previews, and approves each Song work independently.
- A matching title or alias never causes an automatic overwrite. The importer requires an explicit create, merge, or skip decision.

### 17. Local storage and portable transfer

> **Superseded in part by Decision 180.** The original single-plan Mass Package
> became the Celebration-scoped Package reflected in the reconciled text below.
> Local-first storage, Parish Backup, and reviewed transfer rules remain.

- Ordo is local-first and fully functional without internet access.
- The church presentation PC is currently air-gapped. Preparation may occur on an internet-connected Linux computer and be carried to the Windows church PC by USB.
- Ordinary preparation edits autosave immediately to a local database; there is no general Save button that operators must remember to press.
- Live presentation recovery continues to use the more detailed continuous journal described in section 14.
- Cloud or LAN synchronization is not required for the MVP, but stable record identities and import boundaries must not prevent synchronization later.
- Ordo supports two distinct portable package types:
  - A **Celebration Package** is the ordinary deployment and return-transfer unit for one complete Celebration. Exporting it automatically captures or reuses one immutable Presentation snapshot for every Mass plan currently under that Celebration and includes every dependency needed to present those plans offline, including song arrangements, rendered slides, theme assets, images, liturgical content and source references, and fonts when redistribution is permitted. Shared dependencies are stored once by content identity where possible.
  - A **Parish Backup** is a complete snapshot for disaster recovery or migration to another computer. It contains the parish library, choirs, templates, Mass plans, themes, source references, settings, and media.
- The normal deployment workflow is:
  1. Prepare the Celebration and its scheduled Mass plans on the internet-connected Linux computer.
  2. Export a self-contained Celebration Package to removable media.
  3. Import and review it on the air-gapped church PC.
  4. Select and present each included Mass plan entirely offline.
  5. If emergency inserts or Mass-local overrides should be retained, export the resulting Celebration Package back to Linux for post-Mass review and possible promotion.
- Package imports match records by stable IDs rather than titles alone and never silently replace conflicting library content.
- A Celebration Package presents every included Mass plan from that plan's own embedded immutable snapshot and dependencies. These snapshots install as exact presentation content independently of any editable-library reconciliation.
- These per-plan snapshot boundaries ensure that the church PC renders every included Mass exactly as captured on the preparation computer, even when its permanent library contains older, newer, or locally changed records.
- The same import review separately stages editable source records for the receiving parish library. New identities, unchanged identities, safe fast-forwards, and genuinely divergent records are classified before one reviewed commit; accepting or rejecting a library action cannot alter any Presentation snapshot inside the package.
- Restoring a Parish Backup is replace-only in the MVP; Ordo does not attempt to merge two complete parish databases.
- Before restore, Ordo automatically creates a recoverable safety backup of the current installation and verifies the incoming package before replacing local data and assets as one consistent unit.
- If post-restore verification fails, the operator can roll back to the automatically created safety backup.

### 18. Lyric pagination

> **Superseded in part by Decision 255.** Its canonical semantic layout and
> authored-casing rules govern Lyrics; the pagination limits and safety rules
> below remain in force. Exact projected type sizes are provisional pending the
> Canonical Slide Layout Gallery.

- Newly encoded or imported lyrics begin with automatically proposed slide boundaries.
- The default lyric body size is `72` canonical canvas units, approximately equivalent to the existing decks' 54-point PowerPoint text.
- Automatic pagination targets two logical lyric phrases per slide and permits at most four rendered lyric lines.
- Intentional lyric lines, musical phrasing, punctuation, syllable division, and author-supplied phrase boundaries are preserved when proposing slide divisions.
- Verse, chorus, refrain, bridge, and similar section labels use a separately reserved label area and do not consume the four-line lyric budget.
- When a proposed slide would overflow, Ordo splits it at a phrase or intentional line boundary before reducing type size.
- Type may be reduced only as far as `64` canonical canvas units, approximately equivalent to 48-point PowerPoint text.
- Content that still does not fit is flagged for human review. Ordo never silently clips, squeezes, or shrinks it below the minimum.
- Automatic pagination is a draft convenience, not authoritative published structure.
- An operator reviews the generated slide sequence and approves it before the arrangement is treated as presentation-ready.
- Manual slide-boundary corrections take precedence over the automatic proposal and are stored with the arrangement.
- Changing reviewed lyrics, intentional line breaks, arrangement order, or another input that affects rendered pagination marks the affected arrangement **Needs Review** before a new Presentation snapshot is captured.
- Changing a theme marks affected slides for validation and requires review when overflow or another output problem is detected.
- Previously exported Celebration Packages remain unchanged because they contain immutable rendered snapshots.
- Pagination is approved within one independent Arrangement entry. Ordo does not maintain different slide-grouping maps that share the same cross-arrangement lyric lines.
- A materially different choir or seasonal form is encoded as a separate Arrangement entry and tagged for its applicable liturgical seasons.

### 19. Projector-safe area

> **Superseded in part by Decision 255.** The body below is reconciled to the
> `120x80` minimum safe area; full-bleed background and validation rules remain
> in force.

- The minimum congregation-output safe area is `120` horizontal and `80` vertical units on the `1920x1080` canonical canvas.
- Backgrounds and other intentionally full-bleed decorative elements may extend to the canvas edges.
- Lyrics, responses, labels, and important image content remain inside the safe area by default.
- The slide editor visibly indicates the safe-area boundary.
- Any important-content geometry outside the safe area produces a validation warning and cannot be approved by reducing the minimum.
- A Theme may define a larger safe area but never a smaller one.

### 20. Theme inheritance

- The parish maintains reusable Themes.
- Each Mass plan selects one default Theme, which is inherited by its songs, responses, readings, and emergency inserts.
- An Arrangement may declare layout needs, such as centered lyrics or a reserved section-label area, but does not force its own colors or background.
- An individual Mass item may explicitly override the Mass plan's Theme for an exceptional presentation need.
- Arbitrary per-slide styling is outside the MVP.
- Changing a Theme rerenders affected projectable output and invokes the established overflow and review checks.

### 21. Song identification slides

- Each Song occurrence in a Mass plan generates a separate opening identification slide before its first lyric slide.
- The slide prominently displays the Song title and may show its setting, composer, source, or translator in smaller secondary text when known.
- The identification slide contains no lyrics and is an ordinary selectable projectable Cue in the Mass item's sequence.
- It is enabled by default and may be disabled for that individual Mass item when the operator needs to begin directly with lyrics.
- Enabling, disabling, or changing Mass-local identification-slide presentation does not alter the canonical Song work or Arrangement.

### 22. Guaranteed fallback Theme

- Ordo always retains a built-in fallback Theme so missing or damaged custom assets cannot make a Mass unpresentable.
- The fallback uses a pure black background, white heavy sans-serif text, and centered lyrics and responses.
- It has no decorative image, texture, shadow, or animation.
- It uses a bundled open-license font rather than depending on Arial Black or another operating-system font being installed.
- Custom Themes receive automatic contrast validation warnings.
- `Atkinson Hyperlegible Next` is the canonical bundled projection font, using ExtraBold for lyrics/responses and SemiBold for titles, credits, and labels.
- `Noto Sans` is the bundled missing-glyph fallback.
- Arial Black is retired from Ordo's font stack. It is neither the default nor a fallback because relying on an operating-system font would undermine deterministic Linux/Windows rendering and portable Celebration Packages.
- The earlier comparison's apparent Arial Black rendering on Linux was a system substitution, not genuine Arial Black; this reinforced the requirement to bundle the exact selected font files.
- An actual church-projector test remains an acceptance check for exact sizing, spacing, washout resistance, and pagination metrics, but it does not leave the font-family choice provisional.

### 23. Language model

- Language is metadata on the actual text content rather than only a broad Song label.
- Every Arrangement declares its language set.
- Individual reusable sections or lyric lines may carry their own language when an Arrangement intentionally mixes languages.
- Complete translations of the same musical work, such as fully Cebuano and fully English lyric versions, are separate Arrangements under the same Song work.
- A deliberately bilingual or multilingual performance sequence remains one Arrangement whose component text is tagged appropriately.
- A Mass plan records its primary language for filtering and review, but this never prohibits mixed-language content.

### 24. Presentation snapshots without Mass approval

- MVP has no Mass-level approval status, approval button, or approver role.
- The working Mass plan remains editable and autosaved.
- Starting Presenter Mode selects one immutable **Presentation snapshot** for its Mass plan. Exporting a Celebration Package performs the same capture-or-reuse operation independently for every Mass plan under the exported Celebration.
- If the current Mass plan and all rendering dependencies exactly match an existing Presentation snapshot, Ordo reuses that snapshot instead of creating a duplicate.
- Therefore, quitting and revisiting Presenter Mode without making relevant edits opens a new Presentation session from the same Presentation snapshot; it does not create another content version.
- A cleanly started Presentation session begins with congregation output disabled. When the operator explicitly enables it, external Program begins black. Interrupted-session recovery remains governed by section 14.
- Validation problems remain visible. Starting presentation or exporting with unresolved warnings requires operator acknowledgment rather than formal approval.
- Later edits create a difference from the captured snapshot and never mutate an already exported package or an active presentation snapshot.
- These automatic snapshot boundaries provide a foundation for version history and restoration without forcing an organizational approval workflow.

### 25. MVP edit history and restoration

- Ordinary edits support Undo and Redo during preparation.
- Durable automatic checkpoints cover both Mass plans and reusable library content, including Songs and Arrangements.
- Ordo creates checkpoints at meaningful safety boundaries such as before imports, bulk edits, Celebration Package export, and entry into Presenter Mode; entering Presenter Mode again without relevant changes does not create a duplicate content revision.
- Restoring an earlier checkpoint makes its state the new current revision while preserving the later history, so restoration itself can be undone.
- Complex branches, user-named revisions, and advanced visual diffs are deferred beyond the MVP.

### 26. Provenance, rights metadata, and parish scope

- Ordo is scoped to one parish's preparation and presentation work. Its future cloud capability means private synchronization among that parish's installations, not public publishing, public sharing, or a content marketplace.
- Songs and imported liturgical texts may retain informational provenance and rights metadata when known, including source, author or composer, translator, copyright owner, license or permission notes, and attribution.
- Missing or incomplete rights metadata does not block preparation, live presentation, Celebration Package export, Parish Backup, emergency PowerPoint export, or future same-parish synchronization.
- Ordo does not attempt to determine whether the parish has legal permission to project particular material. Responsibility for acquiring and observing applicable permissions remains with the parish.
- Public distribution controls, DRM, and content-marketplace licensing workflows are outside the product direction currently under consideration, not merely MVP features awaiting implementation.

### 27. Optional content-source extensions

- Sulugoon integration is an optional content-source extension rather than part of Ordo's core domain.
- Ordo core owns Mass plans, editable parish content, manual entry, and the normalized import-and-review workflow.
- A content-source extension converts external material into Ordo's source-neutral import draft. Imported material becomes an ordinary parish copy and does not depend on the extension remaining installed or available.
- The Sulugoon extension supplies Cebuano source material without making Sulugoon's structure or language assumptions canonical for English, Tagalog, or future providers.
- The Sulugoon extension will not ship with public Ordo releases. It may exist as a private local extension for the product owner's personal use and must remain architecturally separate from the core application.
- MVP does not require a runtime that loads executable third-party extensions. A private source-specific helper operates outside Ordo and emits a versioned, source-neutral Liturgy Import Document.
- Ordo validates and previews a Liturgy Import Document before copying its content into a Mass plan.
- The same import contract may be used by future English, Tagalog, or other source-specific tools without changing Ordo core.
- A general third-party extension marketplace or user-facing extension installer is deferred.

### 28. Desktop shell, supported hardware, and performance budgets

> **Superseded in part by Decision 252.** The Electron selection, process model,
> and Electron-specific rationale below are retained as decision history. The
> hardware qualification and safety/performance outcomes remain applicable,
> except that Decision 252 replaces the Electron-era 750 MB preferred memory
> target with the native 512 MiB qualification budget and restates the
> historical `1 GB` ceiling as the current `1 GiB` absolute ceiling.

- Use Electron as Ordo's cross-platform desktop shell.
- Electron's bundled Chromium renderer is preferred over a system-webview shell because Ordo must minimize layout and rendering differences between Linux preparation and Windows presentation.
- The larger application and installer size is an accepted trade-off for a consistent bundled rendering engine, offline deployment, and mature multi-window/display control.
- Windows 10 22H2, 64-bit, is the minimum supported Windows installation for the MVP. The initial church PC runs Windows 10.
- Windows 11, 64-bit, is the recommended Windows platform for newly provisioned computers. Windows 10 reached Microsoft end of support on October 14, 2025, so Ordo must pin and qualify a tested Electron release for the air-gapped church PC instead of assuming all future Electron releases will continue to support it.
- Ordo targets x64 Windows packages. A machine merely capable of launching Electron is not necessarily an Ordo-supported live-presentation machine.
- The supported minimum church-PC hardware profile is:
  - Four logical CPU cores, approximately an Intel Core i3 fourth generation or equivalent.
  - `8 GB` RAM.
  - A hardware-accelerated DirectX 11-class integrated or discrete GPU capable of driving two simultaneous 16:9 displays.
  - An SSD with at least `2 GB` free for Ordo in addition to the parish content library.
- The recommended profile is an Intel Core i3/i5 eighth generation, AMD Ryzen 3, or newer equivalent; `8 GB` RAM for Ordo alone; and `16 GB` RAM when Ordo will share the machine with OBS or similarly demanding software.
- `4 GB` RAM is not an officially supported configuration even if Ordo happens to launch, because the operating system, Chromium processes, security software, drivers, and decoded media leave insufficient live-presentation safety margin.
- MVP performance is measured on a minimum-profile reference computer and must satisfy these release budgets:
  - Keep total Ordo memory below a `750 MB` target during a representative large Mass, with `1 GB` as the hard acceptance ceiling.
  - Average below `5%` CPU while idle on a static slide over a 30-second sample.
  - Make Left/Right, Freeze, Blackout, and Space visibly take effect within `50 ms` at the 95th percentile.
  - Lose no navigation input during rapid sequential operation.
  - Enter Presenter Mode from an existing Presentation snapshot within `5 seconds`.
  - Keep Program stable while search, autosave, pagination, thumbnail generation, or future-slide decoding occurs.
  - Scroll an approximately `800`-cue timeline smoothly at both required operator-screen test sizes.
  - Never disturb the current Program frame when a future slide fails to render or decode.
- Ordo must not retain an entire Mass as decoded full-resolution slide bitmaps. A single decoded `1920x1080` RGBA frame is approximately `7.9 MiB`, so an unbounded 500-slide cache would approach `4 GB` before application overhead.
- Presentation rendering therefore uses a bounded cache: only Program, Preview, and a small set of nearby slides remain presentation-ready; the cue timeline uses virtualized, lower-resolution canonical thumbnails and evicts offscreen decoded resources.
- The main Electron process must not perform blocking disk I/O, synchronous IPC, pagination, thumbnail generation, or other long-running work. CPU-heavy work moves off the UI path when measurement shows it is necessary.
- Electron is an adapter around framework-independent presentation-state and canonical-slide-composition modules. Song pagination, Mass-plan logic, snapshots, and live presentation semantics must not depend directly on Electron; this separation exists for Depth, Locality, deterministic tests, and safe process ownership, not because another production shell is planned.
- Linux remains a supported preparation and development platform.
- The local database implementation, installer formats, signing, and offline update workflow remain to be selected.

### 29. Frontend stack and application seams

> **Superseded by Decision 252.** Retained as the historical React/Electron
> design considered before the clean-slate GPUI decision.

- Use React with TypeScript for the operator and congregation renderer interfaces.
- Use Vite to build the Electron renderer applications.
- Do not add Next.js, server-side rendering, or another web-server application framework. Ordo is an offline desktop application whose renderer assets are bundled locally.
- React's larger ecosystem for virtualized timelines and grids, accessible controls, keyboard interaction, arrangement editing, testing, and profiling outweighs Svelte's smaller UI runtime for this product.
- Electron's Chromium processes, decoded media policy, bounded caches, timeline virtualization, and update locality have materially greater performance impact than the difference between React and Svelte.
- React Compiler may be enabled after its compatibility with the chosen Electron/Vite toolchain is verified; the MVP architecture and performance budgets must not depend on it.
- React owns view composition and operator interaction only. It must not own the business rules for Songs, Arrangements, Mass plans, pagination, snapshots, or live presentation state.
- The Presentation Engine is a framework-independent TypeScript module behind a small command-and-state interface covering navigation, Freeze, Blackout, taking Preview live, and recovery.
- The Canonical Slide Renderer is shared by the timeline, Preview, Program, congregation window, and fallback export paths so all consumers receive the same canonical composition rather than independently approximating it.
- Electron main/preload code supplies operating-system adapters for window/display control, durable storage, filesystem access, shortcuts, and process coordination through narrow IPC interfaces.
- Keeping these seams independent of React and Electron preserves testability and prevents desktop-framework concerns from leaking into Ordo's domain and presentation semantics. It is not a commitment to support or migrate to another shell.

### 30. Local persistence ownership

> **Superseded in part by Decision 252.** SQLite, managed-asset consistency,
> domain-shaped persistence operations, and migration requirements remain; the
> Electron utility-process and `node:sqlite` implementation does not.

- Use one local SQLite database for structured parish data, including Songs, Arrangements, Mass plans, Themes, settings, edit revisions, and Presentation-snapshot manifests.
- A dedicated Electron utility process is the sole owner of the live database connection. React renderers never execute SQL or open the database directly, and the Electron main process does not perform database work on its event loop.
- The application reaches persistence through a small asynchronous Storage Module Interface exposed through narrow preload/IPC commands. SQL schemas, transactions, migrations, journal handling, and database-driver behavior remain local to the Storage Module implementation.
- Original images and other large binary assets live in a managed parish asset directory and are referenced by stable IDs and integrity checksums. Disposable thumbnails, decoded frames, and other generated caches are not durable database records.
- The Storage Module coordinates database and asset changes so callers cannot create a committed record that references a missing asset, or silently replace an existing asset with different bytes under the same identity.
- Parish Backups, Celebration Packages, and other portable artifacts are created through coordinated snapshot/export operations. Ordo never treats an uncoordinated copy of a live SQLite main file as a valid backup, because active journal files may form part of the database state.
- The utility-process cost is accepted to isolate persistence faults and keep database I/O away from the main, renderer, and live-presentation paths. Its memory usage remains inside the established total Ordo performance budget.
- Use Electron's bundled `node:sqlite` driver inside the storage utility process for the MVP.
- The synchronous driver Interface is contained inside that utility process and is exposed to the rest of Ordo only through the asynchronous Storage Module Interface, so database calls cannot block the Electron main thread or a renderer.
- Using the bundled driver avoids an external native addon's Electron ABI rebuild and packaging requirements. Ordo pins and qualifies the exact Electron/Node/SQLite combination used for a release rather than depending on whichever versions happen to be installed on the operating system.
- Database features required by Ordo—including foreign-key enforcement, transactions, prepared statements, migrations, integrity checks, and coordinated backup—must be exercised in packaged Windows and Linux acceptance tests before release.
- Do not add an ORM for the MVP. The Storage Module Implementation uses explicit parameterized SQL and ordered, versioned schema migrations.
- The Storage Module Interface exposes domain-shaped operations rather than generic table CRUD or raw query access. This preserves Depth and Locality: callers work with Ordo concepts while schema knowledge and query behavior remain concentrated inside the module.

### 31. Windows installation scope

> **Superseded in part by Decision 252.** The conventional x64 installer,
> offline complete-installer update, data preservation, checksum, and signing
> policy remain. Electron Forge is no longer the packaging implementation.

- Package Ordo with Electron Forge.
- Ship the Windows MVP as a conventional x64 per-machine installer rather than a portable executable.
- Install application binaries under the normal protected per-machine application location and keep durable parish data outside the installation directory.
- Installation and application updates require Windows administrator approval. Administrator access is available on the initial church PC.
- Application upgrades must preserve the parish database, managed assets, settings, snapshots, and backups.
- A portable Windows distribution is deferred to avoid ambiguous duplicate installations and split parish-data ownership.
- MVP application updates use the same complete Windows installer as initial installation. There is no background updater, update server, or custom Ordo update-bundle format.
- The release is downloaded on an internet-connected preparation machine, copied to removable media, and run manually with Ordo closed on the air-gapped church PC.
- On the first launch of a newer application version, Ordo creates a pre-migration Parish Backup and verifies database integrity before applying ordered schema migrations.
- A failed migration must not destroy or partially replace the prior parish state; Ordo retains the pre-migration backup and presents a recovery path.
- The internal single-parish MVP may use an unsigned installer. Each release publishes a SHA-256 checksum, which is verified on the internet-connected preparation machine before the installer is copied to removable media.
- The initial operator may need to acknowledge Windows' unrecognized-application warning during installation.
- Do not install a self-signed root certificate on the church PC merely to suppress that warning; its certificate-management and expanded-trust costs are not justified for the controlled MVP deployment.
- A CA-trusted Authenticode signature becomes a release requirement before Ordo is distributed beyond the product owner's controlled machines. Certificate acquisition is therefore deferred, not rejected.

### 32. Linux installation scope

- Ship an x64 Flatpak as the supported installable Linux MVP release.
- The Flatpak is the single first-class Linux installation format across CachyOS and other supported Linux distributions; Ordo does not initially maintain distribution-specific installation packages.
- Also produce an x64 ZIP archive as a portable development, diagnostic, and recovery fallback.
- The ZIP fallback does not provide normal desktop installation or integration and is not the preferred end-user installation path.
- Defer `.deb`, `.rpm`, AppImage, and native Arch packages until actual user demand justifies their additional build and acceptance-test surface.
- Keep Linux packaging behind the release-packaging Seam so package format does not affect Ordo's domain Modules or portable data formats.

### 33. Source hosting and cross-platform builds

> **Superseded in part by Decision 252.** GitHub, native Windows/Linux CI, draft
> releases, checksums, and data isolation remain. The npm/Node/Electron
> toolchain requirements do not.

- Keep the Ordo MVP source in a private GitHub repository.
- Use GitHub Actions as the cross-platform continuous-integration and release-build environment.
- Build and acceptance-test the x64 Windows installer on a Windows runner rather than relying on Linux-to-Windows cross-compilation.
- Build and acceptance-test the x64 Linux Flatpak and ZIP on a Linux runner.
- Ordinary commits and pull requests run verification without publishing a release; a version tag initiates the versioned release-build workflow.
- A successful version-tag workflow creates or updates a draft GitHub Release rather than immediately publishing a final release.
- The draft collects all platform artifacts, checksums, and release notes. The product owner explicitly reviews and publishes it; publication is not an automatic consequence of a passing build.
- Draft publication is blocked unless the actual packaged Windows and Linux applications pass automated smoke tests on their native CI runners; passing only source-level tests is insufficient.
- Packaged smoke tests cover application launch, creation or opening of temporary parish data, migration of a representative fixture database, rendering of known canonical slides, Celebration Package export-and-open, and Parish Backup creation and verification.
- Physical two-display validation remains a distinct release/deployment concern because hosted CI cannot establish that a particular church projector, display topology, or driver behaves correctly.
- Use npm as the package manager, commit `package-lock.json`, and use clean `npm ci` installs in CI.
- Pin the Node.js major release and exact Electron and Electron Forge versions used for a release so Linux development and native CI builds use a qualified toolchain rather than silently drifting.
- Release output includes cryptographic checksums and retained downloadable artifacts so the Windows installer can be downloaded and verified on the connected Linux preparation machine before removable-media transfer.
- Parish databases, content libraries, Celebration Packages, Parish Backups, and other parish data never enter the source-hosting or CI environment.
- Whether Ordo's source becomes public and which open-source license might apply are product-governance decisions explicitly deferred until after the MVP. The private MVP repository must not make later open-sourcing unnecessarily difficult.

### 34. Local access and future cloud identity

- The offline, single-parish MVP has no in-application login, password, operator account, or role check.
- Anyone who has access to the parish-controlled operating-system account and computer may open and operate Ordo; operating-system security and physical access are the MVP protection layer.
- A future login is optional and appears only when a parish chooses private cloud synchronization. Local preparation and presentation must remain usable without a cloud session or internet connection.
- Future cloud identity, synchronization authorization, commercial licensing, source-code licensing, and anti-tamper measures are separate post-MVP design branches. None is simulated with a local MVP login.
- Whether Ordo remains closed-source or becomes open-source, how non-cloud features might be licensed commercially, and what practical resistance to unauthorized copying is appropriate remain deliberately unresolved until after MVP validation.

### 35. Computed Mass-plan readiness

- Do not add a manually assigned `Draft`, `Approved`, or `Completed` state machine to Mass plans in the MVP.
- Ordo computes Mass-plan readiness from current content and validation results. The operator does not click a button to declare a plan ready.
- Readiness distinguishes `Incomplete`, `Ready with warnings`, and `Ready` for preparation guidance; these are computed assessments rather than organizational approval states.
- `Incomplete` is narrow: it means Ordo cannot faithfully produce part of the intended presentation because a concrete dependency is missing, broken, empty, structurally invalid, or unrenderable.
- Examples include an unfilled required placeholder, no Arrangement selected for an assigned Song, a reference to deleted content or a missing asset, a projectable Cue with no renderable content, an invalid canonical frame, or an empty or corrupted plan.
- Optional omissions, applicability-tag mismatches, unreviewed pagination, missing attribution or provenance, and differences from prior use are warnings rather than causes of incompleteness.
- Projector or display disconnection is runtime Presenter readiness, not Mass-plan incompleteness.
- A Presentation session records that a Presentation snapshot was actually used. A past dated Mass plan remains available as history without an operator-assigned `Completed` status.
- An `Incomplete` assessment does not by itself prevent entry into Presenter Mode. The operator receives a prominent acknowledgment identifying affected Mass items and Cues, then may continue using the valid portions of the plan.
- Unavailable Cues remain clearly marked in Preview. Attempting to take an unavailable Cue live must leave Program unchanged rather than expose an error or partial frame to the congregation.
- Only a fatal inability to capture or open a coherent Presentation snapshot, such as unreadable or irrecoverably inconsistent storage, blocks Presenter Mode entirely.

### 36. Reusing a past Mass plan

- The operator-facing action is **Reuse for another Celebration**.
- Reuse always creates a new, independent Mass plan and requires assigning its new Celebration and date.
- The source Mass plan remains tied to its original Celebration and is never repurposed by changing its date.
- The new plan does not remain synchronized with the source plan; later edits to either plan do not affect the other.
- Avoid the operator-facing term `Replicate`, because it may imply continuing synchronization, and avoid relying on the generic term `Duplicate`, because it does not communicate the liturgical intent or required new Celebration.
- Reuse copies the source plan's latest editable state, including its Mass-item order, selected Songs and Arrangements, responses, readings, Cues, Theme choices, and Mass-local overrides.
- Reuse does not copy the source plan's identity, Celebration/date, Presentation snapshots, Presentation sessions, live state, or Undo/Redo history.
- The operator must assign the new Celebration and date, after which Ordo recomputes readiness and warns about date-specific or liturgical mismatches such as old readings or questionable seasonal applicability.
- The result is a predictable editable starting point, not a claim that the copied content is correct for the new Celebration.

### 37. Mass-plan retention and organization

- Organize Mass plans by Celebration date rather than requiring an operator-assigned lifecycle status.
- Current and future dated plans appear in the normal **Upcoming** view; plans whose Celebration date has passed automatically appear in **History**.
- Moving into History is a derived view change, not an archival operation and not a claim that the Mass was presented.
- Ordo never automatically archives or deletes a Mass plan merely because its Celebration date has passed.
- A manual **Archive** action hides canceled, test, duplicate, or otherwise unwanted plans from ordinary Upcoming and History views.
- Archived plans remain searchable and restorable; Archive is a reversible visibility choice rather than deletion.
- Opening a plan from History shows its latest editable state read-only by default, protecting it from accidental autosaved changes while it is being inspected.
- From that view the operator may explicitly reuse the plan, inspect its Presentation sessions and immutable snapshots, or choose **Edit past plan** for a correction or post-Mass review.
- Ordo does not automatically label one Presentation session as the actual Mass merely because it was the latest session.
- Permanent deletion is available only from Archived and requires a confirmation that summarizes the plan-owned history and content that will be removed.
- A plan with Presentation sessions or snapshots requires stronger confirmation because deletion removes historical evidence.
- Deleting a Mass plan never deletes shared Songs, Arrangements, Themes, or assets still belonging to the parish library; only records and unshared dependencies owned exclusively by that plan are eligible for cleanup.
- Historical plans containing emergency inserts or Mass-local overrides expose a non-blocking **Review local changes** action.
- Local changes are never promoted automatically. If review is ignored, they remain safely attached only to that Mass plan.
- Promotion presents a comparison and requires the operator to choose the affected Song work or Arrangement rather than silently guessing the library target.
- Keeping, promoting, or otherwise reviewing a local change never mutates an immutable Presentation snapshot.

### 38. Stable entity identity

- A Song title is not a unique identifier. Distinct Song works may share a title, and one Song work may be known under several spellings, translations, or local names.
- Durable Ordo entities use UUID version 4 stable identities generated with Node's built-in cryptographically random `crypto.randomUUID()` implementation.
- Store UUIDs in their canonical lowercase textual representation for transparent SQL inspection and portable JSON interchange; parish-scale storage does not justify a more opaque binary representation.
- Keep creation timestamps as explicit fields rather than attempting to infer chronology from identifiers.
- Enforce UUID uniqueness in storage and regenerate if insertion ever reports a collision.
- Stable identities are suitable for offline Celebration Packages, import conflict detection, and possible future same-parish synchronization, while remaining invisible to ordinary operators.
- Creating or importing a likely duplicate never merges automatically. Ordo presents possible matches so the operator can attach a new Arrangement to an existing Song work or deliberately create a separate Song work.

### 39. Song titles, aliases, and match evidence

- Each Song work has one primary parish title and may have multiple searchable alternate titles for local shorthand, spelling variants, translations, or other names referring to the same work.
- An Arrangement may have its own display title when its language, numbered Mass setting, or local usage requires wording distinct from the Song work's primary title.
- Duplicate detection considers normalized primary titles and aliases together with supporting evidence such as composer or source metadata, language, and opening lyric lines.
- Exact-looking and fuzzy matches are recommendations only; the operator decides whether incoming content belongs to an existing Song work or represents a separate work.
- Local aliases do not create another Song work merely to make an alternate name searchable.
- Previously confirmed numbered movements such as `Kyrie — 8th Mass` and `Kyrie — 13th Mass` remain separate Song works rather than aliases of one another.

### 40. External extraction and Song import contract

- Ordo does not interpret arbitrary PowerPoint decks. External tools or AI agents extract heterogeneous parish and choir decks into versioned **Song import documents**.
- The Song import document is the strict contract at the import Seam. The MVP accepts multiple `.ordo-song.json` files in one import operation.
- One document proposes one Song work and includes its schema version and stable identity, primary and alternate titles, composer/source/language/provenance metadata, and one or more choir-specific Arrangements.
- An Arrangement in the document contains reusable song sections, its performance sequence, choir and occasion-applicability metadata, and optional extraction notes or confidence warnings.
- An extractor may group confidently recognized choir renditions as Arrangements under one Song work. When identity is uncertain, it emits separate candidates and leaves consolidation to Ordo's staged duplicate review.
- Imported pagination begins **Needs Review**. Preserved PowerPoint line or slide breaks are extraction hints, not automatically approved canonical pagination.
- Import validation and duplicate staging occur before any parish-library mutation. Ordo recommends **Add as new**, **Attach Arrangement to existing Song**, or **Skip exact duplicate** for each candidate; the operator may revise the recommendation before the approved batch is committed atomically.
- A ZIP song bundle is deferred until an import needs to carry images or other external assets; text-first migration uses multiple JSON documents directly.
- Song import documents require stable UUIDv4 identities for the proposed Song work, Arrangements, and other addressable imported records.
- Importing the same unchanged identities and content is recognized as an already imported document and skipped rather than duplicated.
- When an existing identity arrives with changed content, Ordo shows a comparison and requires a reviewed update decision; it never silently overwrites the parish library.
- Accepting an import update creates ordinary library edit revisions so the previous state remains restorable.

### 41. Song-library search

- Provide one search-as-you-type Interface for both Mass planning and Song-library management rather than separate title and lyric search modes.
- Search primary and alternate Song titles, Arrangement display titles, complete lyrics, composer/source/attribution metadata, and choir names.
- Rank primary-title and alias matches ahead of lyric and supporting-metadata matches so ordinary title lookup remains predictable.
- Allow narrowing by language, choir repertoire, numbered Mass movement, and occasion-applicability tags.
- Searching a remembered lyric fragment must be sufficient to discover a Song even when the operator does not remember its catalog title.

### 42. Merging duplicate Song works

- Provide a reviewed **Merge Song works** action for duplicates discovered after import or ordinary library use.
- The operator explicitly chooses the surviving Song work; Ordo consolidates Arrangements, aliases, metadata, and library references while surfacing conflicting values for review.
- Existing Mass plans, history, and imported references continue to resolve after the merge.
- The retired UUID becomes a permanent identity redirect to the surviving Song work rather than being reused or discarded.
- Resolving retired identities is local to the Song-library Module, so callers do not each implement compatibility behavior.
- A merge creates a restorable library edit revision.

### 43. Song-library retention

- **Archive Song work** is the normal way to remove a Song from ordinary planning use.
- Archived Songs are excluded from default planning results but remain available to historical Mass plans, snapshots, searches that explicitly include archived content, and later restoration.
- Permanent deletion is allowed only when no durable Mass plan, Arrangement relationship, Presentation snapshot, or other retained record references the Song work.
- When deletion is unsafe, Ordo identifies the blocking references and offers Archive instead; deletion never cascades through historical material.

### 44. MVP slide transitions

> **Superseded in part by Decision 256.** Content-to-content changes remain
> cuts, but content-to/from Blackout or Hold now uses the adopted reversible
> `500ms` fade. The body below is reconciled to that boundary.

- Content-to-content changes and Freeze release to nonblank content use
  instantaneous cuts. Content-to/from Blackout or Hold uses the reversible
  `500ms` opacity fade adopted in Decision 256; Presentation state still changes
  immediately.
- Do not expose fade durations, animation styles, or other transition controls in the MVP.
- Instant cuts preserve time-sensitive operator timing, deterministic Preview-to-Program behavior, and the established responsiveness target.
- Keep transition behavior behind the presentation-rendering Seam so a later version may add carefully qualified transitions without changing the Presentation Engine's navigation semantics.

### 45. Output aspect-ratio handling

- The canonical congregation-facing frame remains `16:9` and is never stretched or cropped to fill a mismatched physical output.
- On a `16:9` display, Program fills the available output.
- On any other display aspect ratio, Ordo centers the complete canonical frame and adds black letterboxing or pillarboxing as required.
- Presenter Mode warns the operator when the selected congregation display does not match `16:9`, but the mismatch does not prevent presentation.
- Preview includes the same fitted frame and black bars shown by Program, preserving the requirement that the operator preview and congregation output be visually identical.
- Keep display fitting inside the Presentation Engine's output Interface so every physical-display Adapter receives the same deterministic composition.

### 46. Projector calibration and overscan compensation

> **Superseded in part by Decision 255.** The calibration guide below is
> reconciled to the `120x80` minimum safe area. Output Fit and the remaining
> calibration behavior are unchanged.

- Provide a built-in congregation-output calibration pattern showing the complete frame edges, center markers, and the `120x80` minimum safe-area boundary.
- Provide a uniform **Output Fit** adjustment from `0%` through `10%`, defaulting to `0%`, for hardware overscan that remains after the operator checks Windows and projector settings.
- Output Fit scales the entire canonical `16:9` frame inward without cropping, stretching, or changing its proportions; any resulting surrounding area is black.
- Preview shows the digitally fitted frame and black margin exactly as Program renders them.
- Projector or operating-system configuration is the preferred correction. Output Fit is a pragmatic fallback rather than a substitute for correct native resolution and aspect-ratio configuration.
- Asymmetric per-edge correction is deferred from the MVP because it can conceal physical alignment problems and makes congregation-output geometry less predictable.

### 47. Calibration-setting scope

- Projector calibration and Output Fit are installation-specific display settings, not properties of a Mass plan, Presentation snapshot, Theme, or Celebration Package.
- A church installation remembers calibration for its selected congregation display so ordinary Presenter Mode entry does not require repeated setup.
- A Parish Backup may preserve calibration as explicitly identified device metadata, but restoring it on a different computer or applying it to an unrecognized display resets Output Fit to `0%` and requires a calibration check.
- Changing the selected congregation display prompts the operator to verify calibration before relying on the saved setting.
- Preparation on the Linux machine cannot silently alter the Windows church installation's congregation-output calibration.

### 48. Presenter Mode without a congregation display

- A missing second display does not block entry into Presenter Mode.
- Presenter Mode instead remains in its ordinary **Congregation output disabled** state on the operator display.
- Preview navigation and presentation controls remain usable, but Ordo does not create a congregation-output window on the operator display.
- If a projector or second display becomes available later, Ordo never begins output automatically. The operator must explicitly enable the congregation display, and its Program begins black before any content can be restored.

### 49. Congregation-display disconnection and recovery

- If the active congregation display disappears during a Presentation session, Ordo immediately marks congregation output **Disconnected** and prevents its output window from being relocated onto the remaining operator display.
- The Presentation session, Preview selection, and navigation context remain active so the operator can continue preparing the next Cue.
- The Program monitor shows an unmistakable disconnected placeholder rather than claiming that the last Program frame is still visible to the congregation.
- Reconnecting the same display or attaching another display never resumes visible output automatically.
- The operator explicitly enables the congregation display after reconnection; external Program starts black.
- Pressing `Space` then restores the currently selected Preview to Program, following the established Blackout recovery workflow.

### 50. Congregation-display selection

- Ordo remembers the congregation-display choice per installation, but remembered operating-system display identity is a suggestion rather than unquestioned authority.
- On Presenter Mode entry, Ordo preselects a confidently recognized saved display.
- If there is exactly one display other than the operator display, Ordo suggests that display when no saved match is available.
- When several candidates exist or recognition is ambiguous, the operator must select the congregation display before enabling external output.
- Display selection shows useful recognition evidence including display name, resolution, aspect ratio, and relative position.
- Provide an **Identify displays** action that temporarily renders a large identifier on each connected display.
- Selecting or recognizing a display never bypasses the rule that congregation Program begins black.

### 51. Automatic Presenter preflight

- Opening Presenter Mode automatically runs a concise preflight; the operator does not manually approve or tick off a checklist.
- Preflight verifies Presentation snapshot integrity and required fonts/assets, reports readiness issues captured with the snapshot, checks congregation-display connection and selection, checks resolution/aspect ratio and Output Fit, verifies that the external Program window can be created and rendered, and identifies recoverable interrupted-session state.
- Passed checks collapse into a compact summary. Warnings and failures remain visible with direct corrective actions such as **Select display**, **Show calibration pattern**, or **Review unavailable Cues**.
- Only inability to open a coherent Presentation snapshot blocks Presenter Mode, consistent with computed Mass-plan readiness.
- Display absence or failure leaves Presenter Mode in the established **Congregation output disabled** state rather than blocking it.

### 52. Validated emergency PowerPoint export

- Emergency PowerPoint export operates on an immutable Presentation snapshot and uses the same canonical renderer as Presenter Mode.
- Ordo generates the `.pptx` in a temporary location, then reopens and validates the completed deck before reporting export success.
- Validation checks slide count and order, canonical `1920x1080` full-frame images, speaker-note mapping for operator-only Cues, embedded-image integrity, and expected intentional black slides.
- If any projectable Cue cannot render or the completed deck fails validation, Ordo does not expose the new file as a successful export and does not overwrite a previous valid export. It identifies the affected Cues for correction.
- Renderable warnings may still be exported after acknowledgment, following the existing Mass-plan readiness policy.
- The deck contains actual PowerPoint sections in Mass order, with one section for each top-level Mass item. Its generated congregation slides and operator-only black slides remain grouped within their parent Mass-item section.

### 53. Emergency-deck safety slides

- Every emergency PowerPoint deck begins with an intentional full-black lead-in slide before the first Mass item and ends with an intentional full-black closing slide after the final Mass item.
- The two slides carry private speaker notes identifying their safety purpose.
- Each safety slide occupies a small clearly named **Safety** section, while the intervening liturgical sections remain grouped and named by their parent Mass items.
- This prevents opening or finishing the fallback slide show on congregation-facing content.

### 54. Presenter startup and informal rehearsal

- Do not create a separate Rehearsal Mode, rehearsal status, or rehearsal workflow in the MVP.
- Presenter Mode always opens the operator workspace before creating any congregation-output window.
- Congregation output remains disabled until the operator explicitly chooses **Enable congregation output**, even when a second display is already connected.
- An operator rehearses by using the ordinary Presenter Mode controls without enabling congregation output. This exercises the live presentation path without exposing any content externally.
- Enabling congregation output opens Program on the selected display in black; pressing `Space` then takes the selected Preview live.
- When no second display exists, Presenter Mode behaves the same way except that enabling congregation output is unavailable.

### 55. Near-16:9 display tolerance

- Treat physical display aspect ratios within `0.1%` of canonical `16:9` as exact `16:9` outputs.
- Standard modes such as `1366x768` therefore fill the congregation display without a misleading aspect-ratio warning or a one-pixel black seam.
- The already composed canonical frame is scaled as one unit; output scaling never causes text reflow or layout recomposition.
- Ratios outside the tolerance retain the established proportional fitting, black letterboxing or pillarboxing, and operator warning.

### 56. Passive congregation window and cursor recovery

- The congregation window is a passive borderless output surface that stays above ordinary windows on its assigned display without taking keyboard focus from the operator workspace.
- It accepts no congregation-side controls or mouse interactions and presents no title bar, notifications, diagnostics, or error dialogs; failures remain on the operator screen and external output fails to black.
- The system pointer is hidden only while it is over the congregation window. It is neither hidden system-wide nor trapped, and it becomes visible normally after returning to the operator display.
- While the pointer is over the congregation display, the operator workspace immediately shows a prominent operator-only cursor locator.
- The locator derives return guidance from the operating system's actual display topology rather than assuming the projector is on the right. It may direct left, right, up, down, or diagonally as appropriate.
- The locator includes a compact display-layout diagram identifying the operator display, congregation display, current pointer location, and shortest direction back to the operator display; it disappears immediately when the pointer returns.

### 57. Physical church-system release qualification

> **Superseded in part by Decision 255.** The physical qualification body below
> is reconciled to the `120x80` minimum safe area. The remaining requirements
> stay adopted.

- The MVP is not release-qualified until one complete dress test passes on the actual Windows 10 church PC and congregation projector.
- Qualification imports a representative Celebration Package while offline and cold-starts Presenter Mode.
- Run a full Mass-length Presentation session with representative slide count and content density.
- Verify the actual projector resolution, canonical-frame scaling, readability from the congregation area, the `120x80` minimum safe area, and any required Output Fit.
- Exercise rapid Cue navigation, Freeze, Blackout, emergency editing, congregation-display disconnect/reconnect, and simulated application interruption and recovery.
- Generate, validate, open, and run the emergency `.pptx` using the church computer's installed PowerPoint version.
- Confirm the adopted CPU and memory budgets and verify that Program never exposes operator-only material, application chrome, a cursor, or an error window.
- This is a release acceptance test performed on the deployment system, not an in-application rehearsal-management feature.

### 58. React UI foundation and ownership

> **Superseded by Decision 252.** The visual and interaction ownership
> principles remain, but the shadcn/Base UI/React implementation does not.

- Adopt shadcn/ui as a source-owned primitive foundation, using its Base UI-based React implementations rather than treating shadcn as Ordo's visual identity.
- Commit selected generated primitive source into the Ordo repository and customize it deliberately. Do not import stock dashboard blocks or allow future regeneration to overwrite reviewed Ordo changes blindly.
- Use Tailwind CSS for compiled application styling, with Ordo-owned semantic tokens as the canonical source of color, typography, spacing, density, geometry, focus, and motion decisions.
- Base UI supplies low-level accessible behavior for appropriate controls such as dialogs, menus, tooltips, popovers, comboboxes, collapsibles, and form inputs. All assets, CSS, icons, and fonts are bundled locally; the installed application has no runtime CDN or registry dependency.
- Visible operator workflows remain Ordo-owned deep Modules. In particular, Presenter Workspace, Ordo Rail, Cue Timeline, Output Dock, Mass Planning Workspace, Song Library Workspace, Arrangement Editor, Theme Editor, Display Setup, and emergency-change workflows expose domain-shaped Interfaces rather than raw shadcn or Base UI properties.
- Canonical slide rendering, virtualized canonical thumbnails, pagination visualization, arrangement sequencing, display topology, cursor recovery, calibration, and Theme composition remain custom rendering or interaction surfaces. Wrapping these surfaces in generic cards would add no Depth.
- Avoid the stock visual grammar associated with generic generated dashboards: excessive rounded cards, nested panels, decorative gradients, ambient glows, oversized pills, broad gray-on-gray surfaces, and arbitrary product accents.
- Base UI's keyboard, focus, portal, overlay, and nested-control behavior must be qualified in Milestone 0. Radix remains a possible replacement Adapter beneath the source-owned primitive layer if Base UI fails those packaged Windows/Linux interaction tests; this must not change Ordo-owned Module Interfaces.
- Do not add shallow Ordo wrappers around every primitive. Create an Ordo Module only when it hides meaningful domain or interaction complexity and therefore earns Leverage and Locality.

### 59. Context-sensitive application appearance

> **Superseded in part by Decision 256.** Preparation now offers System
> (default), Light, and Dark instead of deferring manual selection. Presenter
> remains forced graphite-dark and operator appearance stays independent from
> congregation Themes.

- The MVP uses two deliberate appearance polarities selected by workspace rather than one appearance across the entire application.
- Preparation workspaces use a warm, paper-neutral light appearance suited to prolonged library management, planning, arrangement editing, pagination correction, and Theme preparation.
- Presenter Mode uses a graphite-dark appearance suited to a dim church and designed to keep attention on canonical Preview and Program frames.
- Both polarities share one visual system: the same operator typography, geometry, spacing rhythm, control hierarchy, icon rules, focus treatment, and operational state semantics. They are working environments within one product, not unrelated Themes.
- Liturgical colors belong to congregation Themes and their canonical previews. They do not tint the global operator shell, navigation, or ordinary controls.
- Entering or leaving Presenter Mode selects the corresponding appearance automatically. Manual application-appearance selection is deferred beyond the MVP.
- This application appearance decision is independent of congregation slide Themes; changing one never mutates or visually reinterprets the other.

### 60. Unified Atkinson operator typography

- `Atkinson Hyperlegible Next` is Ordo's canonical operator-interface typeface as well as its canonical congregation-projection typeface.
- Operator chrome uses the locally bundled Regular, Medium, and SemiBold weights according to hierarchy and density. ExtraBold remains primarily a projection weight for lyrics and congregation responses rather than the default operator-interface weight.
- `Noto Sans` remains the bundled missing-glyph fallback for both operator and projection text. Ordo does not depend on a Windows- or Linux-installed system font, so the application retains its intended typography across supported platforms and offline transfers.
- Dense operator layouts must be visually qualified at both `1366x768` and `1920x1080`, because Atkinson's wider letterforms can consume more horizontal space than compact UI faces such as Source Sans 3 or Inter.
- Truncation, wrapping, and tabular-data treatments must be designed explicitly; the UI must not compensate for width pressure by silently shrinking ordinary interface text below its adopted legibility target.
- Source Sans 3 and Inter were evaluated as credible compact alternatives but were not adopted. The unified Atkinson family was preferred for legibility, multilingual suitability, and a coherent Ordo identity across preparation, presentation, and congregation output.

### 61. Context-sensitive Presenter primary control

- Presenter Mode does not show a permanent Take button. In Ordo's linked Live state, Cue navigation already updates Preview and Program together, so an always-present Take control would falsely imply a traditional manual transition model.
- When congregation output is disabled, the primary control is **Enable Output**. Enabling remains explicit and follows the established black-first behavior; it never exposes Preview immediately.
- When output is enabled but black, Frozen, or Blacked out, the primary control is **Take Preview Live**. It performs the same transition as `Space`: the selected Preview becomes Program and the session returns to linked Live state.
- While Live, the primary-control area becomes an unmistakable informational state, **LIVE — navigation updates Program**, rather than presenting an inapplicable action.
- Freeze and Blackout remain directly available at all applicable times and retain their keyboard shortcuts. Their availability and state are not hidden behind the context-sensitive primary area.
- Button labels describe the immediate outcome, while Program and Preview state indicators remain separately visible. Color alone is never the only distinction between Live, Frozen, Blackout, or output-disabled states.

### 62. Explicit Ordo-rail reveal

- Presenter Mode never opens the collapsed Ordo rail merely because the pointer hovers at an edge. Reveal requires an intentional click or the rail's keyboard command, preventing accidental overlays during live operation.
- A narrow, clearly labeled **ORDO** tab remains visible while the rail is collapsed so its location and purpose are discoverable without occupying a full column.
- Activating the tab opens the unpinned rail as an overlay over the Cue timeline. This temporary reveal does not resize thumbnails, alter their column count, scroll the timeline, or change Preview, Program, or selection.
- The rail's Pin control converts the overlay into the persistent first column. Unpinning restores the collapsed-tab behavior and returns the reclaimed space to the timeline.
- Clicking outside an unpinned rail or pressing `Esc` closes it and restores keyboard focus to the previously active Cue. Closing the rail never changes presentation state.
- The exact rail keyboard binding remains part of the final shortcut-map review; the interaction contract does not depend on a particular key.

### 63. Minimum Visual Slide Grid thumbnail size

- In supported Presenter layouts, the canonical 16:9 frame inside a Visual Slide Grid item has a minimum width of `300px`, producing a frame approximately `300x169px` on screen.
- The minimum applies to the canonical image area, not to captions or group labels placed outside it. The frame is always a scaled or density-rerendered copy of the canonical slide and never reflows its content to fit the thumbnail.
- Responsive layout removes grid columns before shrinking below the minimum: approximately three columns at `1366x768` and up to four at `1920x1080`, subject to the actual space left by the Ordo rail and Output Dock.
- A narrower resizable window may therefore show two or one grid columns. It must not silently switch the operator to Compact Cue List, change the persisted timeline mode, or alter Preview, Program, selection, or scroll position.
- The `300px` floor keeps representative 72-canvas-unit projected text near 11 screen pixels in the timeline while column 3's larger canonical Preview remains the place for close reading.

### 64. Preparation-shell primary navigation

> **Superseded in part by Decision 257.** Rename the global **Songs** destination
> to **Library**, whose local views are Songs, Responses & Prayers, and General
> Presentations. The remaining shell-navigation rules below remain in force.

- Preparation uses a persistent vertical application-navigation rail rather than opening on a generic dashboard of summary cards.
- **Masses** is the default workspace. **Upcoming**, **History**, and **Archive** are views within Masses rather than separate global destinations.
- **Songs** is the entry point for Song works, Arrangements, Choir repertoire, and Song import. These related library workflows remain locally navigable within the workspace.
- **Themes**, **Transfer & Backup**, and **Settings** are the other top-level preparation destinations. Transfer & Backup owns Celebration Package and Parish Backup operations; Settings owns parish, application, and display configuration.
- Presenter Mode is launched in the context of a selected Mass plan. It is not a disconnected top-level preparation destination with an ambiguous active Mass.
- At comfortable widths the rail shows icons and persistent text labels. At constrained preparation widths it may collapse to icons with accessible names and tooltips while keeping the active workspace unambiguous through structure and shape as well as color.
- The warm paper-neutral shell uses stable split-pane workspaces rather than nested generic cards. Exact pane geometry remains responsive and workspace-specific; changing navigation density never changes the selected domain object or discards editor state.

### 65. Presenter command-chrome ownership

- Presenter Mode divides permanent chrome into a slim global header and the right-hand Output Dock. Live-output commands are not scattered across the header or Cue timeline.
- The global header owns session-wide context only: the active Mass title and date, clock, congregation-display health, timeline-mode access, shortcut help, and Exit Presenter.
- The Output Dock owns everything that describes or changes congregation output. Its stable vertical order is **Program**, **Preview**, output/display status, the directly available **Freeze** and **Blackout** controls, and the previously adopted context-sensitive primary-control area.
- Keeping high-stakes output controls immediately beside Program and Preview makes their effect spatially explicit and preserves the Cue timeline as a navigation surface.
- Responsive density may shorten secondary labels or move low-risk header actions into a compact menu, but it must not relocate or hide Program, Preview, output state, Freeze, Blackout, or the context-sensitive primary control during Presenter Mode.

### 66. Ordo-rail keyboard access and focus return

- `O` is the Presenter shortcut for **Open/focus Ordo**. It is an access command rather than a pin-state toggle.
- When the Ordo rail is collapsed, `O` opens its unpinned overlay and moves focus to the Mass item containing the active Cue. When the rail is already pinned, `O` moves focus into the visible rail without changing its geometry.
- `Esc` closes an unpinned Ordo overlay and returns focus to the active Cue. If the rail is pinned, `Esc` returns Cue focus without collapsing or unpinning the rail.
- Pin and Unpin remain explicit visible actions. Repeatedly pressing `O` cannot unexpectedly resize the Cue timeline.
- The collapsed rail's labeled **ORDO** tab, the visible shortcut reference, and the rail footer expose the `O` command; discoverability does not depend on memorization.
- Opening, focusing, closing, pinning, or unpinning the rail never changes Cue selection, Preview, Program, Freeze, Blackout, output availability, or presentation state.

### 67. Presenter shortcut suspension and focus restoration

- Passive Presenter surfaces such as the Ordo rail and shortcut-reference panel do not suspend the live `F`, `B`, or `Space` commands. Merely inspecting navigation or help must not make these essential controls unavailable.
- While a text field, emergency editor, or modal dialog owns focus, every single-key Presenter command is suspended, including Cue navigation, `F`, `B`, `O`, and `Space`. Typed content can therefore never trigger a presentation transition.
- `Esc` is handled by the topmost editor, dialog, or overlay first. It closes or begins that surface's cancel flow without altering Program.
- Emergency Insert and Emergency Edit Freeze Program before their editing surface accepts input, so shortcut suspension cannot cause the current output to advance inadvertently.
- Saving, cancelling, or closing the surface restores focus to the previously active Cue unless a deliberate action selected a different Cue.
- The shortcut-reference surface explains when live commands are suspended; a visible editing/modal state must not leave the operator guessing why a shortcut is inactive.

### 68. Focused three-pane Mass Planning workspace

- Opening a Mass plan replaces the Mass-list view with a focused planning workspace. A breadcrumb returns to the originating Upcoming, History, or Archive view without making that list a permanent competing pane.
- The persistent application-navigation rail remains outside the planner. Within the planner, a plan header owns Celebration identity, date and context, autosave state, readiness access, Mass Theme access, and the route into Presenter Mode.
- The planner body has three permanent working surfaces: the **Mass outline** for ordered Mass-item titles, the **selected-item editor** for Song/Arrangement, response, Cue, and sequence work, and the **Inspector** for authoritative canonical-slide inspection, selected-scope warnings/dependencies, and item-level Theme override.
- The three panes are structural working areas rather than independent card containers. Resizing or compact navigation may change pane density but never changes the selected Mass item, discards staged editor work, or changes canonical slide composition.
- Song and Arrangement selection is temporary task UI and must not become a permanent fourth pane.

### 69. Song/Arrangement picker slide-over

- Choosing or replacing a Song/Arrangement opens a floating slide-over from the right, visually above the underlying Mass planner. The underlying planner retains its place and selection.
- Search results and Arrangement details may scroll beneath a sticky slide-over header. Choosing an Arrangement stages it; it does not mutate the Mass item until the operator explicitly commits.
- The sticky header's top-right primary action identifies the staged result, for example **Use Choir A arrangement**. At constrained widths its visible label may shorten to **Use arrangement** while its accessible name and adjacent selected-Arrangement context remain explicit.
- A floating `›` control is attached to the slide-over's left boundary and closes the panel in the direction it retreats. It has the accessible name and tooltip **Close Song picker**; `Esc` performs the same dismissal.
- Closing without committing discards only the temporary picker selection. It does not change the existing Mass item, outline position, inspector state, or canonical slides.
- The commit action is not duplicated at the bottom of the desktop slide-over, avoiding competing primary actions and keeping it continuously reachable above long results or details.

### 70. Mass-item insertion control placement

- **Add Mass item** is a local Mass-outline action and appears at the top-right of the Mass outline header rather than at the bottom of the ordered list.
- At comfortable widths it uses the explicit label **+ Add Mass item**. At constrained widths it may compact to a `+` icon button with the same accessible name and a tooltip.
- The insertion workflow and default placement are defined in Decision 71. Decision 72 deliberately omits an outline-wide overflow menu; the complete per-item context menu remains in the immediate interview queue.

### 71. Add Mass item workflow and editor presets

> **Superseded in part by Decision 257.** **General Presentation** replaces the
> separate Custom text and Announcement presets while retaining optional
> Mass-local private delivery context. The remaining insertion behavior and
> specialized presets below stay adopted.

- **+ Add Mass item** opens a compact anchored dropdown from the Mass-outline header. Opening or dismissing it does not mutate the Mass plan.
- The dropdown states its insertion destination. Choosing a type immediately creates, selects, and autosaves a Mass-local item; ordinary Undo can reverse an accidental insertion without an extra confirmation step.
- By default, the new item is inserted immediately after the currently selected Mass item within that item's section. If a section header is the active insertion target, append within that section. If neither an item nor a section is active, require the operator to choose a destination section instead of silently appending to the end of the Ordo.
- Always state the complete insertion destination in the dropdown, such as **After Responsorial Psalm · Liturgy of the Word**, and allow it to be changed before choosing the item type. Creating the item selects it and expands its destination section.
- Per-item context actions will expose explicit **Insert before** and **Insert after** commands that open the same type chooser; their remaining wording and organization are still unresolved.
- The chooser groups **Song**, **Reading**, linked **Response/Prayer**, **General Presentation**, **Hold**, **Blackout**, and **Custom sequence**. Song proceeds into the adopted staged Song/Arrangement picker; the other choices open their corresponding selected-item editor.
- Reading keeps Scripture reference and reading text as operator context by default while separately encoding the closing prompt and congregation response; full-reading projection remains an explicit per-item option.
- Response sequence encodes ordered role-tagged Cues such as operator-only `P` prompts and projectable `T` responses.
- General Presentation supports reusable or Mass-local constrained Text/Image Slides and may pair them with Mass-local private delivery context that never enters Program or the reusable source.
- Hold creates an ordinary operator-only black Cue and does not latch the Blackout state. When Live its black output follows the normal linked Program/Preview rule; Freeze is used when the previous Program frame must remain visible.
- Blackout creates an explicit Cue that enters the persistent Blackout state; Program stays black while Preview may navigate until Space takes the selected Preview live.
- The flexible escape hatch is named **Custom sequence**, not **Blank item**. It starts as an incomplete titled Mass item and lets the planner assemble an unusual mixture of response, text-slide, operator-trigger, and Blackout Cues.

### 72. Mass-outline header action ownership

- The Mass-outline header contains its title and the local **+ Add Mass item** action only. It has no outline-wide `...` overflow menu in the MVP.
- Mass-plan-wide actions belong to the plan header, while actions on an existing Mass item belong to that item's visible or contextual controls.
- Weak speculative commands such as **Expand all** do not justify a miscellaneous menu. An outline-wide overflow may be introduced later only when several coherent, genuinely outline-scoped operations exist.

### 73. Mass-item selection scope

- The Mass outline supports one selected Mass item at a time in the MVP. The selected-item editor and Inspector both follow that single selection.
- Multi-selection and bulk Mass-item actions are deferred. Their absence avoids ambiguous bulk editing, deletion, and movement behavior without blocking the ordinary preparation workflow.

### 74. Celebration and scheduled Mass-plan relationship

- A Celebration is language-neutral liturgical identity and calendar context. Do not create separate English, Cebuano, or Latin Celebrations for the same occasion.
- One Celebration may own several independent Mass plans. Plans may differ by schedule, primary language, choir, selected Arrangements, translated liturgical content, Presider, location, or other local preparation.
- Several Mass plans under the same Celebration may use the same primary language; for example, different schedules may have different choirs and Song Arrangements while sharing substantially similar liturgical content.
- **Duplicate for another Mass** creates an independent Mass plan under the same Celebration and then collects or changes its schedule, language, and choir. It has no continuing synchronization with its source plan in the MVP.
- **Reuse for another Celebration** remains the distinct workflow for creating a plan under a different dated liturgical occasion.

### 75. Conditional template items

- A parish Mass template may contain conditional Mass items whose default inclusion is evaluated from Celebration metadata such as season and explicit Celebration tags.
- Evaluation is automatic-first but advisory rather than an unreviewable universal authority. Included and omitted results remain visible in the preparation outline with their reason; omitted items do not enter Presenter Mode.
- The operator may explicitly include an item omitted by a template rule or omit an included item for the current Mass plan. A plan-level override does not silently rewrite the reusable template.
- When required metadata is missing, rules conflict, or Ordo cannot determine the applicable rubric, the item remains visibly unresolved and contributes a readiness warning instead of being silently included or removed.
- These rules are parish-owned template knowledge. Ordo may ship or import reviewed starting material later, but the core application does not hardcode every liturgical exception as unquestionable behavior.

### 76. Typed template slots and multiple templates per language

- A Mass template may mix concrete reusable content with typed unresolved slots. Initial slot types include **Song slot** and **Reading slot**; a slot preserves the expected liturgical role and placement without pretending its plan-specific content has already been chosen.
- Song slots may be required, optional, or conditional. When instantiated for a Mass plan, their chooser is filtered by the assigned choir and relevant applicability metadata, but Ordo does not silently select or replace a Song merely because it is in that choir's repertoire.
- Reading slots represent roles such as First Reading, Second Reading, and Gospel rather than assuming one fixed count for every Celebration. Operators may insert additional Reading slots into an individual plan, while a dedicated template may encode a recurring exceptional structure when warranted.
- A language may have any number of parish templates. Templates are distinguished by their own name, purpose, and advisory applicability—not by language alone—and the operator may deliberately choose among them.
- This flexible template model must not be mistaken for qualified MVP support for every sacrament or special rite. Funeral, baptism, wedding, Easter Vigil, and other dedicated templates can be added by the parish or developed later without changing the basic Mass-item and slot model.

### 77. Template-library ownership and organization

- Reusable Mass templates are preparation content managed under **Masses → Templates**, not application settings.
- Language is required template metadata and a filter, not a uniqueness boundary or the sole hierarchy. A parish may keep several independently named templates in the same language for different recurring Celebration shapes.
- Template applicability metadata recommends and ranks likely templates but does not prohibit the operator from choosing another template.
- Opening a template reuses the planner's three-pane mental model: ordered template outline, selected item or slot editor, and Inspector. The template library also exposes creation, duplication, language filtering, applicability, modification, and usage context.

### 78. Template instantiation and revision boundary

- Creating a Mass plan from a template materializes an independent copy of the selected template revision into that plan.
- Later edits to the reusable template affect future plan creation only. They never silently add, remove, reorder, or rewrite items in an existing Mass plan.
- A Mass plan may retain the originating template and revision as provenance, but that reference does not imply synchronization.
- Merging or reapplying later template changes into an already edited Mass plan is deferred beyond the MVP because its conflict behavior would require a distinct reviewed workflow.

### 79. Template recommendation and fallback selection

- Creating a Mass plan uses its language and Celebration applicability metadata to rank compatible Mass templates. The best match is preselected but remains visibly identified and changeable before creation; the operator is never locked to the recommendation.
- A parish may designate one general fallback template per language. A more specifically applicable template outranks that fallback, while deliberate manual selection remains allowed.
- If neither an applicable template nor a language fallback exists, Ordo creates an empty Mass plan with a visible readiness warning rather than silently choosing an unrelated template.
- Choir assignment does not select the Mass template. The template defines the Mass structure; the chosen choir's Arrangements subsequently fill or resolve Song slots.

### 80. Independent language-specific liturgical content

- Translated responses and readings are independently authored language-specific content rather than synchronized translations of one canonical text.
- Entries in different languages may share a neutral liturgical role, such as Greeting response or First Reading, without Ordo asserting that their wording is equivalent.
- A language-specific Mass template supplies its recurring responses, while a Reading slot requires content matching the Mass plan's language.
- Ordo does not machine-translate liturgical text or propagate edits between languages. Each version can therefore preserve approved wording and parish-local usage independently.

### 81. Language-aware Duplicate for another Mass

- Duplicating a Mass plan into another schedule with the same language creates a complete independent copy.
- When the target language differs, Ordo creates a target-language derivation using the best matching template for that language instead of copying source-language responses or pretending to translate them.
- Shared Celebration metadata and the newly entered schedule details are retained. Song slots are matched by liturgical role, and an existing Arrangement is retained only when its language metadata is compatible with the target Mass plan.
- Recurring responses come from the target-language template. Target-language readings and other language-bound custom text remain unresolved for deliberate completion.
- Before creation, Ordo shows a summary of what will be retained and what will remain unresolved.

### 82. Mass-details editor and metadata ownership

- Clicking the Mass title/date or its edit affordance opens a floating **Mass details** slide-over above the Inspector, preserving the planner and current Mass-item selection underneath.
- The panel visibly separates **Celebration** fields shared by all scheduled Mass plans—date, liturgical title, season, liturgical color, and applicability tags—from **This Mass** fields—time, primary language, choir, Presider, location, and private preparation notes.
- Both sections autosave and participate in Undo. Plan-specific changes apply immediately.
- When sibling Mass plans exist, the Celebration section displays how many Masses share it. Changing a shared field requires explicit confirmation because the edit affects every plan under that Celebration.

### 83. Celebration-grouped Masses index

- **Masses → Upcoming** is a chronological grouped list in which each Celebration is the parent and its scheduled Mass plans are subordinate rows. Even a Celebration with one Mass retains this grouping.
- A Celebration header shows its date, title, season or occasion context, liturgical color, Mass count, group-level actions, and a roll-up of plan readiness without replacing each plan's own readiness state.
- Each Mass-plan row shows its time, language, choir, readiness, and plan-scoped actions. Clicking the row opens its Mass Planner.
- A Celebration can add another sibling Mass plan, while **Reuse for another Celebration** remains a Celebration-level flow that creates a new Celebration and an independent plan from an earlier source plan.
- Upcoming groups are expanded by default. Older Celebration groups may be collapsed in History, and search or filters may narrow scheduled plans by language, choir, or readiness.
- Creation is explicitly Celebration-first: create the Celebration, then either create a Mass plan for it or reuse an earlier Mass plan for that new Celebration.

### 84. Optional two-stage Celebration and first-plan creation

- The Masses-page primary action is **+ New Celebration**, reflecting that a Celebration exists before its scheduled Mass plans.
- After saving Celebration details, Ordo immediately shows the new Upcoming group and offers two first-plan paths: **Create from template** or **Reuse an earlier Mass plan**.
- Creating from a template collects schedule, language, and choir and exposes the recommended template before creation. Reusing an earlier plan collects the new schedule context and previews which content will be copied or remain unresolved.
- The operator may choose **Do this later**, leaving a valid Celebration with no scheduled Mass plans and clear inline actions to create or reuse one later.
- Once at least one plan exists, **+ Add another Mass** exposes the same two starting methods. A successfully created or reused plan opens directly in the Mass Planner.

### 85. Celebration date, scheduled start, and anticipated Masses

- A Celebration owns its liturgical date, while every Mass plan owns its actual scheduled start date and time. These dates may differ.
- An anticipated Mass held on the preceding civil day remains a plan beneath the later Celebration and therefore uses that Celebration's prepared liturgical context and readings.
- A new Mass plan defaults its scheduled date to the Celebration date. Selecting the preceding day suggests **Anticipated**, but the operator explicitly confirms the timing; Ordo does not decide liturgical correctness from a hardcoded time threshold.
- The Masses index shows the actual scheduled date and time on every plan row and orders upcoming Celebration groups by their earliest scheduled Mass, while retaining the Celebration's liturgical date in its header. A Celebration without plans uses the local start of its liturgical date as its fallback ordering key.
- **Anticipated** is not a generic **Vigil** mode. Celebrations such as the Easter Vigil may require their own structure, readings, and template and must not be reduced to this scheduling flag.

### 86. Metadata validation boundary

- Creating a Celebration requires only its liturgical date and display title.
- Creating a Mass plan requires only its actual scheduled start and primary language. A parish may therefore defer Choir, Presider, Location, notes, and additional liturgical metadata without losing the draft.
- Season, color, and applicability tags are optional, but a conditional template rule that depends on missing metadata remains explicitly unresolved and contributes a readiness warning.
- Missing choir assignment leaves choir-dependent Song slots unresolved. Duplicate schedules warn without blocking deliberate creation.
- A scheduled date different from the Celebration date requires an explicit timing relationship such as **Anticipated**. Inconsistent relationships, such as an anticipated plan scheduled after its Celebration, are validation errors rather than readiness warnings.

### 87. Simplified Celebration metadata and Choir management

- The MVP deliberately omits liturgical grade or rank from Celebration metadata. The shared Celebration fields remain date, title, controlled season, liturgical color, and flexible parish applicability tags; specialized structures are selected through templates and tags rather than a detailed precedence engine.
- **Songs → Choirs** manages lightweight Choir records with a required name and optional short label, preferred languages, private notes, and Active or Archived state. Member rosters, attendance, rehearsals, and contacts remain out of scope.
- Each Choir exposes its simple Song repertoire and the applicable Arrangements beneath each Song. Assigning a Choir in a Mass plan filters the picker toward that repertoire without silently choosing content.
- One exact Arrangement may be associated with multiple Choirs. If Choir A and Choir B sing the same Kyrie wording, sequence, and pagination, both Choirs point to the same Song work and Arrangement instead of duplicating either.
- Archiving a Choir removes it from ordinary new-plan selection while preserving its historical Mass-plan and Arrangement references.

### 88. Musical-composition identity and translated Arrangements

- A Song work represents one underlying musical composition or setting. Two composers' distinct settings remain separate Song works even when they set substantially the same fixed liturgical text.
- A choir-specific wording, sequence, phrasing, or performance variation of the same underlying composition is a separate Arrangement beneath that Song work, not a second Song work.
- A translation that retains the underlying musical composition is a language-specific Arrangement. Two different translations are two independent Arrangements, and an intentionally bilingual rendition is another Arrangement with its own language set and sequence.
- Composer and original text authorship belong to the Song work. Translator, adapter, and arrangement-specific attribution belong to the Arrangement.
- One exact Arrangement may be shared by multiple Choirs. Its lyrics, sections, sequence, and pagination are stored once; an Arrangement with meaningful textual or structural differences remains independent and does not share line-level editing with another.
- If authorship or musical identity is uncertain, Ordo records unverified attribution and keeps likely-different settings separate for later duplicate review rather than merging from title or lyric similarity.

### 89. Lightweight Presider, Location, and notes input

- **Presider** and **Location** are editable comboboxes in Mass details: they suggest parish defaults and previously used values while allowing the operator to enter a new value immediately. A visiting priest or diocesan bishop can therefore be assigned directly to one Mass without first becoming a managed parish record.
- The MVP does not add a clergy directory or location-management workspace. **Parish Settings → Defaults** owns only a lightweight list of **Regular Presiders**, an optional **Default Presider**, and an optional **Default Mass location**. Private preparation notes remain unrestricted multiline text owned by the individual Mass plan.
- A newly created Mass plan copies the optional Default Presider and Default Mass location as its initial values, and the operator may immediately replace either. Regular Presiders appear first in the editable Presider combobox; other previously used names remain available as recent suggestions without silently becoming Regular Presiders.
- The parish default is not a live reference. Changing it affects only Mass plans created afterward and never rewrites historical or already-prepared plans.
- Duplicating a plan under the same Celebration or reusing it for another Celebration retains the source plan's saved Presider and Location instead of replacing either with the current parish defaults.

### 90. Shared Celebration edits and derived review

- After explicit confirmation, editing shared Celebration metadata updates that Celebration context for every sibling Mass plan beneath it.
- A shared metadata edit never silently adds, removes, reorders, or rewrites Mass items already materialized into an existing plan. Ordo instead recomputes readiness and surfaces review findings when conditional items or unresolved dependencies may no longer fit the changed context.
- Changing a Celebration's liturgical date does not silently move the actual scheduled start of its Mass plans. Any now-inconsistent timing relationship is surfaced according to the adopted warning and validation rules.
- Existing Presentation snapshots remain immutable. When a Celebration change affects presentation inputs, entering Presenter Mode after that edit creates a new snapshot; the earlier snapshot remains available as historical presentation state.
- A plan previously reused into another Celebration remains unaffected because reuse creates an independent plan governed by its own target Celebration.

### 91. Mass-item reordering and focus-scoped shortcuts

- Every Mass item is freely movable. A template provides a suitable starting order, but Ordo does not lock operators into that order or pretend to enforce complete liturgical ordering rules.
- Pointer reordering begins only from a dedicated drag handle; clicking the remainder of the outline row continues to select the item. Dragging shows a precise insertion line, supports scrolling through the outline, and leaves the moved item selected and visible.
- With focus on a Mass-item row or its handle inside the **Mass Outline**, `Alt+Up` and `Alt+Down` move that single item by one position. Focus follows the moved row so repeated presses remain predictable.
- The shortcut is inactive while focus is in a text field, rich-text editor, dropdown, dialog, slide-over, middle editor, Inspector, or Presenter Mode. It is not a global command acting merely on whichever item happens to be selected.
- Other reorderable editors may later use the same gesture only when focus is inside their own list. The focused list therefore determines the entity being moved.
- Reordering autosaves, participates in Undo, and never moves multiple items at once in the MVP.

### 92. Mass-item context menu and removal semantics

- Right-clicking a Mass-item row and activating that row's hover-or-focus `...` affordance open the same scoped menu. The two entry points never expose different capabilities.
- The MVP menu contains **Add item above**, **Add item below**, **Duplicate item**, **Move up**, **Move down**, and a separated destructive-looking **Remove from this Mass** action. Boundary-inapplicable movement actions are disabled.
- **Remove from this Mass** removes only that plan's placement and plan-local content. It never deletes a referenced Song work, Arrangement, template, reading-library entry, or other reusable source content.
- Ordinary removal applies immediately without a confirmation dialog, autosaves, and presents a clearly named Undo action. Existing Presentation snapshots remain unchanged.
- The interface reserves **Delete** for actions that actually delete an owned record rather than using it for removal of a Mass-plan placement.

### 93. Explicit Inspector controls instead of a generic overflow

- The Inspector has no generic `...` menu in the MVP. Mixing Mass-item operations, Cue operations, view preferences, and navigation in one unlabeled overflow would make action ownership ambiguous.
- High-value Inspector actions remain visible and scoped: previous/next canonical slide with position, the inspected slide's warning state, item-level Theme override, and a contextual **Fix pagination** route to the owning editor.
- Mass-item actions remain in the Mass Outline row menu, while content editing remains in the selected-item editor. The Inspector does not duplicate either surface.
- If a later release gains several coherent slide-specific commands, it may introduce an accessibly named **Slide actions** menu. View preferences such as safe-area guides should likewise use an identifiable view control rather than a miscellaneous overflow.

### 94. Mass-plan Readiness panel and navigation

- Activating the plan header's readiness indicator opens a floating **Readiness** slide-over from the right above the Inspector, preserving the Mass Outline and selected-item editor underneath.
- The trigger states active counts in words, such as **1 incomplete · 2 warnings**, rather than hiding severity behind a generic icon or inaccurately calling ordinary missing content a blocker.
- The panel begins with computed readiness and counts, then groups findings under **Incomplete presentation** and **Review recommended**. Plan-level findings precede item-level findings within a group; item findings follow Mass-outline order and identify their owning item.
- Each finding states the concrete consequence and offers both a navigable finding title and a specific corrective action. Activating the title selects and scrolls to the relevant Mass item while preserving the panel; activating the corrective action closes the panel and moves focus to the relevant editor control.
- Findings resolve automatically from the current plan state and disappear when their cause is corrected. The MVP has no manual **Mark resolved** state.
- **Incomplete** means some intended output cannot be faithfully produced, but it is not itself a Presenter-entry blocker. **Present** remains available and requires a prominent acknowledgment; Unavailable Cues remain visible and inspectable but cannot be taken live.
- Only a fatal inability to capture or open a coherent Presentation snapshot blocks Presenter Mode. This exceptional condition must be described as inability to present, not combined with ordinary incomplete-content counts.

### 95. Mass Theme chooser and nested Theme Overrides review

- Activating **Theme · Parish Dark** in the Mass-plan header opens a floating right **Mass Theme** slide-over that spans the complete application window, including its header. It is an app-level planning task rather than an Inspector-confined panel.
- The chooser previews each parish Theme through the selected canonical `16:9` slide so typography, pagination, background, contrast, and other presentation consequences can be compared on real Mass content.
- Choosing a Theme is staged. The canonical preview updates immediately, while the Mass plan remains unchanged until the sticky header action commits the selection. Closing the panel or pressing `Esc` discards the staged choice.
- The chooser identifies the currently applied Theme and explicitly discloses Mass items that keep item-level Theme overrides. Applying a different Mass Theme never silently replaces those overrides.
- **Review** opens a child **Theme Overrides** slide-over above the Mass Theme chooser. The parent recedes slightly but remains spatially visible; Back returns to it without losing the staged Theme or override decisions.
- Override review follows Mass-outline order and names each owning item. Every row shows the current item override and the Theme it would inherit after the staged change as an explicit `current override → inherited result` comparison, even when both happen to render similarly.
- **Use Mass Theme** stages removal of one item's override; the all-items action stages the same change for every overridden item. These actions remain reversible within the review and do not partially mutate the plan.
- The parent commit action summarizes and applies the staged Mass Theme selection and staged override removals as one coherent change set. Autosave and Undo follow the ordinary Mass-plan editing rules; existing Presentation snapshots remain immutable.
- **Edit selected theme** routes to the full Theme Editor for the highlighted Theme without implicitly applying that Theme to the Mass plan.

### 96. Theme Editor navigation, specimens, and core property groups

- The full Theme Editor is a dedicated preparation workspace rather than another nested slide-over. One dominant canonical `16:9` frame remains authoritative beside the controls.
- From a Mass plan, **Edit selected theme** opens the highlighted reusable Theme without applying it. **Back to Mass Theme** returns to the same plan and restores the chooser with its staged Theme and Theme-override decisions intact.
- Opening the same editor from top-level **Themes** has no Mass-plan return context and no staged Mass Theme selection.
- A specimen bar changes only the content used to test the Theme; it does not change the Theme's formatting scope, edit Mass content, or create per-slide styling. Stable specimens cover **Lyrics**, **Response**, **Identification**, and **Reading**. **Current Mass slide** is additionally available only when the editor was opened from a Mass plan; the library entry begins with a stable specimen instead.
- Every specimen is rendered through the same canonical `1920x1080` pipeline used by the Mass planner and Presenter surfaces, allowing the operator to expose wrapping, contrast, density, and placement consequences before using the Theme.
- Theme properties are separated into **Typography**, **Colors**, and **Layout** rather than mixing unrelated controls under Type. **Typography** owns text-role selection, bundled typeface, preferred and minimum size, weight, and line spacing. **Colors** owns the text/background pair, picker and editable hexadecimal input, reusable parish colors, immediate canonical-preview feedback, and computed contrast feedback. **Layout** owns horizontal alignment, vertical placement, safe area, and maximum text width.
- Identification titles, credits, and labels may use their established secondary text role rather than inheriting lyric-body weight blindly. Specimen selection and text-role selection remain distinct: the former chooses test content, while the latter chooses which reusable Theme rule is edited.
- Background-image and other decorative-layer controls must remain within the Theme's visual-appearance responsibility rather than Typography; their exact MVP controls and whether **Colors** is later relabeled **Appearance** remain to be finalized without changing the three-way separation.
- Shared-Theme change semantics are extended by Decision 97; Theme revision/Undo and affected-plan rerender review remain unresolved.

### 97. Staged shared-Theme editing and safe duplication

- Editing a reusable Theme occurs in a staged Theme Editor session. The authoritative preview reflects pending changes immediately, but Mass plans and the saved Theme remain unchanged until the operator explicitly saves.
- A Theme may be edited in place even when several editable Mass plans inherit it. Before saving, Ordo prominently warns that this is a shared Theme and identifies how many Mass plans will be affected.
- Saving replaces the reusable Theme's current editable state and causes every editable Mass plan inheriting it to render from the saved changes. Existing immutable Presentation snapshots remain unchanged.
- **Duplicate Theme** is the safe alternative to shared propagation: it creates a new independent Theme from the staged result without changing the original Theme or plans that inherit the original.
- Closing or navigating away with unsaved staged changes must not silently publish them. Exact discard confirmation, Theme revision history, Undo semantics, and affected-plan review behavior remain to be finalized.

### 98. Revision-based Theme Save and Undo

- Every successful Theme **Save** creates a restorable Edit revision before publishing the staged Theme state.
- Save feedback identifies the Theme and the number of editable Mass plans whose inherited rendering was updated, for example: **Parish Dark saved · 12 Mass plans updated**.
- The save result exposes **Undo**. Undo restores the preceding Theme state by creating a later revision rather than erasing history, then rerenders the same currently inheriting editable plans.
- Existing immutable Presentation snapshots are unaffected by both Save and Undo. A later Presenter session uses a newly captured snapshot only when the editable plan or its inherited Theme result has changed.
- The Theme's longer-term revision-history surface and affected-plan review workflow remain to be designed; they do not weaken the immediate Save-and-Undo guarantee.

### 99. Automatic Theme propagation with exception-based review

- After a Theme Save or Theme Undo, Ordo automatically rerenders all editable Mass plans that currently inherit the Theme. Clean rerenders require no approval step and do not interrupt the editor with a modal workflow.
- Human review is requested only for meaningful deltas introduced by the Theme change: new overflow or contrast warnings, changed pagination or slide count, or a newly incomplete projectable Mass item.
- When such deltas exist, Save feedback offers **Review affected plans** and reports the number of plans needing attention. Plans without meaningful deltas are omitted from this review.
- The affected-plan review groups findings by Mass plan and affected Mass item. Activating a finding opens that plan at the owning item and exact affected content rather than leaving the operator to locate the changed slide manually.
- Review is advisory and exception-based. The Theme revision is already saved and remains recoverable through Undo; immutable Presentation snapshots remain unchanged.

### 100. Theme Appearance and MVP background model

> **Superseded in part by Decision 255.** Themes still own ordinary backgrounds,
> but the generated Celebration-title Cue gains the narrow Mass-local custom
> image override defined there. Other per-slide backgrounds remain deferred.

- Rename the Theme Editor's **Colors** property group to **Appearance** so its label matches its responsibility for both foreground colors and the rendered background.
- An MVP Theme background is either a **Solid color** or one **Image**. It is a property of the reusable Theme, not of an individual specimen or canonical slide.
- Image backgrounds use **Cover** fitting and expose focal-position control so an operator can choose which region survives cropping within the canonical `16:9` frame.
- An image background may be combined with one color overlay and adjustable opacity. This is the Theme's built-in legibility treatment; the canonical preview and computed contrast feedback update from the resulting composition.
- Every image background stores a required fallback color. If the image is missing or corrupt, rendering remains coherent and reports the degraded asset rather than emitting a partial frame.
- Theme background assets are portable dependencies and must be included in Celebration Packages and Parish Backups when referenced by captured content.
- Gradients, multiple background or decorative layers, textures, and per-specimen backgrounds are deferred from the MVP.

### 101. Dirty Theme session and navigation-away behavior

- Any staged Theme change that differs from the loaded revision places the Theme Editor in a visible **Unsaved changes** state.
- **Save** and `Ctrl+S` use the same impact-warning, revision creation, propagation, feedback, and Undo behavior; the shortcut does not bypass shared-Theme safeguards.
- Back, close, or any navigation away from a dirty Theme session opens a compact confirmation with exactly three outcomes: **Save changes**, **Discard changes**, or **Keep editing**.
- **Save changes** completes the ordinary Save flow before navigating. **Discard changes** abandons only the staged editor state and leaves the saved Theme and inheriting plans unchanged. **Keep editing** cancels navigation and returns focus to the Theme Editor.
- Successfully using **Duplicate Theme** publishes the staged result as a new independent Theme and resolves the dirty source-editing session. Returning to the source or Mass Theme chooser must not then ask the operator to discard the same changes.
- A clean Theme Editor session exits without confirmation.

### 102. Contextual Theme duplication and Mass-plan return

- **Duplicate Theme** opens a compact naming dialog prefilled from the source name, such as **Parish Dark copy**. The operator may rename it before creation.
- Successful duplication creates a new independent Theme from the complete staged result and switches the Theme Editor to that new Theme. Subsequent edits and saves affect the duplicate, not the source.
- When duplication began from top-level **Themes**, the new Theme remains open in the ordinary library editing context.
- When duplication began through a Mass plan's Theme chooser, **Back to Mass Theme** returns to the preserved chooser and stages the new Theme as that plan's candidate selection.
- Returning from duplication never applies the new Theme by itself. The operator must still commit the Mass Theme chooser, preserving the chooser's staged Theme and override-removal transaction boundary.
- Other Mass plans that inherit the source Theme remain attached to the source and are unaffected by duplication.

### 103. Theme library management, archival, and protected fallback

- The top-level Theme library exposes **Edit**, **Rename**, **Duplicate**, **Set as parish default**, and **Archive** as the MVP management actions.
- **Archive** is the recoverable removal operation. An archived Theme is hidden from ordinary new Theme selection but remains available to existing Mass plans that inherit it and may later be restored.
- An archived Theme continues rendering existing editable plans and immutable Presentation snapshots coherently; archival never substitutes a different Theme into them.
- A Theme that is currently the parish default cannot be archived until the operator chooses a replacement default.
- Permanent Theme deletion is deferred from the MVP. This avoids breaking inheritance or prematurely defining asset-garbage-collection behavior.
- Ordo ships with a protected black-background/white-text safety Theme. It may be selected or duplicated, but it cannot be edited in place, renamed, or archived, ensuring that a dependency-free fallback always exists.

### 104. Visible Theme revision history and restoration

- The Theme Editor header exposes a labeled **History** action in the MVP; revision recovery is not confined to the transient post-Save Undo notice.
- **History** opens a right slide-over listing saved Theme revisions with date and time, a concise generated change summary, and the number of editable inheriting Mass plans affected when that revision was saved.
- A revision may be previewed through the authoritative canonical frame without changing the current staged editor state or publishing it.
- **Restore this revision** uses the same current-impact warning as an ordinary shared Theme Save. Restoration creates a new latest Edit revision from the historical state and never erases the intervening history.
- Restoring rerenders currently inheriting editable plans and uses the same exception-based affected-plan review. Existing Presentation snapshots remain immutable.

### 105. Song-item editor ownership and deliberate Mass-local customization

- The selected Song-item editor is a Mass-planning surface, not a second permanent Arrangement editor. Its default state displays reusable source content without making it directly editable.
- The editor identifies the Mass item's liturgical role and title, selected Song and Arrangement, language, Choir availability, Arrangement section sequence, and the resulting Cue/slide counts.
- **Change song or arrangement** opens the adopted staged picker. **Open in Song Library** navigates to the reusable Song/Arrangement editing context rather than changing library content inside the Mass plan.
- **Customize for this Mass** is the explicit boundary-crossing action. Only after activation may the operator change lyrics, section sequence, or other Arrangement-derived content inside this plan.
- Customized content is visibly labeled **Mass-local override**, and both the editor and Inspector identify it as belonging only to the current Mass plan. Creating or editing it never silently mutates the reusable Song or Arrangement.
- The Inspector continues to show the authoritative canonical slide, selected section/occurrence, source ownership, warnings, item-level Theme inheritance, and owner-directed **Fix pagination** route while either source state is active.
- The exact copy boundary, repeated-section linking, occurrence-specific editing, and later promotion behavior are finalized in subsequent decisions rather than hidden behind the visual state.

### 106. Mass-local Arrangement copy and repeated-section linking

- Activating **Customize for this Mass** copies the selected Arrangement's current reusable sections, performance sequence, and approved pagination into an independent Mass-local override owned by that Song item.
- Once copied, later edits to the reusable library Arrangement do not change the customized Song item. The Mass-local source remains explicit and may be reviewed for later promotion without weakening its independence.
- Repeated occurrences of one section remain linked inside the Mass-local copy. Editing the shared local **Chorus**, for example, updates every Chorus occurrence in that Song item's sequence by default.
- **Edit this occurrence only** deliberately forks the selected occurrence into an independent local section value while leaving the other repeated occurrences linked to their existing local section.
- Reordering or removing one section occurrence changes only the performance sequence. It does not modify or delete the underlying local section text used by other occurrences.
- Copy creation and subsequent local edits follow ordinary Mass-plan autosave, Edit revision, Undo, and readiness/pagination recomputation behavior. Immutable Presentation snapshots remain unchanged.
- Replacement of a Song/Arrangement that already has Mass-local customization, and later promotion into the reusable library, remain separate reviewed workflows.

### 107. Guarded replacement of a customized Song item

- **Change song or arrangement** continues to open the ordinary staged picker even when the current Song item owns a Mass-local override. Merely browsing or selecting a candidate does not change the plan.
- Committing a different Song or Arrangement opens the established nested review slideover. It compares the current customized source with the staged replacement and warns that local lyric, sequence, and pagination changes will be detached from the active item.
- Ordo never attempts to merge Mass-local edits into a different Arrangement. After confirmation, the replacement becomes Arrangement-backed and its warnings, Cue/slide counts, and pagination are recomputed.
- **Return to library arrangement** is a direct action on the local-customization banner. Because it has no preceding picker context, its confirmation opens as one focused slideover rather than as a child nested over the Arrangement picker. The comparison identifies the original library Arrangement and the local changes being removed.
- Both confirmation paths leave the previous customized state recoverable through ordinary Mass-plan Undo and Edit revision history; Presentation snapshots remain immutable.
- Picker-to-review and parent-to-child slideovers must preserve focus, motion direction, stacking, and staged state. Back returns to the unchanged picker selection, cancellation keeps the Mass-local override, and reduced-motion mode removes spatial animation without changing the interaction sequence.

### 108. Song section-occurrence and generated-Cue hierarchy

- The Song item's section sequence is presented as ordered **section occurrences**, such as Verse 1, Chorus, Verse 2, Chorus. These remain the primary rows for understanding and changing the performance sequence.
- Expanding a section occurrence reveals its generated congregation-facing slide Cues in their exact playback order. Each Cue uses the same canonical rendered frame as the Inspector, scaled without recomposition.
- Two occurrences of a repeated section remain distinct positions in the sequence and therefore generate distinct ordered Cue occurrences, while visibly referencing the same linked section source unless one occurrence was explicitly forked.
- Reordering moves an entire section occurrence together with its generated Cues. Operators do not drag individual generated slides across section boundaries; text splitting and slide composition remain owned by the applicable reusable or Mass-local Lyrics editor.
- Selecting a generated Cue focuses that exact frame in the Inspector. Inspector previous/next navigation traverses the flattened Song Cue order and reveals the containing section occurrence when necessary.
- Provide one compact context-sensitive **Expand all / Collapse all** control in the section-sequence header. It changes only editor disclosure state, never the Arrangement, local override, Cue order, or saved pagination.

### 109. Viewport-bounded dialogs and slideovers

- Every dialog, floating slideover, and nested overlay is sized and positioned against the currently visible Ordo application viewport—not the document height or a content pane whose height may grow beyond the screen.
- Preserve a visible exterior gutter around floating overlays. A design described as full-application-height means the visible viewport minus that gutter; it never means the full height of an expanded underlying page.
- When content exceeds the available height, keep the overlay header and required commit/cancel actions reachable while scrolling only its content body. The underlying page does not expand the overlay or scroll in its place.
- Nested slideovers share the same viewport boundary. Parent recession, child focus, and transitions may not push either surface outside the visible bounds.
- Implement overlays through the application overlay/portal layer with explicit viewport-relative maximum height, overflow containment, focus trapping, and `Esc`/focus restoration. Verify the behavior at `1366x768`, `1920x1080`, short resized windows, and increased interface text scaling.

### 110. Song-level identification Cue ownership and editor placement

- Present the optional Song identification slide as a distinct **Song-level Cue** above the section-occurrence sequence in the selected Song-item editor.
- Keep it as the first Cue in the Song item's flattened Cue order whenever enabled. It is selectable like every other projectable Cue and drives the authoritative Inspector frame.
- Render it through the Theme's Identification specimen. Its title and available credits originate from the reusable Song work and Arrangement metadata rather than being duplicated as a Song section.
- Enable identification by default, with visibility controlled independently for each Song item in a Mass plan.
- Permit Mass-specific identification title or credit presentation changes as Mass-local values on that Song item without mutating reusable library metadata.
- Do not allow the identification Cue to be reordered between section occurrences. It belongs before the first section and is not a reusable Song section.
- Keep **Hold** and **Blackout** as separate Mass-item concepts; neither is embedded as a non-section Song Cue.

### 111. Response-sequence exchange blocks

- Build a Response-sequence item from ordered **exchange blocks** rather than an unstructured mixture of Cue rows.
- Each exchange block contains an optional operator-only trigger followed by one required congregation response. Priest (`P`) and congregation (`T`) are the defaults, while both roles remain explicitly editable for deacon, lector, cantor, commentator, and similar exchanges.
- Flatten each block into Presenter order as `operator-only trigger → generated response slide Cue(s)`. When no trigger is authored, the response Cue(s) enter the sequence directly.
- Selecting an operator-only trigger uses the established black Preview while showing its private prompt and role to the operator; advancing selects the paired projectable response.
- Show the response's generated canonical slide Cue or Cues beneath its exchange block, using the same Inspector selection and owner-directed pagination rules as other projectable content.
- Keep exceptional workflows that require arbitrary mixtures, control Cues, or non-paired content in **Custom sequence** instead of weakening the Response-sequence editor's focused model.

### 112. Mass ownership of template-derived Response sequences

> **Superseded by Decision 257.** Response/Prayer entries are reusable Library
> sources retained as links through template instantiation until explicitly
> customized for one Mass.

- Materialize every Response sequence supplied by a Mass template as an independent Mass-owned copy when the plan is created, consistent with the adopted template-instantiation boundary.
- Allow trigger roles/text, response roles/text, exchange order, and generated presentation settings to be edited directly in the Mass plan. Do not require the Song editor's **Customize for this Mass** step because the Response sequence is no longer a shared reusable object.
- Show lightweight provenance such as **Copied from Cebuano Sunday template** without implying inheritance or synchronization.
- Limit edits to the current Mass plan. Later source-template changes never rewrite it, and current-plan edits never silently update the template.
- Provide **Open source template** as a deliberate navigation path when the operator wants to correct future plan creation. Editing that template remains a separate context and does not retroactively change the current plan.
- Do not introduce a separate inherited response library in the MVP. Reuse recurring language-specific liturgical responses through Mass templates.

### 113. Mass-editor autosave, revision grouping, and Undo

- Use autosave throughout ordinary Mass-plan editing; do not add a routine **Save** button to selected-item editors.
- Display quiet persistence feedback in the Mass-plan header: **Saving…** while a local write is pending and **Saved** after it is durably stored.
- Commit text-field edits after one second of inactivity or focus departure and coalesce one continuous field-editing session into a useful undo unit rather than creating a revision for every keystroke.
- Create distinct undo points for structural and semantic commits such as adding/removing/reordering exchange blocks or section occurrences, changing a Song/Arrangement, creating or discarding customization, and approving pagination changes.
- Show temporary action feedback with **Undo** after structural changes. Keep `Ctrl+Z` available for the most recent Mass-plan edit after the temporary feedback disappears.
- Preserve explicit confirmation before guarded destructive-boundary operations; autosave and Undo do not replace the adopted replacement/customization review flows.
- Flush any pending field edit to local durable storage when the editor or application closes. Saving must not depend on internet connectivity.

### 114. Arrangement-free Song creation and nested duplicate preview

- Let the operator create a canonical Song work before creating any Arrangement. **Primary title** is the only required field; composer/original author, aliases, usual liturgical moments, occasion tags, provenance, and rights notes are optional.
- Keep Arrangement-owned fields out of New Song creation: no lyrics, language, Choir association, reusable sections, performance sequence, or slide boundaries are requested.
- Use an explicit **Create song** commit so abandoning the New Song form does not leave an empty library record. After creation, select the new Song overview and show a valid **No Arrangements yet** state with **Add first Arrangement** as the next optional action.
- Present possible duplicate Song works through a compact, collapsed disclosure such as **2 possible existing Songs**. Candidate rows do not permanently occupy the creation form.
- Expanding the disclosure shows concise candidate identities. Selecting one opens a nested, viewport-bounded preview while the New Song panel recedes; it does not navigate to the complete Song page.
- The nested preview shows only evidence needed to distinguish identity: title and aliases, composer/source, usual liturgical moments, and a compact Arrangement/Choir/language summary. Arrangement lyric excerpts may appear only when needed as match evidence.
- Back from the nested preview restores the New Song form exactly as entered. **Use this existing Song** abandons creation deliberately and selects that Song in the library; merely previewing never alters the form or library.
- After creation, ordinary Song-detail edits use library autosave and revision behavior.

### 115. Arrangement sections, inline slide boundaries, and two task tabs

- Use exactly two primary task tabs in the focused Arrangement editor: **Lyrics** and **Sequence**. Remove the provisional separate **Slides** tab.
- Treat an Arrangement section as a reusable musical/lyrical unit such as Verse 1, Chorus, Refrain, Bridge, or Ending. One section may generate one or several projectable slides without becoming several sequence entries.
- Keep exact lyric encoding, intentional line breaks, automatic slide proposals, manual slide-boundary adjustment, canonical generated-slide thumbnails, fit warnings, and pagination approval together inside **Lyrics** for the selected section.
- Represent a slide boundary as presentation structure inside its owning section. Moving or removing the boundary changes slide generation but never creates, removes, or reorders a section occurrence.
- Do not require artificial sections such as Verse 1A and Verse 1B merely because Verse 1 needs two slides. Such labels remain allowed when they describe genuinely distinct musical or lyrical parts.
- Keep **Sequence** focused on ordered section occurrences. One Verse 1 occurrence expands to all of Verse 1's generated slides, and repeated Chorus occurrences continue referencing the same reusable Chorus section.
- Keep the authoritative canonical Inspector visible across both tabs. Selecting a generated slide in Lyrics or a section occurrence in Sequence drives that Inspector.
- Do not retain a dedicated Pagination Review workspace at either Arrangement or Mass-plan level in the MVP. Imported `Needs Review` boundaries and later overflow findings open the exact affected content in its owning editor.

### 116. Keyboard-first lyric pagination and generated-slide navigation

- Encode one Arrangement section in a plain multiline lyric editor. A normal newline continues the current slide; one blank source line between lyric groups starts the next generated slide.
- Preserve ordinary text-editing semantics. Enter and Backspace are not branded or displayed as dedicated Ordo shortcuts: inserting the blank line creates a boundary, and deleting it naturally joins the neighboring slide groups.
- Keep the two GUI boundary actions immediately adjacent. Their labels are contextual and name the exact consequence, such as **Split after line 2** and **Join slide 2 with slide 1**, rather than showing ambiguous key badges.
- Disable the join action when the first generated slide is selected. Keep a quiet, persistent rule beside the editor: **A blank line starts a new slide.**
- Render generated slides as a compact, wrapping navigator of fixed-size `16:9` thumbnails. A lone thumbnail retains the same width and height it would have in a fuller row; it never stretches to fill the available column.
- Treat the Inspector as the single authoritative large preview. Selecting a navigator thumbnail selects its corresponding source range and drives the Inspector; editing the source regenerates the navigator and Inspector without introducing a separate Slides task tab.

### 117. Arrangement metadata boundary and staged details surface

- Keep Arrangement metadata in a dedicated, viewport-bounded floating **Arrangement details** surface opened from the focused editor header. Do not add a third primary task tab or mix this metadata into Lyrics or Sequence.
- Require at least one Arrangement language. Allow several languages only when that exact rendition genuinely uses them; language describes the Arrangement's owned lyric rendition rather than the Song work's identity.
- Keep **Display title** optional. When blank, derive the visible library label from the selected language and Choir context rather than forcing operators to maintain redundant names.
- Associate zero, one, or several Choirs with the Arrangement. Multiple Choirs share one Arrangement only when they sing the exact same rendition; Choir membership is discovery/applicability metadata, not ownership of a separate lyric copy.
- Keep translator/adapter attribution, occasion applicability, and source/rendition/rights notes optional and Arrangement-owned. Keep composer and original-author attribution on the canonical Song work.
- Keep exact lyrics, reusable sections, ordered section occurrences, and slide boundaries outside this surface in the adopted **Lyrics** and **Sequence** tasks.
- Reuse the same staged surface for **New Arrangement** and later **Arrangement details** editing. Creation commits explicitly with **Create Arrangement**; editing commits explicitly with **Save details**; Cancel discards staged metadata changes.
- When an edited Arrangement is shared by several Choirs, state that scope before Save. Saving updates the reusable Arrangement and its library filtering/discovery metadata while preserving all existing Mass references.
- Never rewrite an immutable Presentation snapshot after Arrangement metadata changes. A later snapshot may capture the updated metadata under the established snapshot rules.
- Offer **Duplicate Arrangement** from the existing-Arrangement state as a safe path when the operator intends to branch the rendition rather than update the shared record.

### 118. First-Arrangement creation transition and empty state

- Use the same adaptive Arrangement-details slideover for both creation and editing. In creation it is titled **New Arrangement**, omits existing-record actions such as Duplicate Arrangement, and changes the bottom commit to **Create Arrangement**; in editing it is titled **Arrangement details** and commits with **Save details**.
- Validate the adopted minimum of one language before creation. Keep the display title and all other Arrangement metadata optional under Decision 117.
- Commit creation explicitly. Do not persist an abandoned or invalid empty Arrangement merely because the slideover was opened.
- After **Create Arrangement**, select the newly created Arrangement and land directly in its **Lyrics** task.
- Treat an Arrangement with no sections as a valid authoring state. Show **No sections yet** and make **Add first section** the focused next action.
- Do not manufacture a default **Verse 1** section. Kyrie, Gloria, chants, refrains, and other forms do not share one reliable opening-section convention.
- Keep the empty Arrangement visible in the Song overview if the operator leaves before adding lyrics. It remains a deliberate created record rather than being silently deleted.

### 119. Arrangement-section creation and rename surface

- Give every Arrangement section one required free-form **Section name**. Do not add a separate Section Type field in the MVP; the operator's final label is the section's visible identity.
- Open **Add first section** and later **+ Add section** in the same compact, anchored creation popover rather than another full-height slideover or dialog.
- Offer editable naming suggestions such as **Verse 1**, **Chorus**, **Refrain**, **Bridge**, and **Ending**. Suggestions fill the ordinary name field and never restrict custom names such as **Kyrie**, **Verse 1A**, or **Memorial Acclamation**.
- Propose the next sensible numbered label from the current Arrangement, such as **Verse 3** after Verse 1 and Verse 2, while leaving the proposal fully editable.
- Commit explicitly with **Add section**. After creation, select the new section and focus its lyric editor so keyboard entry can begin immediately.
- Expose the selected section's compact action button beside its row. **Rename section** reuses the small naming surface with the current name populated; editing does not need a separate general-purpose Section-details screen.
- Renaming a section preserves its lyrics, slide boundaries, approval state, and every Sequence occurrence referencing it. Linked occurrences immediately display the new name.

### 120. Independent section creation and Sequence placement

- Creating a reusable section in **Lyrics** never creates or reorders a performance occurrence automatically. Lyrics owns reusable content; **Sequence** owns when and how often that content is performed.
- After section creation, allow a quiet contextual **Add to Sequence** action as an explicit shortcut. Invoking it performs a visible Sequence placement operation rather than treating placement as a hidden side effect of **Add section**.
- Mark an Arrangement with an empty Sequence as incomplete for presentation readiness once it has authored sections. Direct the operator to Sequence to establish performance order.
- Allow authored sections to remain unused without a warning. An unused section may intentionally represent an alternate verse, optional ending, or material retained for later use.
- Keep every Sequence placement as a distinct section occurrence. Repeating a Chorus adds another occurrence referencing the same reusable section; it never duplicates the Chorus lyrics.

### 121. Atomic recoverable Arrangement-section removal

- When a section has no Sequence occurrences, let **Remove section** commit immediately as an ordinary structural edit and show temporary **Undo** feedback.
- When one or several occurrences reference the section, require a guarded confirmation that names both consequences, such as **Remove “Chorus” and its 2 Sequence occurrences?**
- On confirmation, remove the reusable section and every occurrence referencing it in one atomic operation. Never retain broken occurrence placeholders or require the operator to hunt down each occurrence manually first.
- Capture enough revision state for **Undo** to restore the section name, lyrics, slide boundaries, pagination/approval state, and all removed occurrences in their original Sequence positions.
- Keep immutable Presentation snapshots unchanged. Removing the current library section cannot rewrite a snapshot already captured for presentation or export.

### 122. Defer dedicated section duplication

- Do not add a dedicated **Duplicate section** action in the MVP. The known use case—starting from a Chorus or other section whose lyrics need a slight variation—is expected to be uncommon enough that creating a section and copying its text is sufficient.
- Keep the copied result as an ordinary independent section. It receives its own required name, lyrics, slide boundaries, pagination approval state, and later edits; it does not remain linked to the source section.
- Preserve the adopted Lyrics/Sequence boundary: creating the copied section does not create a Sequence occurrence automatically.
- Reconsider a first-class duplication flow only if observed authoring repeatedly makes manual copy-paste burdensome.

### 123. Contextual Sequence-occurrence insertion

- When the operator explicitly adds a reusable section to **Sequence**, create a new occurrence immediately after the currently selected occurrence.
- If no occurrence is selected, append the new occurrence to the end. In an empty Sequence, the occurrence naturally becomes the first entry.
- Apply this insertion rule consistently whether placement begins from the Sequence task or through the contextual **Add to Sequence** shortcut in Lyrics.
- Keep the inserted occurrence visibly selected so its resulting position is unambiguous and the operator can immediately continue building or adjusting the performance order.

### 124. Independent Lyrics-section authoring order

- Let operators manually reorder reusable sections in the **Lyrics** task so sections created later can still be organized into a sensible authoring/navigation order.
- Treat this as authoring order only. Reordering sections never adds, removes, or repositions any Sequence occurrence and therefore never changes the Arrangement's performance order.
- Support both visible drag handles and `Alt+Up` / `Alt+Down` while a section row is focused. Keep the resulting move autosaved and recoverable through Undo.
- Preserve every section's content, slide boundaries, pagination approval state, and occurrence references during the move; only its position in the Lyrics section list changes.

### 125. Linked Sequence-occurrence repetition

- Provide **Repeat occurrence** on the selected Sequence occurrence as a fast path for common repeating patterns such as a Chorus between verses.
- Insert the repeated occurrence immediately after the selected occurrence, following the same contextual placement principle as other Sequence additions.
- Create only another occurrence referencing the same reusable section. Never copy the section, its lyrics, or its generated slides when repeating it.
- Keep adding the same section again from the available-sections list as an equivalent deliberate path; **Repeat occurrence** is a convenience rather than a distinct domain operation.
- Select the new occurrence after insertion so its position and identity are clear and it can immediately be moved or repeated again.

### 126. Atomic Sequence-occurrence reordering

- Reorder Sequence occurrences through visible drag handles or `Alt+Up` / `Alt+Down` while an occurrence is focused.
- Treat one occurrence as the smallest movable unit. Moving it carries every generated slide from its referenced section together and never changes their internal order.
- Keep generated-slide ordering owned by the reusable section in **Lyrics**. Do not expose individual slide reordering from Sequence.
- Autosave occurrence moves and make them recoverable through Undo while preserving the selected occurrence after the move.
- Keep multi-selection and bulk occurrence movement out of the MVP.

### 127. Recoverable Sequence-occurrence removal

- Remove a selected Sequence occurrence immediately without confirmation because this operation never deletes its reusable section, lyrics, or generated slides.
- Expose removal through the occurrence action surface and the `Delete` key while the occurrence row or Sequence list owns keyboard focus. Never intercept `Delete` from a lyric editor, text field, metadata control, or other nested input.
- After removal, select the occurrence that moves into the removed position. If none follows, select the preceding occurrence; if the Sequence becomes empty, move focus to its empty-state action.
- Let removing the final occurrence leave a valid authoring state whose Sequence is explicitly **Incomplete**. The reusable section remains available for later placement.
- Autosave the removal and provide both temporary Undo feedback and `Ctrl+Z`; restoration returns the occurrence to its exact former position.

### 128. Empty Sequence as an incomplete draft

- Treat an empty Sequence as a valid saved authoring state, but never label the Arrangement presentation-ready.
- Show a focused empty state: **No performance sequence yet**, a short explanation that sections must be added in sung order, and **Add first occurrence** as the primary action.
- Label the Arrangement **Incomplete** while its Sequence is empty. If a Mass references it, surface a Mass Readiness finding that identifies the Song and routes directly to this Arrangement's Sequence task.
- Until an occurrence is added, the Arrangement contributes no congregation-facing Song slides to a newly prepared Mass or Presentation snapshot.
- Keep existing immutable Presentation snapshots unaffected by later removal of all current-library occurrences.

### 129. Sequence-reachable Arrangement readiness

- Evaluate Arrangement and downstream Mass readiness only from reusable sections actually referenced by Sequence. Unused sections may remain unfinished alternatives without producing warnings.
- Treat a referenced section with blank lyrics or no valid canonical frame as **Incomplete**, because its intended occurrence cannot produce faithful congregation-facing output.
- Treat referenced but unapproved pagination as **Needs Review** and a readiness warning rather than incompleteness, consistent with the established Mass readiness policy.
- Clear the finding when the referenced section is renderable and its pagination is approved.
- When the same problematic reusable section is referenced several times, report one actionable finding with its affected occurrence count rather than repeating identical findings for every occurrence.

### 130. Position-based Sequence-occurrence presentation

- Display each Sequence occurrence with its current one-based position and unchanged reusable-section name, such as **02 Chorus** and **04 Chorus**. Never rename repeated occurrences to artificial section identities such as Chorus 1 and Chorus 2.
- Treat the position as a transient display label rather than stored content identity. Recompute it immediately after insertion, removal, or reordering.
- Show the occurrence's generated-slide count in its collapsed summary. Expanding it reveals those fixed-order generated slides; selecting a child slide drives the authoritative Inspector.
- Keep generated slides subordinate to their occurrence and section. Repetition may produce visually identical expanded children, but each occurrence remains independently selectable and movable in performance order.

### 131. Stable three-pane Sequence workspace

- Reuse the Arrangement editor's established three-pane shell when switching from Lyrics to **Sequence**, preserving spatial continuity rather than navigating to a differently structured editor.
- Use the left pane as **Available sections**, showing each reusable section's name, generated-slide count, usage count, and an explicit **Add** action. Clicking or focusing the section name alone never mutates Sequence.
- Use the center pane for the ordered, expandable performance Sequence and its adopted occurrence insertion, repeat, reorder, and removal actions.
- Keep the right pane as the same authoritative canonical Inspector used by Lyrics. Selecting an occurrence previews its first generated frame by default; selecting an expanded child slide previews that exact frame.
- Apply the adopted contextual insertion rule when an Available-section **Add** action is invoked, then visibly select the resulting center-pane occurrence.

### 132. Persistent Song Library master-detail workspace

- Use a desktop master-detail workspace for **Songs** at the supported `1366x768` and larger operating sizes rather than navigating between a full-width result page and Song detail.
- Keep the left browser pane responsible for the sticky Songs heading, **New song**, search, filters, visible result count, and Song-work results. Keep its results independently scrollable when the library exceeds the available height.
- Keep the right detail pane responsible for the selected Song work's canonical identity, attribution and metadata summary, reusable Arrangement list, and Song-level actions.
- Let selecting a result update the right detail pane in place without resetting the active query, filters, result scroll position, or search-field focus unnecessarily.
- Opening an Arrangement transitions into its full Arrangement editor. Returning to Songs restores the prior query, filters, selected Song, and result scroll position rather than reconstructing a default library view.
- Adapt below ordinary desktop width by progressively stacking the panes; do not replace the primary desktop design with a card grid.

### 133. Arrangement-level Song Library filtering

- Apply Choir, language, and occasion filters to individual Arrangements rather than independently to the Song work as a whole.
- Retain a Song-work result only when at least one single Arrangement satisfies every active Arrangement filter. Never combine attributes from different Arrangements to manufacture a match.
- State the result relationship compactly, such as **2 matching · 4 total**, so the operator can distinguish the filtered matches from all Arrangements owned by the Song work.
- In the selected Song detail, order matching Arrangements first and emphasize them. Keep nonmatching Arrangements visible but subdued rather than hiding them or making them appear deleted.
- Treat missing Choir or occasion metadata as **Unspecified**, not as universal applicability. An unspecified Arrangement does not satisfy a selected Choir or occasion filter unless **Unspecified** itself is deliberately selected.

### 134. Normalized Song Library search scope

- Search Song-work title, aliases, composer/original-author attribution, and source metadata, together with Arrangement display title, translator/adapter attribution, Choir, language, and full lyrics.
- Keep search results at the Song-work level. When a query matches Arrangement-owned data, require that same Arrangement to satisfy every active Arrangement filter; never satisfy the query with one Arrangement and the filters with another.
- When a query matches Song-work identity or attribution, retain the Song when it owns an Arrangement satisfying all active filters.
- Explain non-obvious results with a compact match reason, such as a matched alias or a short lyric excerpt. Order and emphasize the matching Arrangements under the adopted master-detail behavior.
- Normalize indexed text and queries using Unicode-aware normalization and case folding so search is case-insensitive across supported languages. Normalize inconsequential spacing consistently, but preserve meaningful letters and diacritics in stored/displayed content; normalization affects matching only and never rewrites authored text.

### 135. Automatic-first Song import review and content comparison

- Validate every selected Song Import Document before changing the library. Keep all imported content staged and read-only until the operator commits reviewed actions.
- Classify each candidate automatically as **Add new Song**, **Attach Arrangement**, **Skip unchanged**, or **Needs decision**. Explain each suggestion with concrete normalized identity, attribution, metadata, lyric, Sequence, and pagination evidence as applicable.
- Exclude contract-invalid documents before commit and report their precise validation failures. Invalid candidates remain inspectable in the review session but never contribute a library mutation.
- Require the operator to resolve every **Needs decision** candidate before committing the batch. Allow any suggested action to be changed deliberately during review.
- Commit all approved mutations in one transaction. If any approved action fails, save none of the batch; skipped and invalid candidates do not participate in that transaction.
- Preview the imported document from its comparison panel. When credible library candidates exist, open the same read-only, full-app, viewport-bounded **Compare contents** slideover rather than separate preview surfaces.
- In that slideover, compare the imported document with one selectable nearest library match side by side. Provide **Overview**, **Lyrics**, **Sequence**, and **Slides** views, preserve the underlying review selection and staged action, and return without editing either side.
- Keep the nearest-match selector visible only when one or more credible candidates exist. Comparison is evidence for an import decision, never an automatic merge editor.

### 136. Near-duplicate Arrangement import conflicts

- Classify an imported Arrangement that resembles an existing Arrangement but contains meaningful metadata, lyrics, section, Sequence, or pagination differences as **Needs decision**. Never silently attach it as a new variant or overwrite the existing Arrangement.
- Offer exactly three deliberate outcomes: **Add as new Arrangement**, **Update existing Arrangement**, or **Skip document**. Use **Add as new Arrangement** as the safe initial recommendation when Ordo cannot prove update intent.
- Treat **Update existing Arrangement** as whole-Arrangement replacement under the existing Arrangement identity, not as a field-by-field or line-by-line merge. Capture the replacement as a new restorable Edit revision.
- Before updating, disclose the existing Arrangement's associated Choirs and unsnapshotted Mass plans that will observe the revised reusable content. Preserve Mass-local overrides and immutable Presentation snapshots unchanged.
- Keep lyric, section, Sequence, and pagination comparison available as evidence before the operator chooses. Do not attempt automatic conflict resolution inside those structures in the MVP.

### 137. Song-level identity and per-Arrangement import actions

- Treat one Song Import Document as one internally coherent Song-work candidate that may contain several imported Arrangements.
- Resolve the document's Song-work identity once: either create one new Song work or associate the document with one existing Song work. Never attach different Arrangements from one document to several Song works.
- After Song identity is resolved, classify and review each imported Arrangement independently as a new attachment, unchanged skip, existing-Arrangement update, deliberate new variant, or unresolved decision under Decisions 135–136.
- Preserve whole-document contract validity. If any Arrangement or internal reference is invalid, reject the entire document rather than recovering or importing a valid-looking subset in the MVP.
- Require every Arrangement decision within every otherwise valid approved document to be resolved before enabling the atomic batch commit.

### 138. Song and Arrangement autosave boundaries

- Autosave ordinary Song-detail fields and Arrangement lyric text after one second of inactivity or when focus leaves the edited field. Keep authored content local-first and require no network or routine global Save action.
- Coalesce one continuous typing session into one persisted, undoable edit rather than creating a revision for every keystroke.
- Persist section creation, rename, removal, and reorder; Sequence insertion, repeat, removal, and reorder; and slide-boundary changes immediately as discrete undo points.
- Preserve explicit transactional boundaries where they already communicate staged intent: **Create song**, **Create Arrangement**, and **Save details** commit their respective New Song, New Arrangement, and Arrangement-details surfaces; Cancel discards those staged changes.
- Show quiet editor-header persistence feedback as **Saving…**, **Saved**, or **Couldn’t save**. Do not use successful saves as disruptive notifications.
- Flush pending local edits before navigation or application close. If persistence fails, keep the recoverable edit visible and warn before allowing the operator to leave rather than silently discarding it.

### 139. Editor-local immediate Undo boundary

- Scope immediate Undo and Redo to the currently open Song or Arrangement editor session. Never let an ordinary `Ctrl+Z` reach into a different Song or Arrangement that the operator is not viewing.
- While a text field owns focus, use that field's native text Undo and Redo first so character- and selection-level corrections behave normally during the active editing session.
- Add persisted text-edit sessions and structural Arrangement operations to the current editor's application-level Undo stack. Temporary **Undo** feedback and `Ctrl+Z` address the same editor-local history after an operation commits.
- Clear the immediate editor-session Undo stack when the operator navigates away from that Song or Arrangement or restarts Ordo. Do not make an unseen prior entity the target of a later shortcut.
- Use durable Edit revisions and the explicit **History** surface for recovery after navigation, restart, or older changes. Durable restoration creates a new latest revision rather than rewriting historical states.

### 140. Fine-grained durable revisions with grouped History

- Persist every committed Song or Arrangement Undo point as a durable Edit revision, including each coalesced text-edit session and each discrete structural operation. Do not reserve recoverable history only for manually named checkpoints.
- Preserve the fine-grained revisions underneath the interface so recovery can reach a specific intermediate state after navigation, restart, or interruption.
- Group consecutive related revisions from one editing session into a readable History entry, such as **Edited lyrics · 8 changes**, rather than presenting a flat row for every small edit.
- Let the operator expand a grouped session to inspect and restore its individual revisions when finer recovery is necessary.
- Show creation, import, whole-Arrangement replacement, and historical restoration as explicit standalone History events because they represent meaningful provenance or safety boundaries.
- Retain all Song and Arrangement Edit revisions in the MVP. Do not add automatic pruning or revision-retention settings before real parish usage demonstrates a storage problem.

### 141. Song and Arrangement History-session boundaries

- Start a History editing session with the first committed change after the operator opens a Song or Arrangement editor. Opening and inspecting an entity without changing it creates no empty History session.
- Keep the same Arrangement session across **Lyrics** and **Sequence** navigation because those tasks modify one Arrangement identity.
- End the session when the operator navigates to another Song or Arrangement, closes Ordo, or reaches 30 minutes without a committed change. A later edit starts a new session even if the same editor remained visible.
- Split standalone creation, import, whole-Arrangement replacement, and historical restoration events from surrounding ordinary edits so their provenance remains independently visible.
- Base session grouping on the edited entity. Song-work detail edits and edits to one of its Arrangements never collapse into the same History session.

### 142. Entity-owned Song and Arrangement History surface

- Expose a visible labeled **History** action in the Song-detail header for Song-work identity and metadata history, and another in the Arrangement-editor header for that Arrangement's details, lyrics, Sequence, and pagination history.
- Open History as a viewport-bounded floating right slideover spanning the full application chrome. Preserve the current editor, selection, and navigation context underneath rather than routing to a separate page.
- Flush any pending autosave before opening History. If that flush fails, retain the unsaved edit and do not present an ambiguous supposedly current comparison state.
- List grouped editing sessions with date and time, a generated change summary, revision count, and provenance such as manual edit, import, or restore.
- Expand grouped sessions inline to expose their individual durable revisions without replacing the readable session-level overview.
- Keep Song History and Arrangement History separate and entity-owned. A Song's History does not aggregate its child Arrangements, and an Arrangement History does not mix Song-work metadata revisions into its timeline.

### 143. Song and Arrangement revision comparison and restoration

- Selecting an individual revision opens a nested comparison slideover while the parent History surface recedes and retains its expanded sessions and selection.
- Compare the current entity with the selected historical revision without mutating either. Song comparison covers identity and metadata; Arrangement comparison provides **Overview**, **Lyrics**, **Sequence**, and **Slides** views consistent with Song-import comparison language.
- Emphasize differences and keep unchanged content available on demand. Use the authoritative canonical frame for historical Arrangement slide previews.
- Restore the entire Song or Arrangement revision in the MVP. Do not add field-, section-, line-, occurrence-, or slide-level cherry-picking.
- For an Arrangement restore, disclose associated Choirs and editable Mass plans that will observe the restored reusable content. Preserve Mass-local overrides and immutable Presentation snapshots unchanged. A Song restore changes only that Song work's identity and metadata and does not rewrite its Arrangements.
- Make restoration a two-stage action in the comparison footer. **Restore this revision** transforms the entire footer into a warning-tinted confirmation state with the explanation on its own row and **Keep current** plus **Restore entire Arrangement/Song** on a separate action row.
- Do not enclose the confirmation in another card. **Keep current** returns the footer to its initial state without changing content.
- A confirmed restore creates a new latest Edit revision and leaves all intervening history intact. Show immediate success feedback with **Undo**; Undo likewise creates a later revision.

### 144. Independent Arrangement archival lifecycle

- Allow an Arrangement to be archived and restored independently from its parent Song work. An obsolete choir, seasonal, or language rendition must not require retiring the canonical Song work or its other useful Arrangements.
- Exclude archived Arrangements from ordinary Mass-planning and Arrangement-selection results while retaining them in explicit archived searches and their parent Song's lifecycle view.
- Preserve every existing Mass-plan reference, historical record, Edit revision, and immutable Presentation snapshot that depends on an archived Arrangement. Archival never substitutes another Arrangement or cascades into referenced content.
- Keep the parent Song work active when only one of its Arrangements is archived. Song-work archival remains a separate lifecycle action under Decision 43.
- Permit permanent Arrangement deletion only when no durable record references it. When deletion is unsafe, identify the blocking references and retain the recoverable archived record.

### 145. Song and Arrangement lifecycle-action placement

- Keep frequent creation, editing, and recovery actions visually primary while placing low-frequency lifecycle actions in the owning entity header's overflow menu.
- In Song detail, expose **New arrangement** and **History** directly. Place **Merge with another song** and **Archive song** in the adjacent Song overflow, with Archive separated as the destructive lifecycle group.
- In the Arrangement editor, expose **Arrangement details** and **History** directly and place **Archive arrangement** in its adjacent overflow.
- Keep **Duplicate arrangement** in Arrangement Details, where its metadata and branching context are already visible, instead of duplicating it in the header overflow.
- For an archived Song or Arrangement, replace Archive with **Restore** and expose **Delete permanently** in the same overflow. Keep permanent deletion invokable even when references make it unsafe so Ordo can show the blocking references and recovery-safe alternative instead of silently hiding or disabling the action.

### 146. Non-cascading Song and Arrangement archival state

- Archiving a Song work suppresses that Song and all of its Arrangements from ordinary planning without changing the independent archival state stored on any child Arrangement.
- Restoring the Song makes only its still-active Arrangements ordinarily available again. An Arrangement archived independently before or during the Song's archival remains archived until explicitly restored.
- Never model Song archival as a batch of child-Arrangement archival edits. Preserve child lifecycle intent, Edit history, and all existing Mass-plan and snapshot references unchanged.

### 147. Archive confirmation, navigation, and immediate recovery

- Confirm Song and Arrangement archival in a compact, viewport-bounded centered dialog rather than a nested slideover or full workflow surface.
- Name the affected entity and summarize the planning impact. For a Song, disclose its child-Arrangement count; for an Arrangement, disclose Choir associations and editable Mass references. State explicitly that existing plans and immutable Presentation snapshots remain unchanged.
- Require only **Cancel** and the clearly labeled Archive action. Do not require title entry or another high-friction confirmation for this recoverable operation.
- After archiving a Song, select the next visible Song in the current active-library result set, otherwise the previous visible Song. When neither exists, focus the active-list empty state while keeping Undo available. After archiving an Arrangement, return to its parent Song detail.
- Show immediate success feedback with **Undo**. Provide an explicit `Status: Archived` Song Library filter for later discovery and restoration.

### 148. Inspect-only archived Song and Arrangement state

- Make archived Songs and Arrangements inspect-only until explicitly restored. Keep metadata, lyrics, Sequence, generated slides, and entity-owned History viewable while suppressing editing controls and structural shortcuts.
- Present a clear **Archived** status banner with **Restore** as the prominent lifecycle action. Do not allow ordinary edits to accumulate on content operators reasonably understand to be retired.
- For an archived Song, keep its complete Arrangement list visible for inspection and preserve each child's independent archival state.
- Label an independently active child Arrangement beneath an archived Song as **Available when Song is restored**. Continue labeling an independently archived child as **Archived**; restoring the parent never changes that child state.
- Hide **New arrangement** while the Song is archived. Restore the ordinary authoring controls only after the Song work is restored.

### 149. Childless-only permanent Song deletion

- Never cascade permanent Song-work deletion through its Arrangement tree, even when the child Arrangements appear externally unreferenced.
- Require a Song work to be archived, childless, and otherwise free of durable references before permanent deletion is permitted.
- When Arrangements remain, show each one as a blocking child and require the operator to inspect and permanently delete it through its own reference-safety review.
- Keep every Arrangement deletion independently accountable for its Mass-plan, snapshot, History, and other retained references rather than treating parent deletion as authority to bypass those checks.

### 150. Archived Arrangement references in Mass plans

- Treat a Mass plan's selected reusable Arrangement as a durable reference. Archiving that Arrangement or its parent Song never removes, substitutes, copies over, or invalidates the existing selection.
- Continue rendering the archived library source in every existing Mass plan. Mark the affected Song item with an **Archived library source** warning rather than classifying otherwise valid projectable content as incomplete.
- Let the operator inspect the archived source read-only, restore it, replace it, or remove the Song item. Exclude the archived source only from ordinary selection for another or newly added Song item.
- When duplicating a Mass plan or reusing it for another Celebration, preserve archived Arrangement references for source fidelity and disclose them in the creation review so the operator may deliberately replace them.
- Block permanent Arrangement deletion while any retained Mass plan, Presentation snapshot, or other durable record refers to it. Group and name the blockers, link to editable plans where practical, and never strand a reference through deletion.

### 151. Reference-safety review for permanent deletion

- Open one viewport-bounded reference-safety dialog when **Delete permanently** is invoked for an archived Song or Arrangement.
- If deletion is blocked, group and identify remaining child Arrangements, Mass plans, Presentation snapshots, and other durable references. Link to editable blockers where practical and do not expose an enabled deletion commit.
- If deletion is safe, state exactly which entity content and owned Edit revisions will be erased, then offer **Cancel** and **Delete permanently**. The overflow selection plus this explicit dialog provides sufficient deliberation; do not require retyping the title.
- Treat the entity's own revisions as part of the entity being deleted rather than as self-blocking external references. Disclose their loss in the safe-state summary.
- Make permanent deletion non-undoable. Keep Archive and Restore as the ordinary recoverable lifecycle path.

### 152. Song-merge candidate selection and survivor assignment

- Open **Merge with another song** as a viewport-bounded floating right slideover spanning the full application chrome while preserving the selected Song Library context beneath it.
- Default the currently open Song work to **Survives**. Let the operator search for and select exactly one other Song work as the proposed duplicate to **Retire**.
- Preview the selected candidate's canonical identity, attribution, aliases, liturgical use, and Arrangement summary inside the same entry surface before proceeding.
- Label both roles explicitly and provide **Swap survivor** so the better canonical identity may survive without abandoning and restarting the workflow.
- Enable **Continue to review** only after both roles are defined. Do not mutate either Song, its Arrangements, references, or redirects during candidate selection or role swapping.

### 153. Song-merge review navigation

- Treat Song merge as one linear two-step workflow inside the same full-app floating slideover: **Choose songs** followed by **Review merge**.
- Advancing to review replaces the main slideover's contents rather than opening another top-level slideover. **Back** returns to candidate selection while preserving the selected Songs and survivor assignment.
- Organize the main review around the surviving identity and metadata, Arrangements moving under it, and preserved references/redirect consequences.
- Reserve a Laravel Cloud-style nested child slideover for inspecting one specific conflict or comparison, such as two similar Arrangements. While the child is focused, recess the parent review visually without discarding its state or scroll position.
- Returning from the child comparison restores the same review position. Opening or closing either review surface remains non-mutating; only the eventual final merge commit may alter library state.

### 154. Song-merge metadata resolution

- Keep the surviving Song work's primary title unchanged. Convert the retiring Song work's primary title into an alias of the survivor.
- Automatically take the union of aliases, usual liturgical moments, and occasion-applicability tags, normalizing and deduplicating textual values case-insensitively.
- Collapse exact metadata matches without asking the operator to resolve a false conflict.
- Preserve source and provenance records as distinct entries rather than overwriting or flattening one source into another.
- Treat conflicting composer or original-author values as blocking identity conflicts. Require an explicit choice of the canonical value before merge commit; never automatically concatenate attribution facts or silently prefer the survivor.
- Keep all metadata work staged and reversible during review. Automatic resolutions are visible before commit and may not mutate either Song merely because the review was opened.

### 155. Song merge rehomes but never combines Arrangements

- Rehome every Arrangement owned by the retiring Song beneath the surviving Song while preserving the Arrangement's UUID, full Edit revision history, Choir associations, applicability metadata, archival state, and other identity.
- Preserve all Mass-plan, Presentation-snapshot, import, and other durable references to those Arrangements because their UUIDs and content remain intact; only their parent Song changes.
- Detect and flag exact or likely duplicate Arrangements for operator comparison, but do not line-merge or consolidate their lyrics, sections, sequence, pagination, metadata, or histories as part of Song merge.
- Keep compared Arrangements separate even when their current content appears identical. Song merge therefore means collecting independent Arrangements beneath one canonical Song work, not fusing Arrangement identities.
- Defer any dedicated Arrangement-merge operation beyond the MVP. This conservative boundary prevents accidental loss and keeps Song-merge reversal structurally reliable.

### 156. Atomic Song-merge commit

- Treat the completed merge review as the confirmation boundary. Commit through one explicit **Merge songs** action without adding another dialog or requiring the operator to retype a title.
- Disable commit while any blocking identity conflict, such as contradictory composer or original-author metadata, remains unresolved. Likely duplicate-Arrangement findings are review warnings and do not block because Arrangements remain independent.
- Execute metadata resolutions, intact Arrangement reparenting, creation of the retiring-Song redirect, and recording of the dedicated merge-history event in one atomic storage transaction.
- If validation, storage, asset coordination, or any other part of the transaction fails, roll back the entire attempt and leave both Songs, all Arrangements, and all references unchanged.
- After success, navigate to the surviving Song detail and show a confirmation with **Undo merge**. Do not leave the operator on a retired identity or a stale review surface.

### 157. Targeted Song-merge reversal

- Record enough field-level and structural provenance in the merge-history event to reverse the merge as a targeted inverse transaction rather than restoring the entire library to an earlier snapshot.
- Let the immediate success toast's **Undo merge** run the inverse directly while no intervening edit has affected the merge. Expose **Reverse merge** from the dedicated History event for later recovery through an impact review.
- Restore the retired Song under its original UUID, remove its redirect, and move only the Arrangements recorded as transferred by that merge back beneath it.
- Preserve those Arrangements' current content, Edit revisions, Choir associations, and lifecycle state when moving them back; do not rewind Arrangement edits or archival changes made after the merge.
- Reverse only Song metadata changes attributable to the merge. Leave unrelated later edits and Arrangements created under the survivor after the merge untouched.
- If later edits overlap metadata that the inverse must change, require explicit conflict review before reversal and never overwrite those later values silently.
- Execute reversal atomically. If its dependencies cannot be reconciled, make no changes and explain the blocking impact rather than leaving a partially restored identity graph.

### 158. Retired Song identity redirects

- After merge, retain the retiring Song's UUID as an internal **Merged Song redirect** rather than treating the retired identity as an archived Song or leaving it as an independently selectable library record.
- Resolve every lookup of that UUID to the current surviving Song. Exclude the redirect from ordinary Song Library results, but let searches for an old title or alias return the survivor with a visible **Matched merged identity** explanation.
- Route old internal links to the survivor and briefly disclose that the requested Song was merged. Do not strand bookmarks, History links, import evidence, or other internal navigation on a dead identity.
- Resolve a Song import document that names a retired UUID to the survivor automatically while disclosing the redirect during import review. Write the survivor's current UUID in all new exports and references.
- When a surviving Song is merged again, flatten every earlier redirect so each retired UUID points directly to the newest survivor. Reject any operation that would introduce a redirect loop.
- Retain the merge provenance needed to reverse an eligible merge and reconstruct the prior redirect relationships atomically.

### 159. Active-only Song-merge eligibility

- Permit Song merge only when both the proposed survivor and retiring Song are active library records.
- Exclude archived Songs from ordinary merge-candidate search and disable **Merge with another song** while inspecting an archived Song.
- Require the operator to restore an archived Song before it may participate in a merge. Do not let merge implicitly restore, retire, or otherwise bypass an intentional archival state.
- Because both inputs are active, the resulting survivor remains active and its rehomed Arrangements retain their own independent active or archived lifecycle states.

### 160. Newest-first reversal of chained Song merges

- Allow merge redirects to chain historically while flattening their runtime resolution to the newest surviving Song.
- If merge `A → B` is followed by `B → C`, block reversal of `A → B` while `B → C` remains applied. Explain **Reverse the newer merge first** and link to that newer merge-history event.
- Reversing `B → C` atomically restores Song B and reconstructs the prior `A → B` redirect relationship from stored merge provenance. The earlier merge may then be reversed independently.
- Apply the same newest-first rule to longer chains. Never split, fork, or partially redirect an applied identity chain merely to reverse an older event out of order.

### 161. Redirect-protected permanent deletion

- Treat a Merged Song redirect as durable identity evidence for as long as its merge remains applied. Do not expose independent Archive or Delete actions for the redirect record.
- Block permanent deletion of a surviving Song while any retired Song UUID redirects to it, even if the survivor has no Arrangements or other ordinary references.
- Group incoming redirects as explicit blockers in the existing permanent-deletion safety dialog and link each one to its merge-history event.
- Require the operator to reverse the relevant merges first, thereby restoring the retired Songs and removing their redirects, before the survivor may otherwise qualify for permanent deletion.
- Never satisfy deletion by silently erasing redirects, rewriting historical UUIDs, or leaving old imports and internal links unresolved.

### 162. Preserve rejected attribution claims as provenance

- When a composer or original-author conflict is resolved, store exactly one operator-selected canonical value on the surviving Song work.
- Preserve each rejected attribution claim as noncanonical provenance associated with its source evidence rather than adding it as a second canonical author or discarding it.
- Make the preserved claim inspectable from the surviving Song's provenance and merge History, clearly distinguishing **canonical attribution** from **source-reported claim**.
- If the merge is later reversed, use the recorded field provenance to restore each Song's prior canonical attribution without losing the evidence captured during review.

### 163. Preserve retired Song history as a separate lineage

- Add one dedicated **Songs merged** event to the surviving Song's History rather than interleaving every pre-merge Edit revision from the retired Song into the survivor's timeline.
- Keep the retired Song's prior Edit revisions as a sealed lineage associated with its Merged Song redirect and the merge-history event. Let the operator inspect that lineage from the event without exposing the retired Song as an independently editable library entry.
- Keep each Arrangement's History attached to its unchanged Arrangement UUID while the Arrangement is rehomed beneath the survivor.
- When a merge is reversed, restore the retired Song under its original UUID with its original revision lineage intact, then append the reversal as later history rather than rewriting or deleting the merge record.

### 164. One History-owned Song-merge reversal review

- Expose **Reverse merge** from the dedicated merge event in the surviving Song's History. Open it as a nested, viewport-bounded child slideover while the parent History surface recedes, following the established entity-revision inspection pattern.
- Present one impact review covering the retired Song identity that will be restored, the recorded Arrangements that will return beneath it, merge-attributable metadata and alias changes, removal or reconstruction of redirects, and any later conflicts or blocking dependencies.
- Preserve the parent History event, scroll position, and underlying Song context when entering or leaving the reversal review.
- Keep the immediate post-merge **Undo merge** action direct only while no intervening edit or dependency makes reversal unsafe. If direct Undo is no longer valid, route the operator into this same impact-review surface rather than failing silently or inventing a second recovery flow.
- Commit through one explicit **Reverse merge** action after every required conflict is resolved. Reuse this single surface for clean, conflict-resolution, and blocked reversal states.

### 165. Preserve later permanent Arrangement deletion during merge reversal

- Do not treat the merge-history event alone as a durable content reference that indefinitely blocks permanent deletion of an Arrangement transferred by that merge. The Arrangement must still pass every ordinary child, Mass-plan, Presentation-snapshot, History, and other reference-safety rule before deletion.
- When reversing the Song merge later, preserve a transferred Arrangement's deliberate subsequent permanent deletion rather than resurrecting erased content or failing the entire reversal merely because that Arrangement no longer exists.
- Restore the retired Song and reverse all otherwise applicable merge effects atomically, but leave the permanently deleted Arrangement absent. Retain sufficient tombstone provenance in the merge event to distinguish this known later deletion from corruption or an unexplained missing dependency.
- Disclose the exception prominently in the reversal impact review, including the number and identity evidence of transferred Arrangements that were permanently deleted and cannot be restored.
- Treat this as preservation of a later non-undoable action, not as an unnoticed partial reversal. An unexplained missing Arrangement remains an integrity failure and must block reversal.

### 166. Protect later Song metadata during merge reversal

- Detect a reversal conflict whenever a Song metadata value changed by the merge has since been edited, removed, repurposed, or otherwise diverged from the merge event's recorded result.
- Show only the affected fields in a three-state comparison: **Before merge → Current → Reversal choice**. Preserve access to provenance without forcing the operator through unchanged fields.
- Require an explicit resolution for every overlapping field before enabling **Reverse merge**. Default each resolution to **Keep current** so a historical inverse never silently overwrites newer work; let the operator deliberately restore the pre-merge survivor value instead.
- Restore the retired Song's own original metadata from its sealed lineage independently of the survivor-side choice. Preserve source evidence and rejected attribution claims according to their recorded provenance.
- Apply the same conflict rule to canonical attribution, aliases, usual liturgical moments, occasion-applicability tags, and any later Song metadata introduced into the merge model.

### 167. Defer the complete Song-merge capability until after the MVP

- Exclude Song merge, retiring-Song redirects, merge History events, merge reversal, and their special conflict and dependency handling from the initial church deployment.
- Keep Decisions 152–166 and the approved merge mockups as post-MVP design research rather than discarding them or treating them as current implementation requirements.
- In the MVP, resolve suspected duplicate Songs through ordinary inspection, metadata correction, Arrangement reuse, archival, and reference-safe deletion where eligible. Do not expose a partial or irreversible merge shortcut.
- Do not let unfinished merge failure-state design block implementation of the otherwise complete MVP Song Library and Arrangement Editor.

### 168. Owner-directed pagination correction instead of a review workspace

> **Superseded in part by Decision 257.** General Presentation Text replaces
> the former Custom text editor label; owner-directed correction remains
> adopted.

- Remove the standalone **Pagination Review** workspace from the MVP. Pagination remains a rendering concern and warning category, but not a second place for editing projectable text.
- Expose contextual **Fix pagination** from the authoritative Inspector warning and from Readiness or Theme affected-plan findings. Navigate directly to the exact affected content in its owning editor and retain the Inspector as the canonical preview.
- Route a reusable Arrangement finding to its affected Lyrics section in the Song Library; route a Mass-local Song override to its local Lyrics section; and route Response sequences, Readings, and General Presentation Text to their own Mass-item text editors.
- Keep pagination boundary changes, text changes, and their Undo/revision ownership with that source editor. Do not duplicate those controls inside the Inspector or create an intermediate review surface.
- Use the Readiness panel as the plan-wide overview for unresolved pagination findings. Resolving source content recomputes warnings automatically.

### 169. Inspector navigation follows the selected Mass item's Cue order

- This narrowly supersedes Decision 11's ambiguous operator-only Preview
  wording: the canonical Preview frame is black, while private prompt, role,
  timing, and related-response context remain outside it.
- Navigate the Inspector through the selected Mass item's flattened **Cue** order rather than counting only congregation-facing slides. Label position as **Cue n of total** so operator-only triggers are represented honestly.
- Show a projectable Cue as its authoritative canonical congregation frame. Show an operator-only trigger as a black congregation frame with its private minister role, prompt, and timing context outside the frame; operator-only text may never appear inside the canonical output.
- Keep editor and Inspector selection synchronized in both directions. Clicking a generated Cue focuses it in the Inspector, while Inspector Previous/Next selects and reveals the corresponding Cue in the owning editor hierarchy.
- Stop navigation at the first and last Cue of the selected Mass item and disable the unavailable direction. Do not silently select an adjacent Mass item; changing item scope remains an explicit Mass-outline action.
- Preserve the full `16:9` authoritative frame while showing warning, source, Theme, and private operator context around it. Use contextual **Fix pagination** only for affected projectable content.
- Treat `ordo-inspector-cue-navigation*` as the approved interaction and information-hierarchy reference for this behavior.

### 170. Mass sections group the outline in plans and templates

- Introduce **Mass sections** as ordered named groupings of Mass items in both reusable Mass templates and instantiated Mass plans.
- Use sections to make the Ordo scannable by large liturgical phase, with provisional examples including **Before Mass**, **Introductory Rites**, **Liturgy of the Word**, and **Liturgy of the Eucharist**. The complete default set is not yet adopted.
- A Mass section is structural organization only: it owns no congregation output or Presenter Cue by itself, and it is distinct from an Arrangement section inside a Song.
- Carry template sections into a new Mass plan through the existing copy-on-creation boundary. Later template section changes do not synchronize into existing plans.
- Defer the exact section taxonomy, custom naming/addition/removal, collapse behavior, item insertion target, and cross-section reordering rules to the next grilling branch.

### 171. Six-section default Mass scaffold

- Start new Mass templates with these six ordered Mass sections: **Before Mass**, **Introductory Rites**, **Liturgy of the Word**, **Liturgy of the Eucharist**, **Concluding Rites**, and **After Mass**.
- Treat **Introductory Rites**, **Liturgy of the Word**, **Liturgy of the Eucharist**, and **Concluding Rites** as the four formal structural divisions represented by the scaffold.
- Treat **Before Mass** and **After Mass** as explicit Ordo operational wrappers for parish or operator content rather than as liturgical rites.
- Keep the Communion Rite within **Liturgy of the Eucharist** instead of presenting it as a peer top-level Mass section.
- Use the six sections as the starting scaffold rather than yet declaring them locked; customization rules remain the next decision branch.

### 172. Editable and independently owned Mass-section structure

- Treat the six-section default as an editable starting scaffold rather than a locked schema.
- Let a Mass template add, rename, reorder, or remove its Mass sections so special celebrations and parish-specific structures can be modeled without bypassing the section concept.
- Copy the template's section structure into each new Mass plan. Let that plan add, rename, reorder, or remove sections independently; later template or plan edits never synchronize across the copy boundary.
- Never cascade-delete Mass items when removing a section. Require a non-empty section's items to be moved to another section or explicitly removed from the Mass before the section itself can be removed.
- Keep section changes within the existing Mass-template or Mass-plan autosave, Undo, and revision ownership rather than creating a separate shared section library.

### 173. Mass sections use local identity and editable labels only

- Give every Mass section its own stable UUID and editable display name, without a hidden permanent liturgical type or behavior-bearing taxonomy.
- Let each language-specific template use its own section labels. Renaming a section changes its label without changing its items or causing Ordo to reinterpret its meaning.
- Keep conditional inclusion, liturgical applicability, and other planning behavior on the relevant Mass items, Template slots, and template metadata rather than inferring rules from section names.
- When deriving a plan in another language, use the target-language template's section structure rather than matching, translating, or synchronizing source section names.
- Treat custom and default-derived sections uniformly after creation; the default six are a convenience scaffold, not privileged runtime entities.

### 174. Authoring visibility and non-semantic collapse state

- Show every Mass section in the Mass-template and Mass-plan editors, including empty sections, so the complete authored structure remains visible and every section remains an insertion and drag destination.
- Allow each section to collapse or expand independently and provide **Expand all** and **Collapse all** controls for the outline.
- Keep a collapsed section header informative by showing its Mass-item count and aggregated incomplete and warning counts.
- Automatically expand a section when selection, search, warning navigation, Undo, or another focused workflow reveals one of its Mass items.
- Treat expansion state as local workspace disclosure state only. It does not change Mass content, Cue order, revisions, snapshots, exports, or congregation output.
- Omit empty Mass sections from Presenter Mode while preserving every non-empty section as an outline boundary.

### 175. Section-aware Mass-item insertion

- Refine the adopted **+ Add Mass item** workflow so an item selected in the outline inserts the new item immediately after itself within the same Mass section.
- Let interaction with a Mass-section header make that section the active insertion target; adding then appends the new item within that section.
- When neither a Mass item nor section is active, including a completely item-empty plan that still owns at least one section, require selection of a destination section before the item type may be chosen. Do not silently default to the bottom of the Ordo or to After Mass.
- State the complete current destination inside the anchored Add dropdown and let the operator change it before creation. Reuse the same destination semantics for per-item **Insert before** and **Insert after** commands.
- After creation, select the new Mass item, expand its section, autosave it, and preserve the existing direct Undo behavior.

### 176. Hierarchical Mass-section and item reordering

- Give each Mass-section header a dedicated drag handle. Moving a section moves the complete section and every Mass item it contains as one ordered unit.
- With focus on a section handle in the Mass Outline, let `Alt+Up` and `Alt+Down` move that whole section by one section; retain the existing focus-scoping and text-input exclusions.
- Let a Mass item move across section boundaries by pointer or keyboard. Permit every section, including empty or collapsed sections, to be a destination; pointer hover over a collapsed destination expands it after a short delay.
- Define one keyboard boundary step explicitly: `Alt+Down` on a section's last item transfers it to the beginning of the next section, while `Alt+Up` on a section's first item transfers it to the end of the previous section. Later presses continue within the destination section.
- Make the Mass-item row's **Move up** and **Move down** actions follow the same boundary semantics rather than introducing a separate **Move to section** command in the MVP.
- After any section or item move, keep the moved entity visible and focused, autosave the operation, and make it available to the owning plan or template's Undo history.

### 177. Mass-section header controls and structural invariant

- Compose each Mass-section header from a dedicated drag handle, disclosure caret, section name, item/readiness counts, and a hover-or-focus `...` action. Right-click and `...` open the same scoped menu.
- Let the disclosure caret only expand or collapse. Let the rest of the header activate that section as the current insertion target, clear Mass-item selection, and place the selected-item editor and Inspector into a quiet **Select an item** state.
- Provide **Rename**, **Add section above**, **Add section below**, **Move up**, **Move down**, and **Remove section** in the section menu. Deliberately omit section duplication from the MVP.
- Rename inline, committing with `Enter` and cancelling with `Esc`. Require a nonblank display name unique under Unicode-aware case-insensitive comparison within its owning template or plan so insertion destinations remain unambiguous.
- Disable **Remove section** while the section contains Mass items, and disallow removing the final remaining section.
- Require every Mass template and Mass plan to contain at least one Mass section and every Mass item to belong to exactly one section.
- Autosave section creation, rename, movement, and eligible removal through the owning template or plan, with ordinary Undo and revision behavior.

### 178. Automatic Presenter handoff with focused incomplete-content acknowledgment

- Make **Present** finish pending autosave and prepare a presentation candidate from the exact current Mass-plan revision. Reuse the latest Presentation snapshot when presentation-relevant content is unchanged; otherwise persist a new immutable snapshot only after the operator actually proceeds into Presenter Mode.
- Run automatic preflight against that exact candidate. Enter Presenter Mode directly for a clean result or review-only warnings; do not impose an approval dialog or checklist on ordinary entry.
- When content is incomplete, open one centered, viewport-bounded modal above the visibly dimmed planner. Summarize affected Mass items and unavailable Cue counts, then list findings compactly in Mass-section and item context with an internally bounded scroll area.
- Make **Review and fix** the recommended, initially focused action. It closes the modal, opens the existing Readiness surface, and focuses the first incomplete finding. Let `Esc`, Close, or **Cancel** return unchanged to the planner.
- Provide **Present available content** as an explicit warning-toned secondary action. The action itself is the acknowledgment; do not add an **I understand** checkbox. Unavailable Cues remain visible and inspectable in Presenter Mode but can never be taken live.
- If the operator cancels or chooses review, do not create an unused Presentation snapshot. If snapshot capture or opening fails after proceeding, remain in the planner and show the fatal Presenter-entry blocker with recovery guidance.
- Open Presenter Mode with congregation output disabled and without creating an output window merely because **Present** was clicked.
- Treat `ordo-incomplete-preflight*` as the approved interaction and information-hierarchy reference for the incomplete-content handoff.

### 179. Safe planned exit from Presenter Mode

- Keep **Exit Presenter** visible in the Presenter global header and provide no single-key exit shortcut that could end a live presentation accidentally.
- When congregation output is disabled, exit immediately. When output is enabled, open a centered **End presentation?** confirmation while Program remains unchanged; initially focus **Stay in Presenter** and suspend single-key Presenter commands according to the adopted modal rule.
- On confirmed exit, force Program to black before closing the congregation-output window. Then mark the Presentation session complete and clear its interrupted-session recovery marker so the next launch does not reopen a deliberately ended session.
- Restore the operator window's pre-Presenter bounds and the Mass Planner's prior item/section selection, scroll positions, pane state, and section disclosure state.
- Preserve every saved Mass-local emergency insert or override. Planned exit ends the session but never reverses preparation or emergency changes.
- Keep unexpected termination under the existing interrupted-session recovery model; it is not interpreted as a planned exit and always returns congregation output fail-closed on black.

### 180. Celebration-scoped portable transfer

- This supersedes Decision 17's former single-plan **Mass Package** boundary with
  a **Celebration Package** whose scope is exactly one complete Celebration and
  every Mass plan currently under it at export time. Decision 17's reconciled
  text reflects this replacement.
- Capture or reuse one independent immutable Presentation snapshot per included Mass plan. Never combine several scheduled Masses into one Cue timeline or mutable plan.
- On the church PC, preserve the Celebration hierarchy and let the operator choose which included scheduled Mass plan to inspect, rehearse, or present.
- Include the complete offline dependency closure for all plan snapshots. Deduplicate identical assets and other immutable dependencies by stable content identity or checksum inside the package without creating cross-plan edit coupling.
- Let one returned Celebration Package carry Mass-local emergency changes or overrides originating from any of its included plans for later review on the preparation computer.
- Use **Celebration Package** as the canonical product and domain term because the artifact may contain several scheduled Mass plans. Retire **Mass Package** from current MVP language.
- Require at least one Mass plan to export a Celebration Package; an empty Celebration has no presentable snapshot to transfer.
- Defer exact import verification, local package ownership, same-Celebration replacement/version handling, included-plan selection UI, and return-import review to the next grilling branch.

### 181. Two-layer Celebration Package import review

- Import a Celebration Package through one dedicated temporary review workspace rather than compressing the package-wide workflow into a slide-over.
- Present two explicit but coordinated layers: **Exact presentation** installs the package's immutable per-plan Presentation snapshots and dependencies, while **Library reconciliation** stages editable Celebration, Mass-plan, Song, Arrangement, Theme, and related source records for the receiving parish library.
- Keep exact presentation fidelity independent of reconciliation. A library identity choice, accepted update, rejected update, or newly created local record may never rewrite the embedded Presentation snapshots.
- Classify staged library records as **New**, **Safe update**, **Unchanged**, or **Needs decision**. Resolve stable UUID identity and a remembered package-to-local identity mapping before using normalized metadata and content similarity to suggest a possible semantic match.
- Preserve incoming UUIDs for genuinely new records. When an operator deliberately maps an incoming identity to a different existing local identity or keeps it as a separate copy, retain that reviewed mapping so succeeding home-to-church imports do not repeat the same question without new evidence.
- Keep the primary import commit unavailable while consequential decisions remain unresolved. Show package verification and the included plan snapshots separately from reconciliation counts so readiness to render is not confused with readiness to modify editable library state.
- Use the reviewed interaction hierarchy: full import workspace, compact two-layer summaries, staged-result table, and a focused decision slide-over.

### 182. Revision ancestry determines safe package updates and divergence

- Never infer cross-computer freshness from a visible revision number or timestamp alone. Give each Edit revision its own stable UUID, parent revision UUID, deterministic content hash, creation time, and origin-installation UUID in addition to the durable entity UUID.
- Record the latest revision known to be shared after a reviewed import as that mapped entity's **sync anchor**. Include compact revision identifiers, hashes, and parent relationships in the Celebration Package so the receiving installation can locate a common ancestor without carrying every historical revision's complete content.
- Treat equal revision UUIDs as unchanged. Treat different revision UUIDs with equal canonical content hashes as content-equivalent rather than manufacturing a conflict.
- Permit an automatic safe update only when the incoming state changed after the common revision and the local current content remains effectively equal to that common state. Preserve a locally changed state when the incoming side did not change.
- Require review when both current states changed after their common revision, when the same entity UUID has no provable common revision, or when lineage is missing or inconsistent. The latter is an identity/integrity concern, not an ordinary fast-forward.
- Keep every displaced or rejected local branch recoverable in History. Choosing an incoming state changes the current editable state through a new/imported restorable revision; it does not erase the local branch.
- If a reviewed decision keeps an incoming record as a separate local record, generate a new local UUID and retain its source-to-local mapping for succeeding imports.

### 183. Nested read-only Song comparison for possible package matches

- Resolve each **Needs decision** row in a focused right-side slide-over above the dedicated import workspace. From a possible Song-match decision, open **Compare content** as a wider nested child slide-over spanning the application overlay layer while the parent visibly recedes.
- Compare the selected local candidate and incoming Song across **Identification**, **Lyrics**, **Sequence**, and **Slides**. Render both slide previews through the canonical slide renderer and keep corresponding slide navigation synchronized when a meaningful correspondence exists.
- Clearly label each side's Song and Arrangement identity, attribution, language, section/slide counts, and relevant structural or text differences. Allow switching among other nearest local candidates without abandoning the comparison.
- Make the nested comparison read-only. Return to the parent match decision to choose a separate Song, associate incoming Arrangement content with an existing Song work, or reconcile it with the selected identity; do not duplicate identity-changing actions inside the child.
- Preserve the parent's selected option, scroll, candidate, and focus-return target when opening and closing the child. At constrained widths, use explicit **Local** and **Incoming** tabs instead of shrinking two canonical previews below a useful size.
- Reuse the existing Song-import content-comparison component and interaction language so package reconciliation does not invent a second Song-diff system.

### 184. Verified staging and atomic managed installation

- Never present from or retain a runtime dependency on the Celebration Package file selected on removable media. Copy the selected file into local temporary staging before inspection and leave the original file untouched.
- Before opening import review, validate the package schema and supported version, manifest structure, archive paths and extraction limits, declared checksums, per-plan snapshots, required assets, redistributable fonts, and dependency closure. A package that fails verification cannot reach reconciliation or installation.
- Let **Import celebration** commit one coordinated operation: publish the exact package into Ordo-managed local storage, register its included Presentation snapshots and plans, materialize accepted library-reconciliation actions, and update identity mappings and sync anchors.
- Treat the resulting managed artifact as an **Installed Celebration Package**. The operator may remove the USB after import, and later rehearsal or presentation must remain completely offline.
- If the operator cancels verification or review, discard temporary staging and make no durable package or library change. If file publication, asset installation, database commit, or post-write verification fails, roll back the complete import rather than leaving a partial installed package or partially reconciled library.
- Clean up abandoned staging safely after interruption or restart without deleting any previously committed Installed Celebration Package.

### 185. Immutable versions under one Installed Celebration Package

- Use the Celebration UUID to group repeated exports of the same Celebration under one Installed Celebration Package. Give every export its own package-version UUID and complete artifact fingerprint; a package version is immutable after export.
- Treat a re-import whose package-version UUID and fingerprint are already installed as **Already installed** and make no duplicate package, version, snapshot, asset, or library change.
- After successful verification and library reconciliation, install a different export of the same Celebration as a new immutable package version and make it the **Current version**. Do not create a second Celebration merely because its package contents changed.
- Retain previous package versions in **Package history** with their exact plan set and per-plan snapshots. Deduplicate shared immutable assets by content checksum, and normally expose only the Current version in Masses so old packages do not appear as duplicate scheduled Celebrations.
- Permit deliberate inspection and restoration of a previous package version. Importing a version older than the Current version records it in Package history but never makes it current automatically.
- Treat a different Celebration UUID as a separate Installed Celebration Package even when date, title, metadata, or included Mass schedules appear similar; similarity may warn but does not redefine durable identity.

### 186. Three-way carry-forward of church-local changes

- When a newer Package version arrives while its ordinary local Mass plans contain saved church-local emergency changes, compare three states: the previous version's imported state as the common base, the current local plan revision, and the incoming version's corresponding plan content.
- Use stable Mass-plan, section, item, Cue, and owned-content identities to classify each church-local change. Automatically carry forward a change only when its affected incoming content remains equivalent to the base and its destination still exists.
- When home and church changed the same owned value or structure after the common base, require an explicit import decision. Choosing the church state keeps that value in the reconciled ordinary local plan; choosing the incoming state accepts the incoming value.
- Require explicit confirmation before discarding any church-local change. A discarded change remains recoverable through the previous Package version and its History until that retained version is deliberately removed under future retention rules.
- If an incoming Package version removed the owning Mass plan, Mass item, Cue, or another required destination, do not guess a new destination. Keep the unmappable change attached to the previous version, disclose why it was not carried, and preserve it for return transfer.
- Never modify either Package version's immutable snapshots. Every retained church value remains part of the ordinary local revision history, records its previous-version provenance, and remains reviewable when returned to the preparation computer.
- Make the incoming Package version Current only after every overlapping or unmappable change has been resolved. Non-overlapping carry-forward remains automatic-first and appears in the staged import summary rather than demanding ceremonial approval.

### 187. Current package Celebrations live in the ordinary Masses chronology

- After a successful import, navigate to **Masses** and expose the Current Package version as one ordinary Celebration group in **Upcoming** or **History** according to the actual scheduled starts of its included Mass plans.
- Do not create a parallel package-only Mass browser or show separate package and editable copies of the same Celebration. The reconciled editable Celebration and its attached immutable Package versions share one visible Celebration group keyed by the Celebration UUID.
- Mark the Celebration header with a restrained **Installed package** provenance badge and make source installation, package version, and import time available without dominating the liturgical title and schedule.
- Render every included Mass plan as the established chronological plan row with actual date/time, Anticipated status, primary language, Choir, readiness, and a visible church-local-change indicator when applicable.
- The nearest upcoming scheduled plan may receive quiet emphasis, but import never opens a plan, starts rehearsal, or enters Presenter Mode automatically. Plan selection remains explicit.
- Keep ordinary plan selection and presentation in **Masses**. Reserve **Transfer & Backup** for package verification details, Package history, restoration, removal, return export, and other artifact administration.

### 188. Imported content is ordinary local content

- Once Celebration Package import commits, its Celebration, Mass plans, Songs, Arrangements, Themes, and other accepted source records are ordinary editable local records. Do not introduce an imported-plan mode, package-owned working copy, read-only package Celebration, or parallel runtime identity.
- Treat the retained Package version, artifact fingerprint, imported Edit revisions, and immutable Presentation snapshots as provenance, history, rollback, and later transfer evidence—not as an alternative set of plans that the operator must choose between.
- Selecting an imported Mass plan opens the same Mass Planner as any locally created plan. Its ordinary autosave, Undo, revision History, readiness, editing, rehearsal, and Presenter behavior apply without package-specific interaction branches.
- Keep exactly one **Present** action. If the local plan still matches an imported Presentation snapshot, reuse it under the existing snapshot rule; if the plan changed, capture or reuse a newer snapshot automatically. Never ask the operator to choose between **Present package** and **Present editable plan**.
- Treat package provenance as optional context in details and History rather than a behavioral status. Re-import compares the incoming lineage against the current ordinary local records through the adopted reconciliation model.

### 189. Symmetric Celebration Package round trip

- Use the same **Celebration Package** contract and export/import workflow in both directions. Do not create a separate Return Package type or a church-only synchronization mode.
- Exporting from the church keeps the durable Celebration UUID, creates a new immutable Package-version UUID and fingerprint, and records the exporting installation plus revision ancestry back to the last shared state.
- Every export remains complete and self-contained: include the Celebration's current Mass plans, one captured-or-reused Presentation snapshot per plan, and the complete current dependency closure used by those plans. Never require the receiving installation to possess an earlier Package version in order to apply a delta.
- On the preparation computer, import through the same verification and reconciliation workflow. Ignore equivalent content, accept church-only changes as safe updates, preserve home-only changes, and require review when both installations changed the same owned content after their common sync anchor.
- After further ordinary editing at home, export another complete Celebration Package and import it at church through the same rules. This round trip may repeat without changing artifact types or turning imported records into package-owned content.
- A change summary such as **7 changes since the version received from Home PC** is explanatory provenance only; it does not alter the contents or semantics of the exported artifact.

### 190. Reuse a content-equivalent Package version on export

- Allow Celebration Package export even when the Celebration's current export scope is content-equivalent to the latest retained Package version. Do not make the absence of changes an error or disable portable copying.
- Determine equivalence from the Celebration, all of its current Mass plans, and the complete dependencies reachable from those plans. Unrelated parish-library edits do not invalidate reuse.
- When equivalent, disclose **No changes since Package version …** and copy the exact retained immutable artifact to the chosen destination. Preserve its Package-version UUID and fingerprint rather than manufacturing a new version or regenerating equivalent files.
- Let the receiving installation recognize that exact version as **Already imported** and perform no duplicate package, snapshot, asset, revision, or library mutation.
- Create a new Package-version UUID and artifact fingerprint only when relevant exported content changed. Apply the existing captured-or-reused Presentation-snapshot rule independently to each included Mass plan.

### 191. Contextual Celebration Package export slide-over

- Expose **Export Celebration Package** in each Celebration group's contextual `…` menu in **Masses**. Also expose **Export celebration** in **Transfer & Backup**, where the operator first selects a Celebration. Both entry points open the same export workflow.
- Use the adopted viewport-spanning floating right-side slide-over rather than a centered modal or dedicated full workspace. Import keeps its dedicated review workspace because reconciliation is substantially more complex than export.
- Identify the Celebration by title and date, then summarize either **New version — N changes** or **No changes — existing version will be copied**. Show every included Mass plan grouped chronologically, readiness findings, fatal blockers, estimated package size, and the selected destination.
- Keep the included plan list informational. Do not add plan checkboxes or partial-package controls: the artifact boundary remains the complete Celebration and every current Mass plan.
- Place detailed snapshots and dependencies under a collapsed **Package contents** disclosure so routine export remains scannable without hiding evidence needed for troubleshooting.
- Keep **Export package** as the persistent bottom primary action. Invoke the native file/folder chooser as part of that action, then publish the verified artifact atomically to the selected destination.
- Keep routine export close to the owning Celebration in Masses while **Transfer & Backup** remains the administrative home for Package history, verification details, restoration, removal, and global export/import entry points.

### 192. Export integrity blocks; incomplete preparation may travel

- Treat Celebration Package export as a portability operation, not an assertion that every included Mass plan is presentation-complete. Allow incomplete content to travel so preparation can continue on another installation.
- Show unfilled Song or Reading slots, pagination/readability concerns, absent optional attribution or source metadata, never-presented plans, and ordinary Mass readiness findings as per-plan warnings. Keep **Export package** available without an additional approval dialog or checkbox.
- Block export only when Ordo cannot produce a trustworthy self-contained artifact: the Celebration has no Mass plans; a required asset or bundled font is missing or corrupt; a required Presentation snapshot cannot be captured or validated; dependency closure is incomplete; durable identities or revision lineage are inconsistent; or the destination cannot be published safely.
- Summarize preflight as **Ready**, **Ready with warnings**, or **Cannot export**. Let actionable findings navigate to their owning content or expose a direct repair/retry action when navigation is not applicable.
- Re-run affected checks after a repair and before final publication. Do not weaken manifest, checksum, atomic-write, or post-write verification merely because the only visible content findings were warnings.

### 193. Universal `.ordo` exchange container

- Use `.ordo` as the single native extension for Ordo-compatible portable artifacts, including Celebration Packages, Parish Backups, Song imports, and Liturgy imports. Determine the artifact's meaning from its verified internal manifest rather than its filename or extension alone.
- Define an **Ordo file** as a standard ZIP container with a required root `manifest.json`, a structured JSON payload, and an optional `assets/` tree. JSON remains the readable serialization format; do not invent a proprietary binary serialization, obfuscation layer, or encryption merely to make the file appear native.
- Require the manifest to identify the Ordo container format, artifact kind, schema version, integrity metadata, and the payload/assets it owns. Initial artifact kinds include `celebration-package`, `parish-backup`, `song-import`, and `liturgy-import`.
- Treat `.ordo` as an exchange-container claim, not a trust claim. AI tools may create Song and Liturgy import files, so every artifact kind receives path-safety checks, extraction limits, schema validation, integrity verification, and kind-specific review before mutation.
- Publish the relevant JSON Schemas and packaging contract so AI agents and other offline tooling can create valid Song and Liturgy `.ordo` files with ordinary ZIP and JSON tooling.
- Centralize inspection, validation routing, safe extraction, manifest/version handling, and creation behind one deep **Artifact Container Module**. Import screens consume its small typed result interface instead of implementing ZIP, schema, and safety rules independently.
- Associate `.ordo` with Ordo on supported desktops and route double-clicked files to the review flow declared by their verified artifact kind. Keep emergency decks as `.pptx`; plain authoring/debug fixtures may remain JSON without becoming end-user import artifacts.

### 194. Artifact-specific `.ordo` filenames and safe replacement

- Generate human-readable default `.ordo` filenames according to artifact kind rather than encoding technical manifest data into every name. Examples include `2026-08-23_21st-Sunday-in-Ordinary-Time.ordo`, `St-Joseph-Parish_Backup_2026-08-24_1930.ordo`, `Amahan-Namo_Walter-Arrangement.ordo`, and `2026-08-23_Cebuano-Readings.ordo`.
- Sanitize generated names for supported Windows, Linux, and common removable-drive filesystems. Keep durable identity, artifact kind, schema version, and immutable Package-version identity inside the verified manifest; never infer them from the visible filename.
- For repeated Celebration exports, propose the same date-and-title filename so the current file remains easy to locate on removable media. Do not append UUIDs, revision numbers, or timestamps by default.
- When the destination already exists, rely on the native chooser's explicit replacement confirmation and let the operator rename the new file when both copies are desired.
- Build and verify a temporary sibling artifact before replacing the destination. Publish the replacement atomically where the destination filesystem permits and otherwise use a recoverable replacement sequence; any failure must leave the previous destination file intact or restored.
- A content-equivalent export copies the exact retained immutable Package artifact under the chosen filename; it does not regenerate payload bytes or create another Package version.

### 195. Commit Package history only after verified destination publication

- Treat a newly changed Celebration Package export as a recoverable staged transaction. Build the candidate Package version in managed temporary staging, validate it locally, publish it to the selected destination, read it back and verify it, then commit that Package version to Package history as **Exported**.
- Do not expose a staged candidate as Current or as an ordinary Package-history version before destination verification succeeds. Cancellation, removable-drive disconnection, insufficient storage, write failure, or failed read-back verification leaves no new Package version in History.
- Preserve or restore any previous destination file according to the adopted safe-replacement rule. Never leave a partial `.ordo` under the final filename.
- Journal the narrow interval between successful destination verification and local History commit. After restart, compare the destination artifact fingerprint with the staged candidate and either finalize the matching History entry or discard the abandoned candidate without guessing.
- Clean abandoned staging immediately when safe or during startup recovery. Retain current parish edits and previously committed Package versions regardless of export failure.
- Never advance a sync anchor merely because an artifact was exported. Only a successful reviewed import proves that two installations share the imported Edit revision.

### 196. Persistent verified-export completion state

- After successful destination read-back verification, keep the export slide-over open in a dedicated **Export complete** state rather than relying on a temporary toast or closing automatically.
- Show the filename, destination drive/path, size, Package version with an abbreviated fingerprint, **Verified** status, included Mass-plan count, and whether Ordo exported a new Package version or copied an existing content-equivalent version.
- Provide **Done** as the primary action, **Show in folder** as a secondary action, and **Copy to another location** as a reuse action that republishes the already verified artifact without rebuilding or creating another Package version.
- When the destination is removable, close all Ordo file handles and instruct the operator to use the operating system's Eject command before removal. Do not claim that closing the Ordo write is equivalent to a complete operating-system unmount or cache flush.
- Defer a custom cross-platform **Eject drive** action beyond the MVP. It is not required for artifact correctness and must not be approximated with an unreliable implementation.

### 197. Complete Parish-scope Backup without installation cloning

- Keep the MVP **Parish Backup** complete and non-selective. Include the parish identity and settings; every Celebration, Mass plan, template, Mass-local change, Song, Arrangement, Choir, Theme, and custom asset; durable Edit revisions and restorable History; Presentation snapshots; Package versions and provenance; identity mappings, sync anchors, import records, and other authoritative state needed to preserve offline round-trip ancestry.
- Deduplicate repeated immutable assets by content identity inside the Backup without introducing external dependencies. A Parish Backup must remain self-contained and restore without internet access or access to the source installation.
- Exclude application binaries and bundled application resources, regenerable thumbnails/caches, temporary or staged files, diagnostic logs, and active Presentation-session recovery state.
- Preserve recognized-device display settings only as dormant source-device metadata. Activate them after restore only when the corresponding hardware is recognized; otherwise reset Output Fit safely and require calibration.
- Never restore the source computer's installation UUID onto the destination. A restore recreates the Parish scope while the destination remains its own installation, preserving meaningful revision ancestry and future synchronization evidence.
- Defer selective and incremental backup formats beyond the MVP. They are not variants of a Parish Backup in the current product language.

### 198. Manual portable Backup, automatic safety Backup, and reminder

- Let the operator create a portable Parish Backup explicitly through **Transfer & Backup → Create parish backup**, choose its destination, and receive a read-back-verified `.ordo` file.
- Automatically create a locally managed **safety Backup** before a schema migration, an application upgrade that changes stored data, or a Parish Backup restore. Do not require ceremonial approval for this protective prerequisite.
- Do not treat a locally managed safety Backup as evidence that parish data exists off the computer. It never clears the portable-backup reminder.
- When authoritative Parish-scope content changed and no portable Backup has been successfully verified for seven days, show a non-blocking reminder in Transfer & Backup and the application header. Suppress it when nothing changed since the last portable Backup.
- Present the last successful portable-backup time and the number of authoritative changes since it. Clear the reminder only after successful destination read-back verification.
- Do not schedule writes to absent removable drives or attempt backups while Ordo is not running in the MVP. A missing USB device must not create repeated background errors or alerts.

### 199. Reuse verified artifact-publication UX for Parish Backup creation

- Place **Create parish backup** in a **Protect this parish** section of **Transfer & Backup**, alongside last portable Backup, changes since that Backup, and automatic safety-Backup status.
- Open the adopted viewport-spanning floating right-side slide-over. Show the parish name, complete-scope counts for authoritative entity families, estimated size, integrity-check status, destination, and a collapsed **Backup contents** disclosure.
- Do not offer content checkboxes or exclude controls. A Parish Backup remains complete and non-selective.
- Omit Mass readiness and presentation-completeness findings. Block only when Ordo cannot capture a transactionally consistent Parish scope, read a required authoritative asset, produce a valid manifest and dependency closure, or publish the destination safely.
- Reuse the Celebration Package export progress, temporary-sibling publication, read-back verification, persistent completion state, **Show in folder**, **Copy to another location**, and operating-system eject guidance.
- Keep **Create backup** as the persistent primary action. Clear the portable-backup reminder only after the destination artifact passes read-back verification.

### 200. Replace-only restore workspace with an exact protection gate

- Run **Restore parish backup** as a dedicated full-page workspace rather than a slide-over. Block entry while Presenter Mode or congregation output is active.
- Copy the selected `.ordo` to staging and verify its artifact kind, schema compatibility, checksums, asset closure, and stored-data integrity before exposing any destructive action or mutating current parish data.
- Compare **Current parish** and **Backup** side by side using parish name/UUID, data size, authoritative entity counts, latest activity versus Backup creation time, current installation, and source installation. Prominently warn when Parish UUIDs differ, while still permitting an intentional machine migration or parish replacement.
- State plainly that restore replaces the complete current Parish scope and never merges it. Use a centered final confirmation with **Keep current parish** initially focused and **Restore parish** as the destructive action; do not require typing the parish name.
- Keep **Restore parish** disabled until Ordo proves the current Parish scope is recoverable. Reverify an accessible Backup whose content fingerprint exactly matches the current authoritative state, or automatically create and verify a fresh locally managed safety Backup when any content differs.
- Do not accept age or a historical record of an unavailable portable Backup as protection evidence. A Backup must be currently accessible, content-equivalent, and pass verification immediately before replacement.
- If protection capture or verification fails, block restore. After protection succeeds, show **Current parish protected** with the Backup time and location.
- Restore into a new managed location, verify it completely, atomically switch Ordo to it, and restart into the restored Parish scope. Preserve the pre-restore safety Backup as an ordinary verified Parish Backup; returning to that state uses the same Restore workflow as any other Backup.
- Do not introduce **Undo restore**, a restore-reversal mode, parallel restored branches, or a special recovery artifact. A Backup is a Backup.

### 201. Safety Backups use ordinary manual retention in the MVP

- Keep automatic pre-migration, pre-upgrade, and pre-restore safety Backups as ordinary verified Parish Backups in **Transfer & Backup**.
- Do not create a separate recovery-point type, automatic retention schedule, or **Undo restore** interaction. Restoring a safety Backup uses the same replace-only Parish Backup workflow.
- Retain safety Backups until the operator deliberately deletes them. Show creation time, reason, size, and the consequence that deletion removes that recovery copy before confirming permanent removal.
- Defer age-based cleanup, count-based rotation, and configurable retention beyond the MVP. These operations are rare enough that manual cleanup is sufficient for initial deployment.

### 202. Installation-specific Displays settings page

- Provide one **Settings → Displays** page with a live topology diagram matching the operating system's actual left/right/top/bottom display arrangement and one detected-display tile showing number, name, resolution, aspect ratio, position, and recognition status.
- Mark the current operator display explicitly. Let the congregation display be one selected detected display or **Not assigned**, and save that choice immediately for the installation.
- Keep **Identify displays** as the primary diagnostic action and reuse the same display-picker interaction inside Presenter preflight.
- For the selected congregation display, show a **Calibration** section containing current Output Fit, the adopted `0–10%` uniform slider, canonical fitted Preview, **Show calibration pattern**, and the last calibration-check time.
- Never expose Mass content from Settings. Calibration opens the assigned output black first, shows only the calibration frame after explicit action, and returns to black before closing.
- A disconnected selected display becomes **Unavailable** without silent reassignment. With only one display, keep congregation display **Not assigned** while Presenter Mode remains available with congregation output disabled.

### 203. Same-window native-full-screen Presenter Mode

- This supersedes Decision 10's former maximized/non-exclusive Presenter
  behavior. Entering **Presenter Mode** transforms the existing main operator
  window into native full-screen **Live Control** on the operator display; it
  does not create a dedicated third operating-system window or leave a hidden
  preparation window running alongside it.
- Before transformation, retain the operator window's bounds, maximized/full-screen state, current route, Mass Planner selection, scroll positions, pane state, and section disclosure state. Preparation UI is replaced within the same window while its state remains restorable.
- Keep **Live Control** as the canonical operator-facing screen inside Presenter Mode, not a separate mode or window. Render its confirmations, readiness surfaces, and operational slide-overs within the operator window rather than creating additional operating-system windows.
- The only possible second Ordo window is the passive borderless full-screen congregation-output window on the assigned display. Do not create it merely by entering Presenter Mode; create it only when the operator explicitly enables congregation output.
- On planned exit, force Program black when output exists, close the congregation window, leave native full-screen, restore the exact preparation workspace and previous operator-window state, and preserve saved emergency changes.
- When output is disabled or no second display exists, Presenter Mode consists solely of the one full-screen operator window. Display disconnection closes or invalidates congregation output without disturbing the operator window.
- Keep **Exit Presenter** visibly available and retain the adopted guarded-exit interaction. Native full-screen changes window presentation, not command semantics, crash recovery, or fail-closed Program behavior.

### 204. Offline read-only Diagnostics surface

- Provide one **Settings → Diagnostics** page that works completely offline and summarizes installation health as **Healthy** or **Needs attention** without requiring telemetry, an account, or internet access.
- Report the Ordo application version and build; operating system, CPU, memory, GPU, and rendering environment; database and schema version with integrity status; managed-asset availability and corruption findings; bundled Atkinson and Noto font availability; storage location and free space; congregation-display assignment and calibration state; latest verified Parish Backup and Celebration Package activity; and evidence of a recent crash or interrupted Presentation session.
- Make **Run diagnostics** a fresh, non-mutating inspection. The Diagnostics page must not silently repair, rewrite, migrate, prune, or otherwise alter parish data.
- Keep findings operator-readable and grouped by owning concern. Where an ordinary corrective workflow exists, link the finding to its proper owner such as **Displays**, **Transfer & Backup**, or the affected library entry rather than duplicating repair controls inside Diagnostics.
- Keep Diagnostics operator-only. Running a check must never open congregation output or expose diagnostic content on Program.
- Centralize collection and normalization behind a deep **Diagnostics Module** whose small result interface supplies the Settings page and any later diagnostic-report exporter. Individual UI panels must not inspect storage, assets, fonts, displays, or runtime internals independently.

### 205. Deliberate sanitized Diagnostic-report export

- Add `diagnostic-report` as an explicitly supported `.ordo` artifact kind. **Export diagnostic report** creates it locally for deliberate file or removable-drive transfer; Ordo never uploads or sends it automatically and requires neither internet access nor an account.
- Include the normalized diagnostic summary and check results, application/operating-system/hardware environment, sanitized recent Ordo logs, error codes, timestamps, and anonymous entity identifiers needed to correlate a finding without exposing the entity's content.
- Exclude parish name, Song titles, lyrics, readings, notes, media, usernames, hardware serial numbers, complete filesystem paths, and file bodies. Render necessary paths with generic roots such as `[Ordo data]/assets/...` rather than leaking a user's profile or volume details.
- Provide **Copy summary** for a short operator-readable text summary that follows the same privacy boundary. It is a local clipboard action, not a network-sharing flow.
- Route creation through the adopted Artifact Container Module and obtain all diagnostic content through the Diagnostics Module's normalized result. The report exporter owns packaging and publication but must not independently recollect unsanitized runtime or parish data.
- Apply the universal Ordo-file safety and integrity contract to the report: `manifest.json`, declared kind and schema version, structured JSON payload, optional sanitized log assets, bounded extraction, checksums, and validation. Opening a Diagnostic report is read-only and can never mutate parish state.

### 206. Latest-result plus explicit full Diagnostic scan lifecycle

- Show the latest successfully completed full Diagnostic result immediately, together with the exact completion time, rather than treating stale or partial work as current health evidence.
- On page entry, refresh only inexpensive live facts such as application identity, free storage, and current display connection. Do not automatically run database integrity, complete asset hashing, font validation, or renderer qualification merely because Settings opened or Ordo started.
- Make **Run full diagnostics** an explicit, asynchronous operation with visible overall and per-section progress. It must leave the rest of the operator interface responsive and must never run automatically during Presenter Mode.
- Allow cancellation without changing parish data or erasing the previous completed result. A cancelled, interrupted, or failed run is labeled as an incomplete attempt and never replaces the latest completed overall **Healthy** or **Needs attention** assessment.
- Publish the new overall assessment and persist its normalized result only after the complete requested scan finishes. Individual in-progress checks may show provisional progress but must not masquerade as the current completed result.
- Keep the scan lifecycle behind the Diagnostics Module's interface so the Settings page observes state and issues start/cancel commands without orchestrating database, asset, font, display, or renderer checks itself.

### 207. Issue-first single-column Diagnostics hierarchy

- Present **Settings → Diagnostics** as an ordinary full Settings page, not a slide-over and not a dashboard-like mosaic of cards.
- Keep the page header compact: **Diagnostics**, the latest completed time, and **Run full diagnostics**. While scanning, replace that action with **Cancel** and place one thin overall progress indicator directly beneath the header.
- Lead the page body with a restrained full-width **Healthy** or **Needs attention** strip and a plain-language summary. When findings need action, place **Issues requiring attention** immediately below it; each issue explains the consequence and links to its owning workflow.
- Organize the remaining detail as collapsible bordered rows for **Data & assets**, **Fonts & rendering**, **Displays**, **Transfer & backups**, **Application & system**, and **Recent problems**. Every collapsed row exposes its status and finding count without requiring expansion.
- Place **Copy summary** and **Export diagnostic report** together after the diagnostic detail as secondary troubleshooting actions rather than competing with the primary scan command.
- Retain one readable column at `1366x768`. Wider layouts may increase usable detail within a row but must not convert the page into a generic multi-card dashboard.

### 208. Warm-neutral and graphite semantic color language

- Use the approved warm-paper preparation appearance and neutral graphite Presenter appearance as two coordinated applications of the same semantic token system.
- Use restrained forest green for primary actions and selection. Use a brighter but controlled green for **Live**, amber for **Freeze** and warning/incomplete conditions, near-black with a clear light label and border for **Blackout**, muted red for error/destructive conditions, and restrained blue for neutral information.
- Never communicate an operational state through color alone. Pair every Live, Freeze, Blackout, warning, unavailable/error, and informational treatment with explicit text plus a stable icon or shape cue.
- Reserve liturgical purple, red, green, rose, and white for places that actually represent liturgical color or a Slide Theme. Do not reuse those domain colors as interchangeable application-brand or operational-state accents.
- Treat the approved `design-references/ordo-semantic-palette*` exploration as the visual reference for the relationship among these semantic roles. Production token values may receive small contrast-qualified adjustments while preserving that visual language.

### 209. Google Labs-compatible DESIGN.md as the visual source of truth

- Maintain root-level `DESIGN.md` using the Google Labs `design.md` alpha format: normative machine-readable YAML design tokens followed by human-readable Markdown rationale and usage guidance.
- Make `DESIGN.md` the canonical contract for reusable visual identity, appearances, tokens, layout language, elevation, shapes, component styling, and visual do/don't guidance. Keep domain vocabulary in `CONTEXT.md`, historical rationale in `decision.md`, and product behavior plus implementation scope in `MVP-PLAN.md`.
- Keep `design-references/` as approved interactive evidence rather than production source or a competing specification. Link references from `DESIGN.md`; when a visual reference conflicts with the written contracts, the written contract wins.
- Use the format's canonical sections first and add Ordo-specific sections only where necessary. Unknown-section extensibility permits future iconography, interaction-state, motion, accessibility, responsive-qualification, Presenter-safety, and reference guidance without abandoning the format.
- Do not invent normative token values for unresolved branches. Declare intentional omissions in the YAML front matter and fill them as focus, geometry, iconography, spacing/density, and motion decisions are adopted.
- Treat the external format and CLI as alpha. When implementation begins, pin an exact `@google/design.md` version, run lint and contrast validation through the cross-platform `designmd` command, and review format upgrades deliberately rather than tracking latest automatically.

### 210. Distinct disabled, unavailable, read-only, and not-assigned semantics

- Use **Disabled** only for an action or input that exists but cannot currently execute. Keep its styling neutrally subdued; do not use warning or danger color merely because the control is disabled.
- Use an ordinary native disabled control when its reason is obvious from local context, such as **Save** when nothing changed. When the reason is operationally consequential, such as a blocked Parish restore, keep the control keyboard-focusable, expose its unavailable state through AccessKit, suppress activation, and associate it with a persistent nearby explanation that assistive technology can announce.
- Never rely on a hover-only tooltip to explain a consequential disabled action. A pointer tooltip may repeat the reason but cannot be its sole presentation.
- Use **Unavailable** for content or a capability that cannot currently perform its domain purpose, such as a Cue with a missing dependency or a disconnected saved display. Keep it visible and sufficiently legible, label it **Unavailable** with an icon and short reason, preserve inspection and corrective navigation, and prevent actions that could expose or commit the invalid content.
- Use **Read-only** for intentionally inspectable content that cannot be edited in the current context. Prefer omitting editing controls and stating the read-only context over rendering an entire editable form as disabled.
- Treat **Not assigned** as a valid configuration value, particularly for congregation-display selection. It is neither an error nor the same state as a previously assigned display becoming Unavailable.
- Keep these meanings consistent in Ordo Modules and primitive adapters. A component-library `disabled` prop must not erase Ordo's distinction between action state, content capability, inspection mode, and valid empty assignment.

### 211. Dedicated two-layer visible keyboard focus

- Show Ordo's custom keyboard-focus treatment through `:focus-visible` so keyboard navigation is unmistakable without leaving persistent rings after ordinary pointer activation.
- Use a consistent two-layer focus treatment in both appearances: a `2px` separation ring matching the immediate surface followed by a `2px` blue outer ring. Adopt `#2F6FA3` as `focus-preparation` and `#8FCBFF` as `focus-presenter`; keep the geometry identical while adapting contrast to the warm-paper and graphite surfaces.
- Keep focus independent from selection and operational state. Selection remains a forest-green tinted fill; focus remains the blue outer outline, and both may coexist on the same row, Cue, or control. Never reuse Live green, Freeze/warning amber, Blackout, danger red, or liturgical colors as focus.
- Apply the treatment to the outer wrapper of canonical slide thumbnails and frames. The outline may not enter, resize, crop, or otherwise alter the canonical `16:9` composition.
- Never suppress the browser or operating system's native outline unless the complete Ordo equivalent is present. In Windows high-contrast and other forced-colors environments, allow the system highlight treatment to take precedence.
- Implement focus centrally through the Ordo design-system Module and source-owned primitive seam rather than styling each workspace independently. Qualify the result in dense lists, menus, dialogs, nested slide-overs, editor rows, the Cue timeline, Presenter controls, both supported desktop sizes, and increased text scaling.

### 212. Open separator and border hierarchy

- Use `1px` neutral borders for ordinary pane boundaries, list-row separators, inputs, and structural divisions in both appearances. Prefer shared dividers and restrained surface changes over enclosing every section or row in a separate box.
- Keep adjacent surfaces on one shared edge so their borders never double into an unintended heavier line. Lists generally use row separators; panes use one boundary divider; section hierarchy may use a subtle tonal band without requiring a complete container border.
- Retain the existing `preparation-border` and `presenter-border` tokens for subtle structure. Add `preparation-border-strong: #8E8476` and `presenter-border-strong: #7A857B` for edges whose own visibility is necessary, including otherwise ambiguous controls, canonical frame boundaries, active drop targets, and temporary overlay boundaries.
- Do not use the strong treatment to box every group or simulate a card grid. Ordinary content remains visually part of the workspace.
- Keep selection as a forest-green tinted fill rather than a competing green outline. Keep the adopted blue two-layer keyboard focus independent of borders and selection.
- Continue reserving shadows for temporary elevated ownership such as menus, dialogs, slide-overs, and nested focused children. Major fixed panes and ordinary rows do not receive card shadows.

### 213. Restrained radius system with rounded operator slide viewports

- Adopt exactly two ordinary corner radii: `4px` for controls, interactive-row treatments, compact elements, and slide-viewport chrome; `8px` for menus, dialogs, slide-overs, and other temporary elevated surfaces. Fixed panes, shared dividers, tables, and ordinary open list structures remain square.
- Reserve fully rounded shapes for genuine circles, status dots, and compact status labels whose semantics warrant them. Do not render ordinary buttons, containers, or rows as pills.
- Keep the canonical Slide Renderer square-cornered at `1920x1080`, but display its result through consistently clipped `4px` operator viewports in Cue thumbnails, generated-frame navigation, Inspector, Preview, and Program. Preview and Program use identical viewport geometry.
- Keep congregation output square and edge-to-edge on the physical display. The operator viewport radius is application chrome; it never becomes Slide Theme content, saved slide content, or exported output.
- The small corner mask is compatible with the adopted safe-text margin but may not influence content layout, scale, pagination, or canonical `16:9` geometry. Keyboard focus follows the rounded outer wrapper without entering the rendered canvas.

### 214. Two-level elevation for temporary ownership

- Use exactly two shadow levels in each appearance. **Floating** applies to menus, popovers, dropdowns, and tooltips; **Overlay** applies to dialogs, slide-overs, and the focused child in a nested slide-over sequence.
- Adopt restrained appearance-specific tokens: `0 4px 12px rgba(39, 35, 31, 0.14)` and `0 18px 48px rgba(39, 35, 31, 0.20)` in Preparation; `0 4px 14px rgba(0, 0, 0, 0.40)` and `0 18px 52px rgba(0, 0, 0, 0.58)` in Presenter. These may receive small visual-qualification adjustments without creating additional semantic levels.
- Retain the appropriate neutral border on every elevated surface so its edge remains perceptible without relying on shadow alone.
- Keep fixed panes, rows, buttons, selected items, and canonical slide frames flat. Selection, hover, and ordinary interactivity do not create elevation.
- Combine a nested child's single Overlay shadow with the adopted visual recession of its parent. Never accumulate successively stronger or multiple stacked shadows merely because nesting is deeper.
- Keep all shadows neutral. Presenter uses deeper neutral black for visibility against graphite; neither appearance uses colored glows or shadows for Live, Freeze, Blackout, warning, danger, focus, selection, or liturgical meaning.

### 215. Lucide as the single bundled operator icon family

- Use Lucide as Ordo's sole general operator-interface icon family. Bundle the selected SVG modules locally, import only used icons, pin the dependency with the rest of the implementation toolchain, and preserve the applicable ISC/MIT notices in distributed third-party notices.
- Use `16px` icons inside ordinary controls and `20px` icons for navigation and prominent actions, with a consistent `2px` stroke across Preparation and Presenter appearances.
- Pair icons with visible labels for high-stakes or unfamiliar actions, explicitly including Live, Freeze, Blackout, Present, Restore, Delete, and congregation-display control. An icon never replaces the stable wording of an operational command.
- Limit icon-only controls to familiar compact actions such as Close, More, disclosure chevrons, and drag handles. Give every icon-only action an accessible name and a pointer tooltip that repeats the same name without becoming the sole accessible label.
- Keep ordinary icons outlined. Reserve filled shapes for status markers where the shape helps communicate non-color state.
- Allow a custom Ordo SVG only when Lucide cannot clearly express a domain-specific concept. Custom icons must follow Lucide's grid, stroke, size, and accessibility language; never mix Material, Font Awesome, emoji, or another general icon family into the interface.

### 216. Four-pixel spacing scale with resolution-aware density

- Use a `4px` base spacing scale with named values `4`, `8`, `12`, `16`, `24`, and `32px`. Use `8px` between closely related elements, `16px` between groups, and `24px` between major sections; avoid arbitrary one-off gaps.
- Make default controls and ordinary list rows `36px` high. Allow compact icon controls at `32x32px`, and keep high-stakes Presenter controls at least `40px` high.
- Use `16px` workspace gutters at the compact `1366x768` qualification size and `24px` gutters at `1920x1080`. Overlays continue using their adopted viewport-bound exterior gutters and internal scrolling behavior.
- Use additional desktop area for wider canonical previews, more slide columns, and more visible context rather than proportionally scaling the entire interface. Preserve the established responsive disclosure and optional-rail behavior.
- Let increased interface text scaling grow rows and reflow or disclose secondary information. Never clip labels, overlap controls, or shrink essential text merely to preserve nominal row height.
- Do not provide a user-facing Compact/Comfortable density preference in the MVP. One qualified adaptive density system is sufficient.

### 217. Restrained functional motion with immediate presentation output

> **Superseded in part by Decision 256.** Overlay timing is now `400ms` in and
> `200ms` out, and Blackout/Hold boundaries use reversible `500ms` fades.
> Content-to-content cuts and the remaining restraint rules below stay in force;
> the body is reconciled to those boundaries.

- Use `120ms` for hover, pressed, focus-adjacent, and color transitions; `160ms` for menus, popovers, disclosures, and visible row reordering; and the adopted `400ms` entry/`200ms` exit timing for dialogs, slide-overs, nested-slide navigation, and parent recession.
- Use one calm ease-out curve, `cubic-bezier(0.2, 0, 0, 1)`. Do not use spring, bounce, overshoot, decorative looping, or attention-seeking animation.
- Keep navigation and interface transitions interruptible; animation may never delay accepting another valid command or changing focus ownership.
- Apply presentation-critical state changes immediately and never wait for operator-chrome animation. Preview/Program content-to-content changes, Freeze, and Live restoration use `0ms` output motion; only content-to/from Blackout or Hold uses the reversible `500ms` opacity fade adopted in Decision 256.
- Under reduced-motion preference, remove translation, scaling, animated reordering, and nested-parent recession. Limit clarifying opacity, including Blackout/Hold boundaries, to at most `100ms` while retaining immediate state changes.
- Preserve identical interaction order, focus movement, state change, and final layout with or without motion. Reduced motion changes only the visual path between states.

### 218. Minimal first-run Parish setup with regular-Presider defaults

- On an installation with no Parish scope, show one focused **Set up Ordo** page with two paths: **Start a new parish** and **Restore a parish backup**. Do not use a tutorial carousel or generic onboarding dashboard.
- Starting a new Parish asks for a required Parish name, optional Default Mass location, and optional Regular Presiders. If exactly one Regular Presider is entered, select that person as the Default Presider automatically; with several, let the operator explicitly choose one default or leave the default unassigned.
- Keep Regular Presiders a lightweight Parish-default suggestion list rather than a clergy directory. New Mass plans copy the Default Presider, while every Presider field remains an editable combobox that prioritizes Regular Presiders, shows recent guest names separately, and accepts an immediate custom value for a visiting priest or bishop.
- Creating the Parish generates its Parish UUID and opens the empty Masses workspace. Do not request an account, internet connection, telemetry consent, content language, Choir setup, Theme choice, display assignment, or other optional configuration during first run.
- Initial restore enters the existing full-page restore review, verifies the Backup, and adopts the Backup's Parish identity. Because no current Parish scope exists, do not create a meaningless pre-restore safety Backup; all later restores retain the adopted recoverability requirement.
- Do not provide **Skip setup** because Ordo requires a Parish identity. Keep setup to one meaningful decision surface and allow optional fields to remain blank.
- After new-Parish creation, the empty Masses workspace offers **Create first Celebration**, **Import `.ordo`**, and **Create or manage templates** as practical next actions without forcing any one of them.

### 219. Empty Song Library within the permanent master-detail workspace

- Preserve the ordinary Song Library master-detail shell when the Parish has no Songs. Do not replace the real workspace with a temporary onboarding page, oversized illustration, tutorial carousel, or centered welcome dashboard.
- In the empty results pane, show **No songs yet** with a concise explanation, **Create song** as the primary action, and **Import `.ordo`** as the secondary action.
- Use the empty detail pane to explain the real authoring path in three concise steps: create or import a Song, add an Arrangement with lyrics and Sequence, then associate Choirs and usage metadata as appropriate. Keep this guidance subordinate to the creation actions.
- Distinguish an empty library from an empty query. When Songs exist but the current search or filters match none, show **No songs match** and a direct **Clear search and filters** correction rather than first-use guidance.
- Distinguish an archived-only library from both states. When no active Songs are visible but archived Songs exist, show **No active songs** with **Show archived**.
- Retain stable Song Library chrome, pane geometry, search, filters, and action ownership through all three states so content appearing later does not teach a different interface.

### 220. Empty Mass Templates within the permanent list-editor workspace

- Preserve the ordinary Mass Templates list-editor workspace before the first template exists. Do not replace it with a temporary onboarding page or a special-purpose setup wizard.
- Show **No templates yet** with a concise explanation that a Mass template provides reusable Mass sections, recurring responses, and typed Song/Reading slots for one language.
- Make **Create template** the primary empty-state action. Creation asks only for the template name and language, then opens the existing Template Editor with the adopted six-section scaffold: Before Mass, Introductory Rites, Liturgy of the Word, Liturgy of the Eucharist, Concluding Rites, and After Mass.
- Treat the six sections as editable structural starting points only. Do not speculate about language-specific responses, readings, rubrics, songs, or other liturgical content during creation.
- Keep template creation behind one Template Module interface so the empty-state action and every later **New template** action invoke the same validation, creation semantics, and editor transition.
- Continue allowing a Mass plan to be created without a template. If neither an applicable template nor a language fallback exists, open the already-adopted empty plan with its visible readiness warning rather than forcing template setup.

### 221. Optional Choir setup with inline lightweight creation

- Preserve the ordinary compact list-detail workspace under **Songs → Choirs** before the first Choir exists. Show **No choirs yet**, explain that Choir records support Arrangement discovery and Mass-plan defaults without owning separate lyric copies, and make **Add choir** the primary action.
- Require only a Choir name at creation. Keep short label, preferred languages, private notes, and Active or Archived state as optional details in the Choir management workspace.
- Never require Choir setup before creating Songs or Arrangements. Retain **Not assigned** as a valid choice wherever Choir association is optional; existing readiness rules continue to identify only genuinely unresolved choir-dependent Song slots.
- In editable Choir selectors, keep **Not assigned** available and offer **Create “{typed name}”** when normalized, case-insensitive matching finds no active Choir. Quick creation persists the lightweight record with only its required name and immediately selects it without leaving the Song or Mass task.
- Route both quick creation and the management workspace through the same Choir Module interface so normalization, duplicate prevention, identity generation, and persistence do not diverge across callers.
- Exclude Archived Choirs from ordinary selection suggestions. Let the operator deliberately reveal or manage them from **Songs → Choirs** rather than silently reactivating one through a selector.

### 222. Protected fallback-only Theme Library state

- Never present the Theme Library as literally empty. Always show a **System Theme** section containing the protected **Ordo Black** safety Theme with its canonical specimen preview and visible **Built in** status.
- When no parish-owned Theme exists, show **No parish themes yet** in a separate **Parish Themes** section and make **Create parish theme** its primary action. Treat this as a valid ready-to-present state rather than an error or onboarding blocker.
- Create a parish Theme as an independent copy of Ordo Black and open it immediately in the existing staged Theme Editor. This guarantees a complete renderable starting state while allowing every property to diverge safely.
- Route **Create parish theme** and explicit duplication of Ordo Black through the same Theme Module interface and revision-safe creation semantics. The operator-facing wording may reflect the initiating context, but identity generation, asset copying, and editor handoff remain one implementation.
- Keep Ordo Black as the Parish default until the operator explicitly sets another Theme as default. It remains selectable and duplicable but cannot be edited in place, renamed, archived, or deleted.
- Show **Parish default** on whichever Theme currently owns that role. On first use, Ordo Black therefore displays both **Built in** and **Parish default** without pretending it is parish-owned content.

### 223. Strict Invalid versus Needs-decision import states

- Use **Invalid** only when an Ordo file or contained document fails its contract, archive/path safety, checksum or integrity verification, required dependency closure, or supported schema-version check. Invalid content can never be force-imported or converted into an operator override.
- Keep every failed file visible in its review session with the exact human-readable reason and actions to **Choose another file**, **View technical details**, or **Remove from review**. Do not dismiss the workflow, reduce the failure to a toast, or mutate Parish data.
- Distinguish a valid newer schema from corruption. State that the installed Ordo version cannot read it and may need to be updated; preserve its technical version evidence without attempting a best-effort import.
- In multi-document Song or Liturgy intake, mark an invalid document **Excluded** and allow independently valid documents to continue through review. If any Arrangement or internal reference within one Song import document is invalid, exclude that complete document rather than salvaging fragments.
- Treat a Celebration Package and Parish Backup as indivisible verified artifacts. Failure of any required manifest entry, dependency, asset, snapshot, checksum, or stored-data check blocks the entire artifact before reconciliation, restoration, or installation.
- Reserve **Needs decision** for structurally valid content whose identity or reconciliation outcome needs an operator choice. It blocks the atomic commit until resolved but must not be styled or described as corrupt.
- Perform no database, managed-asset, Package-history, or Parish mutation before the accepted valid set commits atomically. Removing or replacing an invalid review input changes only temporary staging.

### 224. Coherent degraded rendering for a missing Theme background

- When a saved Theme background image is missing or corrupt, render that Theme's required fallback color immediately. Never expose a broken-image symbol, partial composition, or transparent accident to congregation output.
- Mark the reusable Theme **Degraded** and surface one deduplicated Mass Readiness warning wherever that Theme is inherited. Keep the warning advisory because the authored fallback color produces complete projectable output.
- In the Theme Editor, preserve the missing asset reference and show a clear placeholder with **Replace image** and **Remove image**. Never silently clear or rewrite the saved Theme merely because an asset lookup failed.
- Permit Presenter Mode after the ordinary warning acknowledgment. Permit a new local Presentation snapshot to capture the complete fallback-color result while recording the degradation in snapshot diagnostics and readiness evidence.
- Leave every previously validated immutable Presentation snapshot unchanged and available according to its own verified dependency state.
- Block Celebration Package export until the missing referenced image is replaced or deliberately removed. A locally presentable fallback does not satisfy the Package's promise to contain complete editable dependencies.
- Route detection and fallback rendering through the Theme and Slide Renderer Modules, while diagnostics and editor repair actions consume their normalized degraded-asset result rather than independently guessing asset health.

### 225. Advisory Theme-background aspect and resolution qualification

- Recommend Theme background images at `16:9` and at least `1920x1080`. Accept other aspect ratios and smaller images because Cover fitting, focal positioning, and deliberate low-resolution artwork remain valid authoring choices.
- When the selected image is not within the canonical `16:9` tolerance, show **Not 16:9 · Parts of this image will be cropped** beside the authoritative Cover preview and expose focal-position adjustment before Save.
- Treat the live crop preview as the operator's review surface. Once the Theme is saved with that crop, do not produce a recurring Mass Readiness finding merely because the source image is non-`16:9`; the crop is intentional Theme output.
- For an image below `1920x1080`, show the separate advisory **May appear soft on higher-resolution displays**. Do not block Theme Save, snapshot capture, Presenter Mode, or export solely for source resolution.
- Evaluate aspect ratio and effective source resolution inside the Theme asset-intake interface so import, replacement, and initial selection use the same qualification and wording.

### 226. Self-healing bundled-font cache with two-tier trusted recovery

- Ship a verified read-only factory font pack inside Ordo and load projection fonts from a separate versioned runtime cache. Verify the runtime cache against the build's pinned font manifest before canonical rendering.
- When the runtime cache is missing or corrupt but the factory pack remains valid, rebuild the complete cache into temporary storage, verify hashes and a canonical renderer probe, then switch it atomically. Continue without prompting for reinstall and record the successful repair quietly in Diagnostics.
- Never repair bundled fonts from the network, operating-system fonts, an imported Celebration Package, or Parish Backup. Those sources may not silently become application dependencies.
- If the factory pack is missing or corrupt but the runtime cache remains fully verified, continue normal rendering, snapshot capture, Presenter Mode, and export. Report **Font recovery copy missing** as a non-blocking Diagnostics finding because output is sound but local redundancy has been lost.
- Offer **Repair Ordo** guidance for that degraded-redundancy state and direct the operator to run a trusted same-version Ordo installer from offline media. Keep application repair owned by the installer rather than teaching the running application to extract pieces from another installer.
- If both factory pack and runtime cache are unavailable or corrupt, no trusted local font bytes remain. Do not substitute system fonts for congregation output. Keep the operator shell usable with a system fallback, allow ordinary authoring/autosave, but mark exact slide previews unavailable and block new snapshot capture and Celebration Package export.
- In the both-copies-lost state, allow Presenter Mode only for a previously validated snapshot whose complete canonical rendered frames remain intact and pass integrity checks; scale those frames without recomposition. If current edits require a new snapshot, Presenter entry remains blocked.
- If ordinary same-version installer repair fails, fall back to a full trusted uninstall/reinstall that preserves Parish data under the separately adopted retention rules.
- Hide verification, reconstruction, atomic replacement, and structured failure behind one Font Integrity Module interface consumed by startup, Diagnostics, the Slide Renderer, Presenter preflight, and artifact export.

### 227. Fail-safe Freeze when a future Cue becomes unavailable

- Never send an error surface, partial frame, renderer placeholder, or accidental black frame to congregation output when the selected future Cue cannot produce its complete canonical frame.
- If Program is Live when the operator navigates to an unavailable Cue, preserve the previous valid Program output and automatically enter Freeze with the explicit state **Frozen · Cue unavailable**.
- Show the same unmistakable operator-only **Output unavailable** placeholder and specific failure reason in the Cue card and Preview viewport. It is not a canonical projectable frame and must not resemble an ordinary black Hold Cue or Blackout.
- Keep unavailable Cues in normal Preview navigation rather than silently skipping liturgical context. Left/Right and Up/Down continue navigating; selecting a later valid Cue does not change Program while Freeze remains active.
- Let Space take a later valid Preview live through the ordinary unfreeze transition. If the currently selected Preview remains unavailable, Space preserves Freeze and reports why it cannot be taken live.
- If Blackout was already active, preserve Blackout rather than replacing it with Freeze. Navigation continues in Preview, and Space cannot restore an unavailable Cue; Blackout remains until a valid Preview is deliberately restored.
- Preflight should identify snapshot/frame failures before Presenter entry, but the same normalized Cue-availability contract must protect against corruption, decode failure, or renderer failure discovered during the active session.

### 228. Non-modal fail-closed interrupted-session presentation

- After unexpected termination, reopen directly into the recovered native-full-screen Presenter workspace rather than adding a blocking recovery dialog before the operator can regain context.
- Show a persistent operator header strip: **Presentation recovered · Congregation output is black**. Restore Preview to the last selected Cue and show the last Program frame only as clearly private pre-interruption context; never claim that it is currently visible externally.
- Restore journaled navigation, Freeze/Blackout/Live intent, emergency inserts, and emergency overrides to the operator session while keeping actual external Program fail-closed on black.
- If the previously assigned congregation display is confidently recognized and output had been enabled, recreate its passive window black. If it cannot be recognized, enter the adopted Disconnected state without moving output to another display.
- Let Space take the valid recovered Preview live and dismiss the recovery strip. Keep **Exit Presenter** available so an old or no-longer-relevant interrupted session can end through the ordinary safe exit flow.
- Do not add a second Resume confirmation. The black external output, persistent recovered-state strip, deliberate Space action, and visible Exit preserve safety while minimizing recovery latency during an actual Mass.

### 229. Installer-owned Repair, Upgrade, and downgrade refusal

- Make running the complete Windows installer the ordinary application-repair path. A same-version installer offers **Repair Ordo** or performs an equivalent explicit repair, replacing application binaries, factory fonts, and other packaged resources while preserving all Parish data outside the installation directory.
- After same-version repair, rebuild the runtime font cache and run application-resource, font, and canonical-renderer verification automatically. Do not create a Parish Backup merely for repair because no authoritative stored data or schema changes.
- Treat a trusted newer installer as **Upgrade Ordo**, not as a source of files to inject into the older build. Preserve the adopted complete-installer offline update flow; on first launch, verify integrity and create the locally managed safety Backup before any required ordered schema migration.
- Refuse an older installer as an in-place downgrade and never extract its font pack into a newer build, even when font family names appear unchanged. Supporting arbitrary data/schema downgrade is outside the MVP and risks opening newer Parish state with older semantics.
- Defensively refuse to open a Parish database whose schema is newer than the running application, without migrating backward, rewriting, or partially opening it, even if an operator bypassed installer downgrade protection.
- For controlled unsigned MVP deployment, retain published checksum verification before copying the installer to air-gapped media. Once public distribution begins, also require the adopted trusted publisher/signature checks.

### 230. Uninstall preserves Parish data by default

- Present exactly two uninstall scopes with **Keep Parish data — Recommended** preselected. This removes application binaries and disposable runtime caches while preserving the Parish database, managed assets, settings, Presentation snapshots, Package history, and app-managed local Backups outside the installation directory.
- A later compatible reinstall discovers the retained Parish scope and opens it only after ordinary schema-compatibility and integrity checks. Same-version installation should normally expose Repair, so uninstall-first is not required for routine application repair.
- Offer **Remove all local Ordo data** as the explicit destructive alternative. Before commit, list the app-managed categories that will be deleted, show the latest verified portable Parish Backup, and warn clearly when current authoritative changes are not protected.
- Require one explicit danger confirmation for full local removal, but do not require typing the Parish name. Do not invent Undo; recovery afterward exists only through a surviving portable Parish Backup or Celebration Packages according to their ordinary scopes.
- Delete only Ordo-managed local data selected by that scope. Never search for or delete exported `.ordo` files in user folders, arbitrary filesystem locations, removable drives, or another installation's storage.
- Keep application uninstall and Parish-data deletion as separate installer operations internally so the safe default cannot accidentally broaden through a packaging flag or cleanup routine.

### 231. Later-discovered Package and Backup availability versus damage

- Use **Unavailable** when a known Celebration Package or Parish Backup location cannot currently be accessed, commonly because its removable drive is absent. Make no corruption claim without reading and verifying the artifact.
- Use **Damaged** only when an accessible artifact fails checksum, archive, manifest, dependency, snapshot, or stored-data verification. Keep the artifact visible in History with its exact failed checks; never silently delete or relabel it as a generic missing file.
- Prevent a Damaged artifact from import, restore, presentation, or qualification as current portable-backup protection. Never attempt archive surgery by discarding failed entries, rewriting the manifest, or treating the surviving subset as valid.
- Offer **Locate another copy**. Replace the retained artifact atomically only when the selected copy verifies with the exact artifact identity and complete fingerprint; otherwise route it through the ordinary new import or Package-version workflow.
- For a Damaged Parish Backup, also offer **Create new backup** when the current Parish scope passes integrity checks. If the current Parish is itself unhealthy, require another independently verified Backup rather than manufacturing protection from damaged state.
- Verify the retained artifact, extracted installed dependencies, ordinary imported Parish records, and independently stored Presentation snapshots as separate managed copies. Damage to a retained Package file does not cascade into otherwise verified local records or frames; a damaged snapshot affects only that snapshot and not sibling Mass plans or Package versions.
- Keep current Parish-store corruption under Diagnostics and Parish recovery. Do not mislabel it as Celebration Package or Parish Backup damage merely because those artifacts may be possible recovery sources.

### 232. Discoverable contextual keyboard-shortcut reference

- Expose a labeled **Keyboard shortcuts** entry under Help in Preparation and in the Presenter Ordo-rail footer. Do not make discovery depend on memorizing a shortcut or interpreting an unlabeled icon.
- Use `Ctrl+/` to toggle the shortcut reference across Ordo. Do not use a bare `?`, which conflicts with text entry and varies awkwardly across keyboard layouts.
- Open the reference as a viewport-bounded floating right panel rather than a centered modal so the current workspace and operational context remain recognizable.
- Group commands under **Global**, the current workspace, **Editing**, and **Presenter**, placing the current context first. Display exact keycaps with concise stable action wording; omit shortcut search and customization from the MVP.
- In Presenter Mode, keep the shortcut reference passive: opening it never changes Program, and `F`, `B`, and `Space` retain their adopted presentation meanings. A text editor, emergency editor, or true modal continues suspending every single-key Presenter command.
- Let `Esc` close the reference and restore focus to the previously active Cue or control. Reuse one Shortcut Reference Module result across Help and Presenter so visible bindings cannot drift from registered commands.

### 233. Predictable multi-pane focus order with `F6` region navigation

- Keep ordinary `Tab` and `Shift+Tab` traversal in the same visible reading order as the GPUI focus tree. Visual reordering may never make keyboard order disagree with the interface, and ordinary panes may not trap focus.
- Add conventional desktop `F6` and `Shift+F6` navigation for jumping forward or backward among visible major regions. In Preparation, cycle **Global navigation → Workspace header/actions → Primary list or Mass outline → Main editor → Inspector**; skip every absent, hidden, or collapsed region.
- Keep selection and focus independent. Selecting a Song, Mass item, Cue, or other row may update contextual editor and Inspector content, but it never steals focus from the selected row. Moving into another region requires an explicit navigation action.
- Within lists, trees, tab sets, grids, menus, and Cue collections, use the expected component-level arrow-key pattern without converting `Tab` into row-by-row navigation. Preserve each component's selected item and meaningful focus target when the operator leaves and later returns to its region.
- Let a focused modal dialog or focused slide-over own and contain focus until it closes. Closing or completing it restores focus to the invoking control or the most relevant surviving entity when that exact control no longer exists.
- In Presenter Mode, use the smaller visible-region cycle **Ordo rail → Cue timeline → Output Dock and controls**. Skip a closed unpinned rail; treat an open overlay rail as the active focused surface; never let an `F6` jump change Preview, Program, Freeze, Blackout, or output enablement.
- Implement region registration and traversal once through the Ordo workspace shell/design-system seam. Workspaces declare their ordered regions and active targets without independently owning global key handling.

### 234. Operator-only interface scaling isolated from canonical slide rendering

- Provide installation-specific **Interface scale** choices of `100%`, `125%`, `150%`, `175%`, and `200%`, persisted as an installation preference and controlled through Settings plus standard `Ctrl++`, `Ctrl+-`, and `Ctrl+0` controls. Scale operator text, controls, spacing, hit areas, and focus treatment together rather than enlarging glyphs inside fixed-height chrome.
- Treat Windows/Linux system display scaling and Ordo Interface scale as operator-environment concerns only. They may resize and reflow Ordo's non-slide chrome but may never change the content geometry, font metrics, wrapping, pagination, safe area, or Theme typography of a canonical Slide.
- Do not implement Interface scale as an indiscriminate GPUI/window scale that also multiplies canonical Slide layout. Apply it through operator-shell layout and typography tokens outside the Canonical Slide Renderer seam; fit the passive congregation view through the qualified Output Fit path.
- Compose every Slide in an explicit `1920x1080` logical coordinate space independent of GPUI viewport dimensions, operating-system scale, Ordo Interface scale, and physical output density. A target display's physical resolution or scale factor may request a sharper raster, but density changes only sampling quality: logical glyph positions, line breaks, element bounds, and pagination remain identical.
- Treat thumbnails, Inspector, Preview, and Program as viewports onto a completed canonical frame. Interface scaling may change those viewport sizes and grid column counts, but it only scales the finished frame uniformly and never causes the Slide Renderer to reflow it.
- At increased scale, grow or reflow operator rows, reduce visual-grid columns as necessary, disclose secondary metadata, and keep viewport-bounded overlays internally scrollable. Never clip or overlap primary actions; retain the high-stakes Presenter controls even when the timeline reaches one column.
- Qualify the isolation contract with golden fixtures at Windows and Linux system-scale factors and every Ordo Interface-scale step. Assert invariant logical layout/pagination independently from output-raster density, and verify both supported desktop sizes plus the physical church display.

### 235. Concise screen-reader naming and safety-prioritized announcements

- Give every major operator region an explicit stable accessible name, including **Global navigation**, **Workspace actions**, **Mass outline** or the applicable primary list, **Mass editor**, **Inspector**, **Cue timeline**, **Program**, **Preview**, and **Output controls**. Keep names aligned with visible product vocabulary rather than exposing implementation terms.
- Give each interactive Cue one concise accessible name containing its position, meaningful title or first line, Cue type, and whether it projects, waits as an operator-only trigger, holds, or Blackouts. Expose complete slide or private-context text through a separately associated description or structured child content rather than turning the control's name into an entire paragraph.
- Represent focus, selection, current Preview, current Program, expanded/collapsed state, and availability through appropriate semantic state as well as visible styling. Do not force a focus jump merely to announce a state change.
- Let ordinary arrow-key Cue navigation speak through the newly focused Cue. Do not duplicate every movement through a live region. When linked Live causes that focused Cue to become Program, include concise current-state context without rereading the full slide.
- Use one restrained polite status channel for expected operator actions such as **Frozen**, **Blackout active**, **Preview taken live**, **Congregation output enabled**, and successful output restoration. Deduplicate repeated or unchanged messages so rapid operation does not build an announcement queue.
- Reserve urgent interruption for unexpected safety events that require immediate awareness: congregation display disconnection, an unavailable selected Cue, automatic Freeze, or loss of usable Program output. State both the event and the protective result, such as **Cue unavailable; previous Program held and Freeze activated**.
- Keep priest/minister prompts, timing context, diagnostics, and other operator-only information available to assistive technology solely in the operator workspace. The passive congregation renderer exposes and displays only congregation-facing canonical output.
- Centralize semantic labels and operational announcement events through shared workspace/Presentation interfaces so visual state, registered commands, and assistive output cannot drift. Qualify with keyboard-only and Windows screen-reader testing without requiring a screen reader for ordinary operation.

### 236. Editor-first adaptation for constrained Preparation workspaces

- Retain the full primary-list-or-outline, editor, and Inspector composition at the qualified `1366x768` desktop size under ordinary `100%` Interface scale. Wider space may increase useful preview and editing area but does not change ownership among those regions.
- Base adaptation on effective available workspace dimensions after operating-system scaling, Ordo Interface scale, window resizing, navigation chrome, and minimum usable logical pane widths of `240px` for primary list/outline, `520px` for editor, and `300px` for Inspector—not on physical monitor resolution alone.
- As horizontal space first becomes constrained, preserve the primary list or Mass outline beside the editor and turn the Inspector into a labeled right-side slideover. At narrower effective widths, preserve only the main editor and expose the primary list/outline and Inspector as opposing labeled slideovers.
- Permit only one temporary side pane to be open at a time in the constrained layout. Opening the other replaces it without losing selection, scroll position, editor state, or autosaved content; dismissal returns focus to its opener or the selected relevant entity.
- Never introduce whole-workspace horizontal scrolling, squeeze the editor beneath its usable content minimum, clip primary actions, or hide selection context merely to retain nominal three-pane geometry. Keep the current entity/title visible in compact workspace chrome.
- Keep the workspace's primary action visible and labeled where practical. Lower-priority actions may move into a clearly labeled overflow menu while preserving their wording, availability reason, keyboard access, and focus restoration.
- Treat layout mode as responsive presentation state, not persisted Parish content and not a separate editor mode. Returning to sufficient width restores fixed panes and their prior meaningful focus/scroll targets without reopening temporary slideovers.

### 237. Safety-first constrained Presenter composition

- Keep the adopted wide Presenter composition as the preferred operating experience: at `1920x1080`/`100%`, pin the Ordo rail and retain the Cue timeline plus dedicated right Output Dock; at `1366x768`/`100%`, begin with the Ordo rail collapsed and keep the timeline beside the stacked Program/Preview Output Dock.
- When effective width becomes too constrained for the side-by-side timeline and Output Dock, make the Ordo rail overlay-only and reflow the Output Dock into the main Presenter column above the Cue timeline. This is a responsive presentation of the same Presenter state, never a second operational mode.
- Keep Program and Preview simultaneously visible in one fixed side-by-side `16:9` monitor band in that constrained layout. Their internal frames remain identical canonical compositions; the wider layout simply provides the better and larger monitoring experience.
- Place congregation-output status, Freeze, Blackout, and the contextual primary action immediately below the monitor band. These high-stakes controls remain labeled, fixed, and keyboard-accessible; they never move into overflow or scroll away.
- Give the Cue timeline all remaining space and make only it independently scrollable. Preserve the operator's persisted Visual Slide Grid or Compact Cue List choice; a constrained Grid may fall to one column but never silently switches modes.
- Compact only low-risk session/header actions into a clearly labeled overflow. Never allow whole-Presenter scrolling, hide either monitor, or let responsive reflow change Cue selection, timeline position, Preview, Program, Freeze, Blackout, or congregation-output state.
- Determine this adaptation from effective available dimensions after system/Interface scale, not physical resolution alone. Returning to sufficient space restores the preferred wider composition and the meaningful focus targets of its regions.

### 238. Explicit responsive truncation and disclosure hierarchy

- Never truncate or replace with icon-only ambiguity the text for **Live**, **Freeze**, **Blackout**, congregation-output health, active warnings/errors, readiness counts, scheduled times, or the contextual primary action. Reflow these labels or adapt surrounding lower-priority content first.
- Let the current workspace or entity title wrap to at most two lines in its owning header. In narrow list, outline, and Ordo-rail rows, keep titles to one ellipsized line; allow Cue captions outside their canonical frame up to two lines.
- Reveal a visually truncated value in full on pointer hover and keyboard focus through the shared tooltip behavior. Its accessible name or associated text always contains the full value, never the rendered ellipsis.
- Collapse older breadcrumb ancestors into an accessible overflow menu before shortening the current entity. Keep the immediate navigation context and current entity visible.
- Remove or move secondary metadata into its owning Inspector/detail surface before compressing primary identity, state, or action text. Where compact formatting is necessary, use an intentional shorter equivalent rather than cutting characters unpredictably.
- Never ellipsize user-authored lyrics, readings, congregation responses, announcements, notes, or other editable prose. Wrap them or provide local scrolling within the bounded owning surface. Canonical Slide text remains governed exclusively by reviewed pagination and the Canonical Slide Renderer, never by operator-interface truncation.
- Prohibit marquee/auto-scrolling labels, hover-only access to essential meaning, and font shrinking below the design-system minimum as overflow remedies.

### 239. Approved constrained/high-scale Presenter acceptance reference

- Approve `design-references/ordo-presenter-high-scale.html` and its standalone rendering as the current visual acceptance reference for Presenter adaptation from `100%` through `200%` Interface scale at the compact physical desktop context.
- Keep the `175%` view as the reference's default because it makes the constrained composition explicit: Ordo is overlay-only; Program and Preview remain simultaneously visible and identical while Live; output status, Freeze, Blackout, and the Live primary state remain fixed beneath them; and the Cue timeline alone receives the remaining scrolling space.
- Retain the interactive scale and Grid/List controls as visual evidence that Interface scaling changes only operator chrome and responsive composition. Canonical frame text and geometry do not reflow, the selected Cue remains represented consistently, and the user's Cue-view preference survives adaptation.
- Treat the reference as information-hierarchy evidence rather than production markup or a source of exact breakpoints. The wider Presenter remains the preferred experience; production thresholds arise from effective available dimensions and qualified minimum region sizes under Decisions 234 and 237.

### 240. Approved paired Mass Planning responsive acceptance reference

- Approve `design-references/ordo-mass-planning-responsive.html` and its standalone rendering as the paired Mass Planning visual-acceptance reference for `1366x768` and `1920x1080` at `100%` Interface scale.
- Use one identical selected Song item, Arrangement, generated Cue, canonical Slide, Mass plan, and application state in both views so the comparison demonstrates responsive composition rather than different content or a separate workspace mode.
- At `1366x768`, retain the complete Mass outline, selected-item editor, and Inspector. Compact secondary labels and the **+ Add Mass item** control where necessary, preserve the plan header's warnings, Theme access, and **Present** action, and keep the selected item's canonical Slide visible in the Inspector.
- At `1920x1080`, preserve the same pane ownership and ordinary control size while spending additional space on more visible outline rows, fuller metadata and action wording, more generated-Cue thumbnails, and a larger useful Inspector preview.
- Treat the reference as approved information hierarchy and density evidence rather than production markup or an exact breakpoint source. Effective available dimensions and minimum useful pane widths continue to govern the production adaptation under Decisions 234 and 236.

### 241. Keep physical qualification outside the MVP application

- Keep the actual church PC/projector dress test as a mandatory release gate and project-owned release checklist rather than parish-domain content or an Ordo workflow.
- Record test evidence with the qualified build's release artifacts or project notes. Do not add an **Installation qualified** state, certification record, qualification history, or checklist-management screen to the MVP.
- Continue providing only the operational tools Ordo itself must own—Display Setup, calibration, Output Fit, Diagnostics, Presenter safety behavior, and passive congregation output. Human sign-off across the real room, cabling, PowerPoint fallback, and full-session conditions remains outside the application.
- Revisit in-application installation qualification only after real deployments demonstrate a recurring operator need; do not anticipate it before MVP field experience.

### 242. Split physical qualification into normal and adversarial passes

- Qualify a release candidate through two distinct runs on the actual church system rather than mixing deliberate faults into one undifferentiated rehearsal.
- **Normal session pass:** run a representative Mass plan uninterrupted and at realistic pacing for a full Mass-length duration. Use it to assess congregation readability, ordinary operator flow, projector/output stability, sustained CPU and memory behavior, thermal behavior, cursor hiding, and absence of unintended Program exposure.
- **Adversarial recovery pass:** run a shorter scripted sequence covering rapid navigation, Freeze, Blackout, cursor recovery, emergency insert/edit, congregation-display disconnect/reconnect, and simulated application interruption/recovery. Verify the protected Program result and the operator's recovery path for each event.
- Require both passes. A failure in one does not erase the evidence from the other, but the build is not church-qualified until the failed pass is corrected and rerun as required by the later requalification policy.

### 243. Invalidate physical sign-off according to change risk

- Tie physical qualification to the exact release candidate rather than treating one successful church test as permanent approval for all later builds.
- Require both the normal and adversarial physical passes again after changes to Presenter state, canonical Slide rendering, bundled fonts, congregation-output windows, display handling, GPUI/platform or other rendering dependencies, packaging, interruption/recovery, or performance-sensitive code.
- For changes confined to Preparation behavior with no path into snapshot capture, Presenter, rendering, output, recovery, or packaging, require the normal automated/native gates plus a short church-PC sanity check rather than another full Mass-length run.
- When a physical check fails or its result is ambiguous, correct the candidate and rerun at least the affected pass. Rerun both passes whenever the correction or root cause crosses both stability and recovery concerns, or when impact cannot be bounded confidently.
- Record which code/build change triggered or waived full requalification in the external release evidence; never infer validity merely from a version-number category such as patch, minor, or major.

### 244. Keep completed qualification evidence local and privacy-safe

- Commit only a blank, reusable physical-qualification report template to the source repository. Do not commit completed parish qualification reports.
- Store a completed report locally beside the exact qualified release bundle and its checksums so the evidence and tested binaries remain traceable together on the air-gapped deployment path.
- Capture the build identity/hash, published checksum, date, operator, relevant PC/Windows specifications, operating-system scale, operator and congregation display modes/topology, cable or adapter path, qualification fixture or Package hash, performance observations, both pass results, failures, corrections, reruns, and final sign-off.
- Exclude parish-authored content, congregation photographs or recordings, personal data beyond the deliberately named tester, and unsanitized diagnostics. Evidence screenshots, if useful, must contain only the operator's qualification fixture or sanitized diagnostic/calibration material.
- If Ordo later becomes open source, publish at most a sanitized qualification summary. Retain the full local report and church-specific topology outside source hosting.

### 245. Require operator and congregation-area observer during readability qualification

- During the normal physical pass, keep the Presenter operator at the church PC and assign a separate observer to assess congregation output from the room. One person should not be expected to operate the Presenter and judge distant projection simultaneously.
- Check the farthest routinely occupied seating area and at least one meaningfully off-center viewing position under representative church lighting.
- Judge comfortable practical readability, not merely technical visibility. The observer reports clipping, unsafe margins, poor contrast, soft or unstable rendering, timing artifacts, and text that requires strain to read.
- The observer need not be a trained AV operator, and neither role becomes an Ordo account, parish record, or in-application workflow. Record only the deliberately supplied tester names or roles in the external report.

### 246. Use one sanitized two-plan physical-qualification Package

- Maintain one reusable, versioned, sanitized Celebration Package for physical qualification and record its exact hash in each completed report.
- Include a **Normal Mass** plan with a realistic Sunday structure and pacing, both adopted congregation Themes, identification Slides, short responses, one-to-four-line lyric Slides, deliberately long wrapped text, reading/prose output, a repeated chorus, operator-only triggers, Hold, Blackout, representative Cebuano text, and text that exercises the bundled fallback font.
- Include a separate **Recovery Drill** plan whose visibly numbered Cues make rapid navigation, Preview/Program comparison, Freeze, Blackout, cursor recovery, display disconnect/reconnect, emergency edit, application restart/recovery, and final Program-state verification unambiguous.
- Keep the fixture free of parish-owned lyrics, personally identifying data, and content whose distribution would create avoidable copyright or privacy risk. Qualification exercises Ordo's structures, lengths, glyphs, and operational states rather than preserving a real parish Mass.
- Version the fixture whenever its contract or coverage changes. A report identifies both the application build and fixture hash so results remain reproducible.

### 247. Require at least a 60-minute normal physical pass

- Run the uninterrupted **Normal Mass** physical pass for at least `60 minutes`, including realistic idle intervals and ordinary Cue pacing rather than rapid automatic advancement.
- When a regular local Sunday Mass typically lasts longer than `60 minutes`, use that ordinary local duration for the pass. The MVP does not require simulating an unusually long feast, vigil, or special-rite duration merely to extend the clock.
- Count duration only while the release candidate remains in the intended Presenter session on the actual church PC/projector path. Setup, installation, import, calibration, and the separate adversarial drill do not satisfy the normal-pass duration.
- Keep longer automated or bench soak tests available for performance engineering, but do not substitute them for the minimum physical room-and-projector pass.

### 248. Make safety, output, readability, performance, and fallback failures release-blocking

- Fail physical qualification for a crash, hang, data corruption, lost operator input, or inability to recover through the documented operator path.
- Fail for any operator-only text, application chrome, pointer, diagnostic, partial frame, unintended application window, or other non-congregation material appearing on Program.
- Fail for incorrect Preview/Program behavior, Freeze or Blackout failure, wrong display targeting, unsafe display-disconnect behavior, or unsafe interruption/restart recovery.
- Fail for congregation text that is missing, clipped, unexpectedly reflowed, unstable, or not comfortably readable from the adopted observation positions, and for canonical scaling, safe-area, bundled-font/fallback-glyph, contrast, or Output Fit failure.
- Fail for exceeding the adopted `1 GiB` absolute memory ceiling, missing the adopted Presenter command-latency limits, losing rapid input, or otherwise violating the physical performance gates.
- Fail when the generated emergency PowerPoint cannot be opened and run correctly using the church PC's installed PowerPoint version.
- A minor operator-chrome polish issue may remain non-blocking only when it has no effect on meaning, accessibility, timing, recovery, operator confidence in a safety state, or congregation output. Record every accepted non-blocking finding with rationale; ambiguity is blocking until resolved.

### 249. Permanently select Electron as Ordo's desktop shell

> **Superseded by Decision 252.** Retained to explain the previous trade-off
> and why it was deliberately reopened after stronger evidence became
> available.

- Lock Electron + React as Ordo's production desktop architecture. Electron is no longer a provisional default and the implementation plan will not include a Tauri, GPUI, NativePHP, Wails, or other shell bake-off.
- Accept Electron's structural Chromium/Node memory and distribution overhead in exchange for one pinned Chromium generation across Windows and Linux, mature multi-window/display/fullscreen control, an established React toolchain, offline packaging, process/utility isolation, and the strongest low-risk path to future Chromium media support.
- Retain the `750 MB` preferred target, `1 GB` hard memory ceiling, `<50 ms` p95 Presenter-command target, rapid-input, idle-CPU, startup, and physical qualification gates. Framework selection does not excuse missing a performance budget; failure requires profiling, bounded caches, renderer-local hot paths, process correction, or an honestly revised supported hardware profile—not automatic framework reconsideration.
- Keep keydown-to-Preview and keydown-to-Program paint measurement in Milestone 0. The live command path remains an in-memory Presentation Engine transition plus narrowly subscribed operator and congregation updates, with no SQLite, filesystem, autosave, pagination, image decode, synchronous IPC, or broad timeline rerender on the path.
- Bound decoded full-resolution frames, keep operator thumbnails genuinely low-resolution and virtualized, preload only the nearby presentation set, release obsolete media, and measure memory by Electron process. Keep hardware acceleration enabled by default and treat software rendering only as a diagnosed compatibility fallback.
- Treat Discord as evidence that a heavily interactive React/Electron desktop product can be responsive, not as an Ordo benchmark. Ordo's own packaged measurements on the minimum-profile and actual church PC remain authoritative.
- Reject Tauri because its likely footprint advantage is unproven for Ordo's two-WebView workload and its WebView2-on-Windows/WebKitGTK-on-Linux split directly weakens canonical rendering parity. Reject GPUI because it replaces React with a pre-1.0 Rust UI rewrite and transfers far more cross-platform rendering/media ownership to Ordo. Reject NativePHP because its Desktop product still bundles Electron while adding Laravel/PHP runtime layers. Keep Wails outside the roadmap while its required multi-window generation remains prerelease and cross-engine.
- Preserve the Display Control, Storage, Presentation Engine, and Canonical Slide Renderer seams for Depth, Locality, process isolation, and testability. They do not signal a planned future shell migration.
- Primary research references: [Electron process model](https://www.electronjs.org/docs/latest/tutorial/process-model), [Electron performance guidance](https://www.electronjs.org/docs/latest/tutorial/performance), [Electron screen Interface](https://www.electronjs.org/docs/latest/api/screen/), [Tauri WebView versions](https://v2.tauri.app/reference/webview-versions/), [Tauri Windows installer/WebView2 choices](https://v2.tauri.app/distribute/windows-installer/), [GPUI status](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md), and [NativePHP Desktop building](https://nativephp.com/docs/desktop/2/publishing/building).

### 250. Use a hybrid global-rail and local-tab navigation hierarchy

> **Superseded in part by Decision 257.** Rename the global **Songs** destination
> to **Library** and place Songs, Responses & Prayers, and General Presentations
> in its local navigation. The global-rail/local-tab hierarchy remains adopted.

- Use one persistent vertical application rail for global Preparation destinations rather than presenting global workspaces as a horizontal tab strip.
- Make **Masses** the default global destination, followed by **Songs**, **Themes**, and **Transfer & Backup**. Anchor **Settings**, **Help**, and **Diagnostics** at the bottom of the same rail.
- Reserve horizontal tabs for views or tasks within the current global destination or selected entity. Examples include **Upcoming / History / Archive / Templates** inside Masses, **Lyrics / Sequence** inside an Arrangement, and the adopted task navigation inside the Theme Editor.
- At comfortable effective widths, show rail icons with persistent labels. At constrained effective widths or increased Interface scale, collapse it to accessible labeled icons with matching tooltips and a non-color-only active treatment.
- Keep the rail compact when necessary in focused width-hungry editors, while preserving current destination, breadcrumb context, primary action, selection, focus, and return state. Responsive rail presentation is operator UI state, never persisted Parish content.
- Presenter Mode replaces the Preparation application shell with Live Control and therefore does not show this global rail; its separate collapsible **Ordo rail** remains a Presenter-specific navigation surface.

### 251. Separate strategic planning from checkbox-driven execution milestones

- Assign authority by subject: `MVP-PLAN.md` owns product outcome, scope, journeys, strategic architecture, contracts, verification strategy, release gates, risks, and deferred work; `CONTEXT.md` owns domain terms and invariants; `DESIGN.md` owns visible behavior and visual tokens; and `MILESTONES.md` owns task order, status, acceptance criteria, and delivery gates. Use this ledger to resolve gaps and reconcile any conflict into the applicable shorter document before implementation.
- Move the implementation sequence into `MILESTONES.md` and make it the sole task-status document so parallel milestone descriptions cannot drift.
- Give every execution task a Goal, intended Changes/locality, dependencies, objectively checkable checkbox acceptance criteria, and at least one E2E scenario.
- Tag verification honestly as automated CI, automated native GPUI UI testing, native packaged automation, scripted manual work, or physical church qualification. Human judgment and actual projector/PowerPoint behavior may never be relabeled as automatic merely to produce a green CI result.
- Order work as executable vertical slices and risk qualification rather than disconnected screens. Test domain behavior through the owning deep Module Interface and retain Phase gates that leave a demonstrable application state.
- Keep post-MVP Song merge, cloud/login, arbitrary PowerPoint import, media playback, remote control, and other deferred features out of the MVP task graph.

### 252. Replace Electron with a clean-slate Rust and GPUI implementation

- This supersedes Decisions 28–31, 33, 58, and 249 to the exact full/partial
  extents recorded by the forward notice in each affected section.
- Lock Rust plus Zed's GPUI as Ordo's production desktop architecture for the
  MVP. Electron, React, TypeScript, Vite, Tailwind, shadcn/Base UI, Chromium,
  Node.js, preload/IPC, and Electron Forge are no longer part of the intended
  implementation.
- Treat this as a deliberate supersession of Decision 249, not an additional
  shell experiment or a dual-frontend strategy. The implementation plan will
  contain no Electron fallback application and no ongoing GPUI-versus-Electron
  bake-off.
- Restart development from a clean repository and source tree after this
  planning baseline. The committed production repository and its current
  `MVP-PLAN.md`, `CONTEXT.md`, `DESIGN.md`, and upstream pins are authoritative.
- Use a Cargo workspace with pure Rust domain Modules and explicit native
  Adapters. Keep Song Library, Mass Planning, Pagination, Presentation Engine,
  Canonical Slide Renderer, Storage, Portability, PowerPoint export, Display
  Control, and Presenter Preflight behind the already adopted small,
  domain-shaped Interfaces.
- Pin the Rust toolchain, Cargo dependency graph, GPUI/gpui-platform revision,
  and any optional GPUI behavior library used by a release. GPUI is pre-1.0;
  upgrades are scheduled compatibility work with qualification evidence, not
  an automatic move to upstream `main`.
- Build an Ordo-owned native UI foundation directly on GPUI. Translate the
  accepted `DESIGN.md` tokens and HTML acceptance references conceptually;
  HTML, CSS, Tailwind classes, React components, and shadcn source are visual or
  behavioral references only and are not production dependencies.
- Third-party GPUI libraries such as `gpui-base` or `gpui-component` may be
  evaluated for difficult low-level behavior, but adoption requires an exact
  pinned revision, license review, native accessibility/focus tests, and a
  contained Adapter that does not expose library properties through Ordo's
  visible workflow Interfaces. No such library is assumed until qualified.
- Use native/server operating-system window decorations for the Preparation
  window in the MVP. Presenter Mode remains borderless full-screen. Defer a
  custom client-side titlebar unless native qualification demonstrates an
  operational need.
- Run the application as one native process while keeping the Presenter hot
  path in memory and free from database, filesystem, pagination, image decode,
  autosave, or other blocking work. Move SQLite ownership and CPU-heavy jobs to
  dedicated Rust worker threads/tasks behind asynchronous, typed Module
  Interfaces; do not reproduce IPC merely to mimic Electron's topology.
- Use SQLite through a pinned Rust driver behind the Storage Module. Qualify the
  concrete driver in Phase 0; the planned baseline is `rusqlite` with a single
  owning worker thread, explicit SQL, versioned migrations, WAL/recovery policy,
  and domain-shaped requests. Callers never receive raw connections or SQL.
- Keep one shared Canonical Slide Renderer for timeline thumbnails, Inspector,
  Preview, Program, congregation output, snapshots, and PowerPoint frames. Its
  `1920x1080` logical composition and bundled-font shaping must be independent
  of GPUI operator layout and operating-system scaling. Cross-platform glyph,
  wrap, pagination, and frame parity is the principal Phase 0 technical gate.
- Replace browser/Electron UI testing with pure Rust Module tests, GPUI
  `TestAppContext` and internal action/render harnesses, golden-frame tests,
  native packaged smoke tests, scripted accessibility checks, and physical
  projector qualification. Do not pretend external desktop automation is
  reliable where GPUI/AccessKit cannot yet expose stable automation identity.
- Preserve the existing release outcomes: native x64 Windows 10/11 support,
  supported Linux preparation, conventional offline Windows installation,
  Linux Flatpak plus diagnostic ZIP, no runtime internet requirement, and
  native CI builds. Replace Electron Forge/npm packaging with pinned Cargo and
  platform-native packaging scripts.
- Retain the adopted `<50 ms` p95 Presenter command goal, rapid-input safety,
  idle-CPU gate, bounded frame caches, and `1 GiB` absolute memory ceiling.
  Adopt a `512 MiB` total-process qualification budget for every representative
  Phase 0 run; exceeding either limit blocks Phase 1 until an explicit budget
  and native-memory rationale revision is adopted.
- Keep the existing `8 GB` church-PC profile as the conservative initial
  qualification machine, not as proof that GPUI requires 8 GB. Decision 252
  removes Chromium as the reason for rejecting 4 GB; support for a lower-memory
  machine may be added only after the same representative Presenter, recovery,
  and physical safety gates pass with adequate operating-system and GPU
  headroom.
### 253. Use short internal crates and an interactive UI laboratory

- Organize the initial production workspace into seven internal crates named
  `domain`, `storage`, `rendering`, `presentation`, `ui`, `platform`, and
  `ordo`. Do not repeat the `ordo-` product prefix on internal crate names;
  the workspace already supplies that context.
- Use `domain`, not `core`, for the pure Song, Mass Planning, Pagination, and
  related domain Modules. This avoids collision and ambiguity with Rust's
  built-in `core` crate and makes the crate's responsibility explicit.
- Make `ordo` the composition-root crate and the user-facing executable. It
  owns feature views and application wiring but no domain, persistence,
  canonical-rendering, or platform rules.
- Treat crates as dependency-enforcement tools rather than a folder for every
  future feature. Begin Portability and PowerPoint export as Modules or create
  their dedicated `portability` and `powerpoint` crates only at the milestone
  where their deep Interfaces justify the extraction. Otherwise keep the Module
  in the closest existing production crate that preserves dependency direction
  and update the affected milestone Changes paths before implementation. Do not
  scaffold pass-through crates merely to match a diagram.
- Keep the native `ui` Module internal to Ordo. Build its visual language from
  `DESIGN.md` on GPUI while using a pinned and qualified `gpui-base` for useful
  unstyled behavior. Do not adopt the styled `gpui-component` layer wholesale,
  and do not expose `gpui-base` properties through Ordo workflow Interfaces.
- Add a development-only `tools/ui-lab` binary that depends on `ui`, not on the
  complete application. It is the interactive proving ground for primitive
  states, themes, viewport presets, Interface scale, keyboard/focus behavior,
  overlays, accessibility semantics, long and multilingual content, and
  constrained layouts.
- Define UI-lab scenarios as reusable fixtures that can be opened manually and
  exercised by the GPUI automated UI harness. The lab is not shipped to parish
  users and must not become a second implementation of Ordo workflows or a
  general-purpose published component library.
- Keep workspace package names and directory names aligned. If a crate is ever
  intentionally published outside the workspace, reconsider its public package
  name at that time instead of burdening every internal import now.

### 254. Use a layered, repository-local agent-development system

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

### 255. Use fixed semantic congregation-slide layouts

- This supersedes Decision 19's default safe-area dimensions, Decisions 46/57's
  corresponding calibration/qualification guides, and the relevant
  layout/background limits in Decisions 18 and 100; their unaffected safety and
  inheritance rules remain.
- Compose every Slide in renderer-owned header, body, and footer zones on the
  canonical `1920x1080` canvas. Do not support freely draggable elements in the
  MVP. Use default safe-area insets of `120` horizontal and `80` vertical
  canonical units; Themes may increase but not reduce them. An absent header or
  footer collapses so the body receives the space.
- Lyrics use an optional top-left section label and authored-case, left-aligned
  text optically centered in the remaining region. The label defaults on,
  repeats on every generated Slide, and is toggled once per Song item.
- Response Slides render an optional repeated authored header and an emphasized
  uppercase congregation response. A language-specific Mass template owns the
  default copied into its plan, and an item may inherit, hide, or override it;
  `TUBAG:` is the Cebuano template default, not a renderer literal. Store
  authored casing unchanged. Response sequences contain ordered optional
  private minister text plus required projected congregation responses.
- Reading identification uses a large uppercase proclamation title with its
  reference at bottom-left and no redundant First/Second/Gospel output label.
  Subsequent Scripture Slides preserve authored casing, use smaller text, retain
  the reference on every Slide, paginate automatically, and honor blank lines as
  hard boundaries.
- Ordinary Gospel templates may order an optional opening exchange,
  identification, congregation acclamation, optional Scripture, and closing
  exchange. Language-specific templates own the wording and optional blocks. A
  Good Friday preset may omit the opening greeting and pre-passage acclamation
  while retaining the closing exchange.
- Responsorial Psalms use the same template-default plus plan/item override
  rule; `SALMO RESPONSORYO` is the Cebuano template default. They project the
  refrain before the first cantor verse and after every verse (`n + 1` linked
  occurrences). Cantor verses are private by default and use Wait Cues that
  leave Program unchanged; optional projection turns them into ordinary
  Scripture-style Slides.
- Song identification is a bottom-left block with a large uppercase title,
  smaller composer/source, and optional Arrangement/Choir credit. The automatic
  selectable Celebration-title Cue precedes the first Mass section, defaults
  on, doubles as the idle Cue, uses an uppercase optically centered title, and
  anchors its localized date bottom-left.
- Celebration-title appearance may inherit the Mass Theme, select another
  reusable Theme, or replace only the background with one Mass-local image.
  Keep the resolved Theme's other typography/layout rules and require cover,
  focal-position, overlay, contrast, and fallback-color handling.
- General Presentations have only constrained Text and Image modes: optional
  header/center-left body/footer, or one contained foreground image with
  optional header/caption. Theme backgrounds remain separate.
- Validate actual canonical geometry, never character count. Lyrics target two
  phrases and no more than four rendered lines; split before shrinking and warn
  below the approved minimum. Song and Celebration titles allow at most three
  lines before readiness warning. Exact numeric type levels remain provisional
  until the retained Canonical Slide Layout Gallery is visually qualified.

### 256. Refine operator appearance, overlays, and bounded output motion

- This supersedes Decision 59's fixed Preparation polarity and Decision 217's
  overlay timing, and narrowly supersedes Decision 44 only for transitions at
  Blackout/Hold boundaries; content-to-content cuts remain unchanged.
- Preparation offers System (default), Light, and Dark appearance. System
  follows operating-system changes live. Presenter always uses graphite dark
  and restores the Preparation preference on exit.
- Use the same visible dim scrim for dialogs, ordinary slideovers, and unpinned
  Presenter Ordo. It captures the first outside click and dismisses the
  temporary surface without activating content beneath it. Pinning Ordo removes
  the scrim and makes it a fixed pane.
- Keep `O` as open/focus Ordo without changing pin state. Pin/Unpin is an
  explicit visible action; `Esc` closes an unpinned Ordo or returns focus from a
  pinned Ordo without collapsing it.
- Place operator-only toasts bottom-center. Use transient toasts for
  confirmation/Undo and persistent owned notices for failures or hazards.
- Content-to-content Slide changes remain cuts. Content-to-Blackout/Hold and
  Blackout/Hold-to-content fade for `500ms`, independently in Preview and
  Program. A contrary command reverses from current opacity with proportional
  remaining duration; never queue presentation commands.
- Slideovers open in `400ms` with fast-start/slow-finish motion and close in
  `200ms` with smooth acceleration. Motion is interruptible. Reduced motion
  removes translation and limits clarifying opacity to `100ms`.
- Retain fixture-backed HTML acceptance references under `design-references/`
  as behavior and hierarchy evidence. The first addition is one Canonical Slide
  Layout Gallery covering every adopted template, variants, backgrounds, and
  overflow cases through the same scaled canonical frame. Production remains
  native GPUI and aims for behavioral/design-intent parity, not browser pixels.

### 257. Make Library a multi-view destination with revision-bound reuse

- This supersedes Decisions 64/250's Songs destination label and Decision 112's
  Mass-owned template Response copies, and supersedes Decision 71's separate
  Custom text/Announcement presets plus Decision 168's Custom text editor label
  with General Presentation.
- Rename the top-level Songs destination to **Library** with local views
  **Songs**, **Responses & Prayers**, and **General Presentations**. Keep Song
  and Arrangement as their specialized domain rather than forcing every entry
  into one generic content schema.
- Response/Prayer and General Presentation entries are reusable sources. A Mass
  item references one until **Customize for this Mass** creates a local override;
  Mass templates retain these links when instantiated.
- A General Presentation Mass item may add private delivery context without
  changing the reusable source. That context remains operator-only and never
  enters a canonical frame.
- One Mass plan owns one reusable-source binding per source. Every still-linked
  occurrence uses that binding's Edit revision. Reusing the source again uses
  the same bound revision; a newer upstream revision appears as **Update
  available** until reviewed, and advancing updates every still-linked
  occurrence atomically. Local overrides remain unchanged.
- A local override remembers its source and base revision and exposes compare
  and replace-with-latest behavior when upstream changes. Presenter sessions pin
  resolved revisions; Preview, Program, and upcoming Cues never hot-refresh.
  Historical Presentation snapshots remain immutable.
- Promoting a local override never auto-merges across a changed upstream. Review
  latest source beside local content and save an explicit new source revision.
  Advance the current plan binding on success; relink the promoted occurrence
  only when it exactly matches the saved source. Other plans receive **Update
  available**.
- Archiving never breaks a bound occurrence and is not by itself a readiness
  failure. Hide archived entries from selection by default and expose them with
  **Include archived**. Restore before ordinary source editing. Permanent delete
  is limited to never-referenced entries with no retained history.

### 258. Bound live insertion, auto-play, windows, and save safety

- This extends Decisions 11–12a, 54, 113, and 203 without changing their
  unaffected live-safety, autosave, or same-window invariants.
- `Insert` Freezes and opens an anchored chooser for General text/image,
  Response sequence, or Song. Insert immediately after the current Mass item in
  the same section, select its first projectable Preview on save, and leave
  Program frozen until Space. Missing Songs created live are Mass-local first
  and may be promoted later.
- Emergency edits and inserted content autosave to the Mass plan by default.
  Updating a reusable source is a separate reviewed action and never rewrites
  immutable snapshots.
- Auto-play starts only by explicit operator action and targets exactly one
  General Presentation item or one Mass section. `A` toggles the item scope;
  the visible menu offers **Loop this section**. Never loop the complete plan or
  an arbitrary selection. Timed Songs are post-MVP.
- `A` is active only while focus is within the Cue timeline or Output Dock and
  no text editor, emergency editor, or true modal owns focus. Passive Ordo-rail
  and shortcut-reference focus suppress it while leaving `F`, `B`, and Space
  active.
- The reusable General Presentation owns an eight-second default duration,
  inherited by its Slides with optional Slide overrides. Every newly shown
  Slide receives its full timer, including after manual Previous/Next, and the
  last wraps to the first in scope. Manual navigation outside scope, Freeze,
  Blackout, leaving Presenter, or recovery stops and resets to manual.
- Keep one operator window. Presenter transforms it; emergency editors are
  slideovers and only congregation output uses another window. Presenter with
  output disabled is the rehearsal workflow, not a separate Rehearse mode.
- Back, close, and application exit flush pending autosave. On failure, block
  navigation with Retry save, Keep editing, and destructive Leave without
  saving; a successful retry resumes the original action.

### 259. Keep Celebration defaults small and make reuse review explicit

- Celebration owns one default title. A Mass plan inherits it and may override
  the projected title for another language or local wording; clearing restores
  inheritance. Do not add Celebration primary-language or localized-title maps.
- Reuse for another Celebration marks every copied Reading **Needs Review**,
  even when the language matches. Editing or reviewing clears one flag; **Mark
  all readings reviewed** clears the remainder after confirmation. Reading text
  stays language-specific Mass-plan content rather than Celebration metadata.
- **Next Mass** targets only the next scheduled sibling under the same
  Celebration. Fade to black, close the current session safely, and open that
  plan's preflight without exposing it automatically. Hide the action when no
  later sibling exists.
- Use **Mass plan** for the complete scheduled preparation and **Ordo** for its
  ordered sections/items. Avoid **Mass Ordo**.

### 260. Separate human Song authoring from durable exchange and defer the ERD

- This supplements Decisions 16/40 and 135–138; `.ordo` remains the machine
  contract while Markdown is an editor-entry format.
- Accept `.md` and `.txt` as human-authored input for one Song plus one
  Arrangement, opening directly into the unsaved editor with inline validation
  and duplicate review. Retain `.ordo` as the versioned machine exchange and
  history container with stable identity and idempotent re-import.
- Use YAML front matter with required `title` and optional `aliases`, `composer`,
  `source`, `rights`, `arrangement`, `language`, `choirs`, `translator`,
  `adapter`, `liturgical_moments`, `occasions`, and `notes`.
- A blank line is a hard Slide boundary within the current labeled section.
  Labels persist across Slides; `[Unlabeled]` starts label-free content.
  `[Repeat: label]` is a linked occurrence, may refer forward or backward, and
  matches case-insensitively after trimming. Missing or ambiguous references
  remain visible and make the draft not presentation-ready without preventing
  draft save.
- Do not create a speculative ERD before persistence exists. Add `ERD.md` with
  the SQLite migration task; make it a physical schema reference whose Mermaid
  overview and per-table details correspond directly to authoritative
  migrations, while `CONTEXT.md` remains the domain glossary.

### 261. Derive display pixels through the compositor, not a CPU raster cache

- This supplements Decision 234, which stays unchanged: canonical composition
  remains an explicit `1920x1080` logical space, and Interface scale, system
  scaling, and output density still change sampling only. What changes is how a
  completed canonical frame reaches a screen.
- Surveyed open-source presentation engines draw live output from a retained
  scene their compositor owns: LibreOffice Impress walks a primitive tree,
  OpenLP renders an HTML display document in an embedded browser engine, Quelea
  builds a JavaFX scene graph, and FreeShow mounts one component tree for both
  preview and projector. Full-frame rasterization appears only at the periphery
  — slide-sorter thumbnails, transition textures, screen capture, and export.
  The engines that do cache one bitmap per slide are PDF-backed and have no
  structured scene to retain, which is not Ordo's situation.
- Treat the logical `Frame` as a display list to be painted, not as input to a
  CPU compositing pass. The pinned GPUI revision paints already-shaped glyphs
  directly and caches their masks in a sprite atlas keyed by face, glyph, size,
  subpixel variant, and scale factor, shared across every slide and viewport.
  Compositing RGBA on the CPU, encoding it, decoding it, and uploading it
  reimplements that atlas per frame and holds the result in process memory.
- Keep the deterministic CPU rasterizer. It remains the qualified path for
  export, golden fixtures, and the density-invariance evidence, and it is the
  reason Ordo can produce exact pixels from a logical frame without capturing a
  live surface the way the surveyed engines must. Retire it from the display
  path only, and only on T0.6's measurement.
- Accept that compositor-painted output cannot be byte-identical to
  `tiny-skia` goldens, because platform glyph rasterization and subpixel
  handling differ. Qualify the display path against canonical composition —
  identical logical geometry, wrapping, and pagination — and keep byte-exact
  golden comparison on the CPU path, which is the separation T0.4 already
  established.
- Rasterize a full frame on the display path only where a retained redraw buys
  nothing: blending two frozen frames during a transition, which is what
  Impress uploads two textures for. Not for static slides, and not before
  transitions exist.
- Decide by measurement, not by this entry. T0.6 compares both paths and
  confirms the bundled canonical faces resolve through the GPUI text system
  rather than a same-named system installation; T1.3's caching scope is
  reconciled against that result before it starts.

### 262. Render Reading identification in sentence case at the Celebration title level

- This narrowly supersedes only Decision 255's Reading-identification rule;
  Decision 255 stays historical and authoritative for every other template, and this does not revive older Celebration-title appearance wording that T0.4.10 already updated.
- Render the proclamation title exactly as authored (normalized sentence
  case, never uppercased): `Pagbasa gikan sa basahon ni Sirac` / `Sirac
  27:30-28:7`, `Pagbasa gikan sa sulat ni San Pablo ngadto sa taga-Roma` /
  `Roma 14:7-9`, `Pagbasa sa Santos nga Ebanghelyo sumala ni San Mateo` /
  `Mateo 18:21-35`.
- Compose the title at the shared candidate `135px`/weight `800`,
  left-aligned to the safe-area left edge and vertically centered in the
  body region above the unchanged bottom-left meta reference; references
  keep their size and position. No First/Second/Gospel labels, and no other
  template, schema, parser, UI, compositor, or typography change.

### 263. Render Reading passage at a ReadingPassage-only 88px candidate

- This narrowly adds a bounded ReadingPassage-only slice; this supersedes Decision 255's shared 70px passage size only for ReadingPassage and makes no change to GeneralText or any later superseding decision.
- Compose `ReadingPassage` bodies at the pending candidate `88px`/weight
  `400` (`70 * 1.25 = 87.5`, rounded; line advance `102px`), left-aligned
  to the safe-area left edge and top-anchored in the body zone, with the
  unchanged `50px` bottom-left reference. Reading behavior, blank-line
  boundaries, and authored casing are unchanged.
- `GeneralText` stays at `70px`; operator, UI, and lab-control typography
  are unchanged. No other fixture, golden, template, schema, parser, UI,
  raster, or compositor change.
- The existing short English fixture wording matches the Catholic USCCB
  text at https://bible.usccb.org/bible/john/1; no additional Scripture
  text is added.

### 264. Render Response at a Response-only 113px candidate, centered vertically

- This narrowly adds a bounded Response-only slice; this supersedes Decision 255's Response size/vertical placement only for Response and makes no change to Lyrics, Psalm, Song identification, Reading, GeneralText, or any later superseding decision.
- Compose `Response` bodies at the pending candidate `113px`/weight `400`
  (`90 * 1.25 = 112.5`, rounded; line advance `130px`), centered
  horizontally and centered vertically in the available body zone. The
  shared `PROVISIONAL_DISPLAY_SIZE` stays at `90px`; uppercase body,
  optional authored header behavior, and the `50px` header size/weight/position
  are unchanged. Present/empty headers reserve the `58px` header zone with
  their one-line body starting at `y=504`; an absent header keeps no header
  rect with its one-line body starting at `y=475`.
- Lyrics, Psalm, Song identification, ReadingPassage top anchoring, GeneralText
  `70px`, operator `24/16/14`, and lab-control typography are unchanged. No
  other fixture, golden, template, schema, parser, model, UI, raster, or
  compositor change; no fixture text change.

## Final design-status summary

The following branches are resolved for the MVP baseline:

1. **Presenter command chrome:** completed—command ownership, placement, Ordo-rail access, discoverability, focus return, and shortcut suspension are adopted.
2. **Mass Planning workspace:** completed for the MVP—the focused three-pane hierarchy, floating Arrangement picker, top-right Add Mass item placement, owner-directed pagination, Inspector Cue navigation, editable six-section scaffold, section behavior, and Presenter handoff are adopted.
3. **Song Library and Arrangement Editor:** completed for the MVP—Arrangement-free Song creation, collapsed/nested duplicate preview, the persistent Song Library master-detail workspace, normalized Song/Arrangement/lyrics search, Arrangement-level Choir/language/occasion filtering, hierarchical automatic-first atomic Song import review with read-only content comparison and explicit near-duplicate Arrangement revision boundaries, Song/Arrangement autosave boundaries, editor-local immediate Undo, fine-grained durable revisions with deterministic History-session grouping, entity-owned History surfaces, nested whole-entity comparison and guarded restoration, independent non-cascading Arrangement archival, inspect-only archived state, reference-safe non-cascading permanent deletion, preservation of archived Arrangement references in Mass plans, and the complete Lyrics/Sequence authoring workspace are adopted. Song merge and its explored reversal model are retained for post-MVP work only.
4. **Pagination handling and Theme Editor:** completed for the current scope—pagination correction routes directly to the owning editor rather than a standalone workspace; Mass Theme selection, override review, dedicated Theme Editor navigation, canonical specimen previews, Typography/Appearance/Layout controls, staged shared editing, revision-based Save/Undo, exception-based affected-plan review, solid/image backgrounds, dirty-session navigation, contextual duplication, recoverable library management, and visible revision history are adopted.
5. **Transfer, backup, and settings workspaces:** completed for the current scope—Celebration Package and Parish Backup creation/restore, manual-only safety-Backup retention, installation-specific Displays and calibration, same-window native-full-screen Presenter transformation, and offline Diagnostics with deliberate sanitized export, explicit scan lifecycle, and issue-first hierarchy are adopted.
6. **Shared visual tokens:** completed—the coordinated warm-paper/graphite appearances, semantic colors, Google Labs-compatible `DESIGN.md`, interaction-state behavior, focus, separators, corner geometry, elevation, iconography, adaptive spacing/density, and functional motion are adopted.
7. **Non-happy-path screens:** completed for the MVP—first-run, empty/fallback-only workspaces, strict import states, asset/font recovery with installer-owned repair, safe uninstall retention, fail-safe unavailable-Cue behavior, disconnected-output recovery, interrupted-session presentation, and later-discovered Package/Backup damage are adopted.
8. **Accessibility and keyboard review:** completed for the MVP—visible focus, non-color state foundations, reduced-motion tokens, contextual shortcut discovery, complete multi-pane focus order, operator-only interface scaling isolated from canonical Slide composition, and concise safety-prioritized screen-reader semantics are adopted.
9. **Responsive visual acceptance:** completed for the software references—the responsive rules, constrained/high-scale Presenter reference, and paired `1366x768`/`1920x1080` Mass Planning reference are approved. Physical church Presenter/projector qualification remains the final operational branch and eventual release gate, administered through the external release checklist rather than Ordo UI.
10. **Physical church qualification design:** completed—the external two-pass gate, risk-based requalification, privacy-safe evidence, operator/observer roles, sanitized two-plan fixture, `60`-minute minimum normal run, and blocking-failure boundary are adopted. Execution remains an implementation/release activity on the finished release candidate and actual church system.
11. **Desktop shell research:** completed—Rust + GPUI supersedes Electron + React as a clean-slate implementation. Performance and cross-platform canonical-rendering gates are mandatory, and no dual-shell bake-off is planned.
12. **Preparation-shell navigation:** completed—the global vertical application rail distinguishes destinations from workspace-local horizontal tabs, adapts to effective width/scale, and disappears when the main window transforms into Presenter Mode.
13. **Implementation execution plan:** completed and reconciled to the GPUI restart—`MILESTONES.md` owns detailed ordered tasks, checkbox acceptance criteria, dependencies, test-mode tags, E2E scenarios, and phase gates, while `MVP-PLAN.md` remains strategic.
14. **Native workspace locality:** completed—the initial short-named `domain`, `storage`, `rendering`, `presentation`, `ui`, `platform`, and `ordo` crates enforce the major dependency directions; `tools/ui-lab` provides an internal interactive and automated primitive proving ground; later crates must earn extraction through a deep Interface.
15. **Agent-development system:** completed—the production repository owns its implementation documentation and agent guidance; a small external Rust/GPUI skill baseline complements focused repository skills created only from proven repetition; `cargo xtask` is the initial offline Ordo-aware inspection and verification surface.
16. **Canonical Slides and reusable content:** completed for behavior—the fixed semantic Slide templates, bounded blank fades, Preparation appearance preference, multi-view Library, plan-level reusable-source bindings, reviewed promotion/archive rules, General Presentation auto-play, live insertion, Reading reuse review, Song Markdown authoring, and migration-owned ERD are adopted. Exact projected type sizes remain intentionally provisional until the Canonical Slide Layout Gallery is qualified.

## Completed Mass Planning interview record

The Mass Planning branches below are finalized for the MVP; no unresolved queue remains here:

1. **Mass outline header:** completed—its title and **+ Add Mass item** are the only MVP actions; an outline-wide `...` menu is deliberately omitted under YAGNI.
2. **Mass-item interactions:** completed—single selection, unrestricted handle/keyboard reordering, one shared row/context menu, explicit non-destructive removal wording, autosave, and Undo are adopted; multi-selection remains deferred.
3. **Inspector ownership:** completed—the ambiguous Inspector `...` is removed; canonical-slide navigation, warning state, Theme override, and contextual **Fix pagination** remain explicit, while outline and editor actions stay on their owning surfaces.
4. **Readiness and warnings:** completed—the header exposes explicit incomplete/warning counts; a floating Readiness panel groups computed findings, navigates or focuses corrective controls, and resolves automatically; incomplete output requires acknowledgment but only snapshot-level fatal failure blocks Presenter Mode.
5. **Mass Theme control:** completed—a viewport-bounded floating chooser previews and stages a parish Theme; a Laravel Cloud-style nested slide-over reviews `current override → inherited result`, and one commit applies the Theme choice and staged override removals coherently. **Edit selected theme** routes to the adopted dedicated Theme Editor and preserves staged chooser context on return.
6. **Mass metadata and templates:** completed for the current scope—a language-neutral Celebration may own multiple independent scheduled Mass plans, including anticipated plans on the preceding civil day and several plans in one language; conditional items, typed Song/Reading slots, multiple templates per language, **Masses → Templates** ownership, independently copied structure with retained reusable-content links, visible applicability-based template recommendation with one fallback per language, independently authored language-specific liturgical content, language-aware duplication, a two-section Mass-details slide-over, a Celebration-grouped chronological index, optional two-stage Celebration/first-plan creation, minimal metadata without liturgical grade/rank, lightweight Presider/Location input with non-retroactive parish defaults and regular-Presider suggestions, and non-destructive shared-edit review are adopted.
7. **Selected-item editor:** completed for the current scope—the Song editor's plan/library ownership boundary, explicit Mass-local customization state, independent copy boundary, linked repeated-section semantics, guarded replacement behavior, expandable section-occurrence/generated-Cue hierarchy, optional Song-level identification Cue, linked reusable Response-sequence exchange blocks, consistent autosave/Undo behavior, and owner-directed pagination correction are adopted.
8. **Mass-section grouping:** completed—the six-section default scaffold is editable and independently copied; sections use UUID-plus-label identity without inferred liturgical behavior, have informative authoring-only collapse, explicit insertion targets, hierarchical reordering, scoped header actions, non-cascading removal, unique nonblank names, and a minimum one-section/exactly-one-membership invariant.
9. **Inspector Cue navigation:** completed—the selected Mass item's flattened Cue order drives a synchronized Inspector; projectable Cues show canonical output, operator-only triggers show black with private context outside the frame, and navigation stops at item boundaries.
10. **Presenter handoff:** completed—the exact **Present** action, automatic candidate validation and snapshot reuse/creation, clean/warning direct entry, centered incomplete-content acknowledgment, fatal blocking, output-disabled entry, interrupted recovery, and safe planned exit/return behavior are adopted.

## Working domain language

- **Ordo:** The ordered structure of one prepared Mass. Context must distinguish this domain object from the product name, Ordo.
- **Celebration:** The dated liturgical occasion and its calendar metadata.
- **Mass plan:** A parish-specific prepared instance for a Celebration, built from a template and containing ordered items.
- **Reuse for another Celebration:** Creation of a new, independent Mass plan from an existing plan, assigned to a different Celebration and date while leaving the source plan unchanged.
- **Mass item:** A top-level titled unit in the plan, such as an entrance song, Greeting, reading, Psalm, response sequence, announcement, or blackout/hold.
- **Mass section:** An ordered named grouping of Mass items representing a large-scale phase of the prepared Ordo. It has no Cue or projectable output of its own and is distinct from a Song's Arrangement section.
- **Cue:** An ordered operator-relevant event within a Mass item. A cue may or may not produce congregation output.
- **Wait Cue:** An operator-relevant Cue that deliberately leaves Program unchanged.
- **Operator-only trigger:** A priest/minister phrase or action used to time the next projected response.
- **Canonical logical slide canvas:** The fixed `1920x1080` coordinate space in which every congregation-facing slide is composed and text is wrapped before resolution-independent scaling.
- **Projectable output / slide:** A rendered 16:9 congregation-facing frame.
- **Program:** Exactly what the congregation display currently shows.
- **Preview:** The operator's selected current/upcoming rendered output.
- **Congregation output disabled:** A Presenter Mode state in which no congregation-output window exists while operator Preview and navigation remain available; it is distinct from Blackout.
- **Freeze:** Hold Program while allowing Preview navigation.
- **Blackout:** Force Program to black while allowing Preview navigation.
- **Song work:** The canonical identity of a song independent of choir, arrangement, language version, or liturgical use.
- **Merged Song redirect (post-MVP):** The retained identity of a Song work retired by a reviewed merge, resolving its old UUID, titles, and aliases to the current survivor without remaining independently selectable.
- **Occasion applicability tag:** Manually reviewed metadata indicating that a Song is suitable for or conventionally used during a liturgical season or special occasion. It is not a claim that the Song's numbered Mass label inherently denotes that season.
- **Arrangement:** An independent choir- or season-specific rendition of a Song work, containing its own reusable sections, performance sequence, reviewed pagination, and applicability metadata.
- **Source reference:** Informational provenance for parish content, such as its origin, attribution, and retrieval date; it does not lock the editable parish copy.
- **Canonical rendered frame:** The single composition produced from the canonical logical slide canvas and shared—only scaled or rerendered at a different pixel density—by the timeline thumbnail, Preview, Program, congregation display, and image-based fallback export.
- **Mass-local override:** A live correction stored only with the current Mass plan until explicitly promoted through post-Mass review.
- **Reusable-source binding:** The plan-level association that keeps every still-linked occurrence of one reusable Library source on the same Edit revision.
- **General Presentation:** A reusable constrained Text or Image sequence intended for announcements and other non-liturgical congregation output.
- **Auto-play scope:** One explicitly started General Presentation item or Mass section whose eligible Slides loop until manual control resumes.
- **Song import document:** A versioned JSON representation of one canonical Song work, its reusable sections, metadata, and one or more Arrangements.
- **Liturgy Import Document:** A versioned, source-neutral representation of externally prepared content for one Celebration, validated and previewed before it is copied into a Mass plan.
- **Live Control:** The operator-facing presentation screen shown inside the existing main window while Presenter Mode is active; it is not a separate operating-system window.
- **Presenter Mode:** The native-full-screen presentation state that transforms the existing main operator window into Live Control and may own one separate congregation-output window.
- **Presentation snapshot:** An immutable captured version of one Mass plan used for a Presenter Mode session or as one member of a Celebration Package.
- **Presentation session:** One live operator run of a Presentation snapshot, with its own Program, Preview, Freeze, Blackout, and recovery state.
- **Edit revision:** A restorable historical state of a Mass plan or reusable library entry; restoring one creates a later revision rather than deleting history.
- **Celebration Package:** A self-contained portable bundle containing one complete Celebration, one independent Presentation snapshot per included Mass plan, and all offline dependencies required to present them.
- **Parish Backup:** A complete portable snapshot used to restore or migrate a parish's Ordo data, settings, and media.

## Explicitly deferred from the MVP

- Multi-parish cloud tenancy and public self-service onboarding.
- Private same-parish cloud synchronization across parish installations.
- Full support for funerals, weddings, baptisms, novenas, and other special rites.
- Rehearsal management and choir-member proficiency tracking.
- Song-usage history and analytics.
- Separate granular staff roles and approval permissions.
- Automated recommendation engines.
- A first-class Mass-setting aggregate that groups related Ordinary movements and prefills them as a suite.
- Remote phone control.
- Audio playback as a core presenter feature.
- Song merging, retiring-Song redirects, merge History and reversal, and the associated conflict/dependency workflow.

## Grilling outcome

- All consequential functional MVP branches identified during the original interview are resolved.
- Song Library, Arrangement Editor, Mass Planning, Mass-section grouping, and Presenter handoff design are complete for the MVP. Celebration-scoped portable transfer now supports repeated self-contained preparation-to-church-to-preparation round trips while imported content remains ordinary local content; export edge cases are next.
- Deliberately deferred post-MVP topics remain listed above and do not block implementation planning.
- `MVP-PLAN.md` is finalized as the implementation-planning baseline.
