# Ordo Domain Language

Ordo supports parish teams in preparing and presenting a Catholic Mass while keeping operator context distinct from what the congregation sees.

## Liturgy planning

**Celebration**:
The language-neutral liturgical occasion, its liturgical date, and calendar metadata. One Celebration may have several scheduled Mass plans in the same or different languages, including an anticipated plan on the preceding civil day.

**Mass plan**:
A parish-specific independent preparation for one scheduled Mass under a Celebration, built from a template and containing ordered Mass items. It owns its actual scheduled start, which can differ from the Celebration's liturgical date. Plans under the same Celebration may differ by time, language, choir, or local content.
_Avoid_: Service, presentation file

**Default Mass location**:
An optional parish preference copied into a newly created Mass plan as its initial Location. It is not a live reference and does not change existing plans or replace the saved Location when a plan is duplicated or reused.

**Presider**:
The person recorded on one Mass plan as presiding at that scheduled Mass. A Presider may be one of the parish's regular Presiders or a freely entered guest.
_Avoid_: Celebrant in Ordo field names, clergy account

**Regular Presider**:
A parish-level Presider suggestion kept for quick Mass-plan selection. It is not a clergy directory record and carries no contact, assignment, or canonical-status data.

**Default Presider**:
An optional Regular Presider copied into a newly created Mass plan as its initial Presider. It is not a live reference and never rewrites existing plans.

**Anticipated Mass**:
A Mass plan scheduled on the preceding civil day while belonging to the later Celebration whose liturgical context it uses. It is not a generic synonym for a Vigil with its own distinct liturgical structure.

**Duplicate for another Mass**:
Creation of a new, independent Mass plan under the same Celebration. With the same language it is a complete copy; with a different language it is a target-language derivation rather than a textual copy. Neither outcome retains synchronization with the source plan.
_Avoid_: Reuse for another Celebration

**Target-language derivation**:
A Mass plan created from an existing plan under the same Celebration using an applicable template in another language. It may retain compatible song Arrangements by liturgical role, but language-bound liturgical text remains independently authored.

**Reuse for another Celebration**:
Creation of a new, independent Mass plan from an existing plan, assigned to a different Celebration and date while leaving the source plan unchanged.
_Avoid_: Replicate, changing the source plan's date

**Mass-plan readiness**:
A computed assessment of whether a Mass plan has unresolved presentation dependencies or review concerns; it is not an approval or operator-assigned status.
_Avoid_: Approval status, completion status

**Incomplete Mass plan**:
A Mass plan for which some intended presentation content cannot be faithfully produced because a required dependency is missing, broken, empty, invalid, or unrenderable. Incompleteness requires explicit acknowledgment before presenting, but it is not itself an inability to enter Presenter Mode; unavailable Cues cannot be taken live.
_Avoid_: Blocked Mass plan, rejected Mass

**Presenter-entry blocker**:
A fatal condition that prevents Ordo from capturing or opening any coherent Presentation snapshot. It is distinct from incomplete content within an otherwise coherent plan.
_Avoid_: Missing Arrangement, ordinary readiness warning

**Ordo (Mass order)**:
The ordered structure of a Mass plan as understood and navigated by the operator. Context must distinguish this from the Ordo product.

**Mass item**:
A top-level titled unit in a Mass plan, such as a song, reading, rite, response sequence, announcement, or hold.
_Avoid_: Slide group

**Mass section**:
An independently named grouping of Mass items representing one ordered large-scale phase of the prepared Ordo. Its label carries no inferred liturgical behavior; it owns no Cue or congregation output and is distinct from an Arrangement section.
_Avoid_: Arrangement section, slide group

**Remove from this Mass**:
Removal of one Mass item's placement and plan-local content from a Mass plan. It does not delete any referenced reusable Song work, Arrangement, template, reading-library entry, or other library content.
_Avoid_: Delete Song, delete Arrangement

**Conditional Mass item**:
A template-defined Mass item whose default inclusion depends on Celebration context, while remaining visible and explicitly overridable during preparation.

**Template slot**:
A typed unresolved position in a Mass template that records the expected liturgical role without claiming that the Mass-plan-specific content has been selected. Song slots and Reading slots are kinds of Template slot.

**Template applicability**:
Advisory metadata used with a Mass plan's language and Celebration context to rank reusable Mass templates. It recommends a starting structure but never prohibits deliberate selection of another template.

**Liturgical role**:
A language-neutral description of the purpose fulfilled by authored liturgical content, such as Greeting response or First Reading. Sharing a role does not assert that entries in different languages are equivalent translations.

**Language-specific liturgical content**:
An independently authored response, reading, or other liturgical text for one language. Versions in other languages are separate content and are not kept synchronized.

**Cue**:
An ordered operator-relevant event within a Mass item; it may or may not have congregation-facing output.
_Avoid_: Slide

**Custom sequence**:
A Mass item assembled from an operator-chosen mixture of Cue types when a specialized Song, reading, response, announcement, or control workflow does not fit.
_Avoid_: Blank item

**Operator-only trigger**:
A minister's phrase or action used by the operator to time a following Cue, without itself being projected.

**Exchange block**:
One ordered unit in a Response-sequence item that pairs an optional operator-only trigger with one required congregation response. Presenter navigation encounters the trigger first when present, then the response's projectable Cue or Cues.
_Avoid_: Dialogue slide, arbitrary Cue group

**Mass-owned response sequence**:
A Response-sequence item copied from a Mass template into one Mass plan and edited independently thereafter. Its template reference is provenance, not inheritance, and changes do not synchronize in either direction.
_Avoid_: Shared response, inherited response

**Source reference**:
Informational provenance attached to parish content, such as its origin, attribution, or retrieval date; it does not make the content immutable.

**Content-source extension**:
An optional provider that converts external material into a source-neutral import draft. It does not own parish content after import and is not required to reopen or present that content.
_Avoid_: Canonical source, core liturgy module

## Songs

**Song work**:
The canonical identity of one underlying musical composition or setting, independent of choir, Arrangement, language version, or liturgical use. Similar titles or shared liturgical text do not unite distinct musical settings into one Song work.
_Avoid_: Song version

**Merged Song redirect**:
The retained identity of a Song work retired by a reviewed merge, resolving its original UUID, titles, and aliases to the current surviving Song work without remaining an independently selectable Song.
_Avoid_: Archived Song, deleted Song

**Arrangement**:
An independent choir-, season-, or language-specific rendition of a Song work, with its own reusable sections, performance sequence, pagination, and applicability metadata. Multiple Choirs may share one Arrangement when they use that exact rendition.
_Avoid_: Pagination variant

**Archived Arrangement**:
A recoverably retired Arrangement that remains part of its Song work and continues resolving historical references, but is excluded from ordinary planning and Arrangement selection until restored.
_Avoid_: Deleted Arrangement, inactive Song

**Translated Arrangement**:
A language-specific Arrangement that retains the underlying composition of its Song work while independently owning its translated lyrics, sequence, pagination, translator attribution, and Choir applicability.

**Arrangement section**:
A reusable musical or lyrical unit within one Arrangement, such as Verse 1, Chorus, Refrain, Bridge, or Ending. It may generate one or several projectable slides while remaining one section referenced by the performance sequence.
_Avoid_: Slide, section created only to force a slide split

**Slide boundary**:
A presentation division between lyric phrases or lines inside an Arrangement section. It determines where one projectable slide ends and the next begins without changing the section's musical identity or the performance sequence.
_Avoid_: Section boundary, sequence entry

**Section occurrence**:
One ordered placement of a reusable Song section within an Arrangement or Mass-local performance sequence. Several occurrences may reference the same linked section while occupying distinct Cue positions.
_Avoid_: Duplicated section

**Song-level Cue**:
A Cue owned by one Song item's overall presentation sequence but not by any section occurrence. An identification Cue is Song-level because it precedes the section sequence without becoming a reusable lyric section.
_Avoid_: Song section, Mass item

**Occasion applicability tag**:
Reviewed metadata indicating that a Song work or Arrangement is suitable for or conventionally used during a liturgical season or special occasion; it is advisory rather than restrictive.
_Avoid_: Liturgical rule

**Arrangement filter match**:
One Arrangement that satisfies every active Arrangement-scoped Song Library filter, such as Choir, language, and occasion. A Song work remains a result when it owns at least one such Arrangement; attributes from separate Arrangements are never combined into one match. Missing metadata is **Unspecified**, not universal applicability.
_Avoid_: Song-level match assembled from several Arrangements

## Presentation

**Projectable output**:
A congregation-facing slide composition.
_Avoid_: Cue

**Program**:
Exactly what the congregation display currently shows.

**Preview**:
The projectable output currently selected by the operator for navigation or presentation.

**Congregation output disabled**:
A Presenter Mode state in which no congregation-output window exists, while operator Preview and navigation remain available.
_Avoid_: Rehearsal Mode, Blackout

**Congregation display not assigned**:
A valid installation setting in which no congregation display is selected. It is distinct from a previously assigned display becoming unavailable.
_Avoid_: Unavailable display

**Freeze**:
A presentation state that holds Program while allowing Preview navigation.

**Blackout**:
A presentation state that forces Program to black while allowing Preview navigation.

**Hold**:
An ordinary operator-only checkpoint Cue with black projectable output; unlike Blackout, it does not establish a persistent presentation state.
_Avoid_: Blackout

**Unavailable Cue**:
A Cue whose intended projectable output cannot currently be produced faithfully. It remains inspectable but can never be taken live until corrected.
_Avoid_: Disabled Cue

**Mass-local override**:
A correction belonging only to one Mass plan until explicitly promoted through post-Mass review.

**Live Control**:
The operator-facing presentation screen shown inside Ordo's existing main window while Presenter Mode is active.
_Avoid_: Presenter window, separate operator window

**Presenter Mode**:
The native-full-screen presentation state that transforms Ordo's existing main operator window into Live Control and may own one separate congregation-output window.

**Presentation snapshot**:
An immutable captured version of one Mass plan used for a presentation session, Celebration Package, or emergency export.
_Avoid_: Approved Mass

**Presentation session**:
One live operator run of a Presentation snapshot, with its own navigation, output, and recovery state.
_Avoid_: Presentation snapshot

**Edit revision**:
A restorable historical state of a Mass plan or reusable library entry; restoring it creates a later revision rather than deleting history.

**Theme**:
A reusable congregation-output visual identity inherited by a Mass plan's projectable content unless a Mass item explicitly overrides it.
_Avoid_: Per-slide style

**Mass Theme**:
The Theme selected as the default visual identity for one Mass plan's projectable content.
_Avoid_: Application theme, operator-shell appearance

**Theme override**:
A Mass item's explicit selection of a Theme instead of inheriting its Mass Theme. Clearing the override restores inheritance rather than copying the current Mass Theme onto the item.
_Avoid_: Per-slide formatting

## Portability

**Ordo file**:
An `.ordo` exchange container whose verified manifest identifies a Celebration Package, Parish Backup, Song import document, Liturgy Import Document, or Diagnostic report. The extension indicates compatibility, not provenance or trust.
_Avoid_: Trusted Ordo file, proprietary binary

**Parish scope**:
The private boundary containing one parish's library, plans, packages, backups, installations, and any future synchronized copies. It does not imply public publishing or exchange with other parishes.
_Avoid_: Public cloud, content marketplace

**Celebration Package**:
A self-contained portable bundle of one complete Celebration, an independent Presentation snapshot for every Mass plan included at export time, and all dependencies required to present them offline. The same artifact type carries ordinary local changes in either direction between parish installations.
_Avoid_: Mass Package, Return Package, delta package, Parish Backup

**Installed Celebration Package**:
A verified Celebration Package artifact lineage retained in one Ordo installation's managed storage for provenance, history, rollback, and later transfer; its imported domain records are ordinary editable local content.
_Avoid_: Open USB package, imported Celebration

**Package version**:
One immutable export of a Celebration Package, grouped with other exports by Celebration identity while retaining its exact included plans and Presentation snapshots.
_Avoid_: Duplicate Celebration, editable package

**Library reconciliation**:
A reviewed Celebration Package import process that maps its editable source records into ordinary receiving-parish records while leaving the package's exact Presentation snapshots unchanged.
_Avoid_: Automatic merge, snapshot import

**Sync anchor**:
The latest Edit revision proven to be common to one mapped entity on the source and receiving installations, used to distinguish a safe update from independently changed content.
_Avoid_: Latest timestamp, revision number

**Parish Backup**:
A complete portable snapshot of one Parish scope's authoritative data, settings, media, history, and transfer ancestry. It restores the Parish scope without cloning the source computer's installation identity.
_Avoid_: Celebration Package

**Unavailable artifact**:
A known Celebration Package or Parish Backup whose location cannot currently be accessed, without evidence that its contents are damaged.
_Avoid_: Damaged artifact, corrupt file

**Damaged artifact**:
An accessible Celebration Package or Parish Backup that fails its required integrity or dependency verification and therefore cannot fulfill its portable purpose.
_Avoid_: Unavailable artifact, partially usable archive

**Song import document**:
A versioned structured representation of one Song work and its associated metadata and Arrangements, carried as the `song-import` kind of Ordo file.

**Staged Song import**:
A validated set of proposed Song-library actions derived from Song import documents. Each document resolves one Song-work identity and may contain several independently classified Arrangement actions; the library remains unchanged until all reviewed actions commit together.
_Avoid_: Imported Song before commit, automatic merge

**Invalid import candidate**:
An Ordo file or contained import document that fails its required contract, integrity, dependency closure, or supported-version check and therefore cannot participate in an import commit.
_Avoid_: Needs-decision candidate, warning, force-importable file

**Needs-decision import candidate**:
Structurally valid imported content whose identity or reconciliation outcome requires an operator choice before the staged import can commit.
_Avoid_: Invalid file, corrupt content

**Arrangement import update**:
An explicit import decision that keeps an existing Arrangement's identity while replacing its current reusable content with the staged imported Arrangement as a new restorable revision. It is a whole-Arrangement revision, never a line-level merge, and does not alter Mass-local overrides or immutable Presentation snapshots.
_Avoid_: Automatic merge, new Arrangement variant

**Liturgy Import Document**:
A versioned, source-neutral representation of externally prepared liturgical content for one Celebration, carried as the `liturgy-import` kind of Ordo file and reviewed before it becomes parish content in a Mass plan.
_Avoid_: Sulugoon file

## Operations

**Diagnostics**:
An offline, operator-only assessment of the current Ordo installation's health and environment. It reports findings and routes the operator to their owning workflow but does not repair or mutate parish data.
_Avoid_: Repair tool, telemetry

**Diagnostic report**:
A deliberately exported, sanitized Ordo file containing installation-health findings and troubleshooting evidence without parish presentation content or identifying environment details.
_Avoid_: Telemetry report, Parish Backup, support upload
