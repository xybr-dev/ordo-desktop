//! The atomic import commit (T2.10).
//!
//! An approved batch is every mutation of one review: new Choirs, new Songs,
//! new Arrangements and whole-Arrangement replacements. All of it is written
//! in one immediate transaction, or none of it is. The identities are the
//! import's, so an id that is already taken refuses the batch instead of being
//! regenerated, and every fact the review relied on (the target is present,
//! not archived, still at the head that was shown) is checked again inside the
//! transaction, where no other writer can move it.

use domain::identity::{ArrangementId, ChoirId, EntityId, RevisionId, SongId, Timestamp};
use domain::revisions::{RevisionEntity, RevisionKind};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::revisions::{self, ArrangementContent, RevisionContent, RevisionRecord, SongContent};
use crate::worker::Core;
use crate::{RecordKind, StorageError, map_error};

use super::read::{read_arrangement, read_choir, read_song};
use super::records::{ArrangementRecord, ChoirRecord, NewChoir, NewSong, SongRecord};
use super::write::{
    ArrangementFields, WriteSteps, insert_arrangement_links, insert_arrangement_row,
    insert_choir_languages, insert_choir_row, insert_occurrence_row, insert_section_row,
    insert_song_links, insert_song_row, timestamp_text, validate_choir_fields, validate_song,
};

/// One new Arrangement of the batch, with the identity the document gave it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedArrangement {
    /// The identity the Arrangement keeps; never regenerated.
    pub id: ArrangementId,
    /// The Song it belongs to: stored already, or new in this batch.
    pub song: SongId,
    /// Its metadata, sections (with their pagination states) and Sequence.
    pub content: ArrangementContent,
}

/// One existing Arrangement whose whole content the batch replaces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrangementReplacement {
    /// The stored Arrangement.
    pub id: ArrangementId,
    /// The content it takes, with its section and occurrence identities.
    pub content: ArrangementContent,
    /// The head revision the review showed. `None` means it had no history
    /// (written before revisions existed) and gets a `creation` revision of
    /// its stored content first.
    pub expected_parent: Option<RevisionId>,
}

/// Every mutation of one approved import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportBatch {
    /// New Choirs with their identities.
    pub choirs: Vec<(ChoirId, NewChoir)>,
    /// New Songs with their identities.
    pub songs: Vec<(SongId, NewSong)>,
    /// New Arrangements with their identities.
    pub arrangements: Vec<ImportedArrangement>,
    /// Existing Arrangements to replace.
    pub replacements: Vec<ArrangementReplacement>,
    /// The kind the batch asks for on each new entity's first revision:
    /// `Import` for a `.ordo` document, `Creation` for a Markdown draft; any
    /// other kind is refused. Migration 0004's `edit_revision` CHECK allows a
    /// revision with no parent only when its kind is `creation`, so both are
    /// stored as `creation` until a migration relaxes it (see
    /// `stored_first_kind`). Replacements are always `replacement`.
    pub first_kind: RevisionKind,
}

/// What the commit wrote, exactly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportCommittedRecords {
    /// The Choirs created, in batch order.
    pub choirs: Vec<ChoirRecord>,
    /// The Songs created, in batch order.
    pub songs: Vec<SongRecord>,
    /// The Arrangements created, then the Arrangements replaced, each as
    /// stored after the commit.
    pub arrangements: Vec<ArrangementRecord>,
    /// Every revision appended, in the order written (a backfilled
    /// `creation` revision comes right before its `replacement`).
    pub revisions: Vec<RevisionRecord>,
}

fn require_free(
    tx: &Transaction<'_>,
    sql: &'static str,
    id: EntityId,
    kind: RecordKind,
) -> Result<(), StorageError> {
    let taken = tx
        .query_row(sql, params![id.to_string()], |_row| Ok(()))
        .optional()
        .map_err(map_error)?
        .is_some();
    if taken {
        return Err(StorageError::IdentityCollision { kind, id });
    }
    Ok(())
}

/// Refuses a target that is missing or archived: `Ok` only when present and
/// live. `sql` selects `archived_at` for the id.
fn require_live(
    tx: &Transaction<'_>,
    sql: &'static str,
    id: EntityId,
    kind: RecordKind,
    field: &'static str,
) -> Result<(), StorageError> {
    let row = tx
        .query_row(sql, params![id.to_string()], |row| {
            row.get::<_, Option<String>>(0)
        })
        .optional()
        .map_err(map_error)?;
    match row {
        None => Err(StorageError::MissingReference { kind, id }),
        Some(Some(_)) => Err(StorageError::InvalidInput {
            field,
            reason: "archived",
        }),
        Some(None) => Ok(()),
    }
}

fn require_choir(tx: &Transaction<'_>, choir: ChoirId) -> Result<(), StorageError> {
    let found = tx
        .query_row(
            "SELECT 1 FROM choir WHERE id = ?1",
            params![choir.to_string()],
            |_row| Ok(()),
        )
        .optional()
        .map_err(map_error)?
        .is_some();
    if found {
        Ok(())
    } else {
        Err(StorageError::MissingReference {
            kind: RecordKind::Choir,
            id: choir.entity(),
        })
    }
}

/// Everything that can be refused without opening a transaction.
fn validate(batch: &ImportBatch) -> Result<(), StorageError> {
    if !matches!(
        batch.first_kind,
        RevisionKind::Creation | RevisionKind::Import
    ) {
        return Err(StorageError::InvalidInput {
            field: "first_kind",
            reason: "not allowed",
        });
    }
    for (_, choir) in &batch.choirs {
        validate_choir_fields(
            &choir.name,
            choir.short_label.as_deref(),
            &choir.languages,
            choir.notes.as_deref(),
        )?;
    }
    for (_, song) in &batch.songs {
        validate_song(song)?;
    }
    for arrangement in &batch.arrangements {
        revisions::validate_arrangement_content(&arrangement.content, true)?;
    }
    revisions::validate_link(RevisionKind::Replacement, None, true)?;
    for replacement in &batch.replacements {
        revisions::validate_arrangement_content(&replacement.content, true)?;
    }
    Ok(())
}

/// The kind a first (parentless) revision is stored with. The schema CHECK
/// `(kind = 'creation') = (parent_id IS NULL)` leaves no other choice today.
fn stored_first_kind(_requested: RevisionKind) -> RevisionKind {
    RevisionKind::Creation
}

fn song_content(song: &NewSong) -> SongContent {
    SongContent {
        title: song.title.clone(),
        aliases: song.aliases.clone(),
        original_attribution: song.original_attribution.clone(),
        provenance: song.provenance.clone(),
        rights_notes: song.rights_notes.clone(),
        liturgical_moments: song.liturgical_moments.clone(),
        occasions: song.occasions.clone(),
    }
}

impl Core {
    /// Writes every mutation of `batch` in one immediate transaction.
    pub(crate) fn commit_import(
        &mut self,
        batch: ImportBatch,
    ) -> Result<ImportCommittedRecords, StorageError> {
        validate(&batch)?;
        let at = self.timestamp();
        timestamp_text(at)?;
        let (conn, ids, fault) = self.write_parts()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_error)?;
        let mut steps = WriteSteps::new(fault);
        let created = timestamp_text(at)?;
        let mut revisions_written: Vec<RevisionRecord> = Vec::new();

        let mut choirs = Vec::with_capacity(batch.choirs.len());
        for (id, choir) in &batch.choirs {
            require_free(
                &tx,
                "SELECT 1 FROM choir WHERE id = ?1",
                id.entity(),
                RecordKind::Choir,
            )?;
            let text = id.to_string();
            insert_choir_row(&tx, &mut steps, &text, choir, &created)?;
            insert_choir_languages(&tx, &mut steps, &text, &choir.languages)?;
            choirs.push(*id);
        }

        let mut songs = Vec::with_capacity(batch.songs.len());
        for (id, song) in &batch.songs {
            require_free(
                &tx,
                "SELECT 1 FROM song WHERE id = ?1",
                id.entity(),
                RecordKind::Song,
            )?;
            let text = id.to_string();
            insert_song_row(&tx, &mut steps, &text, song, &created)?;
            insert_song_links(&tx, &mut steps, &text, song)?;
            revisions_written.push(revisions::append_first(
                &tx,
                ids,
                &mut steps,
                at,
                RevisionEntity::Song(*id),
                stored_first_kind(batch.first_kind),
                &RevisionContent::Song(song_content(song)),
            )?);
            songs.push(*id);
        }

        let mut arrangements = Vec::with_capacity(batch.arrangements.len());
        for imported in &batch.arrangements {
            write_new_arrangement(
                &tx,
                ids,
                &mut steps,
                at,
                &created,
                imported,
                stored_first_kind(batch.first_kind),
                &mut revisions_written,
            )?;
            arrangements.push(imported.id);
        }

        let mut replaced = Vec::with_capacity(batch.replacements.len());
        for replacement in &batch.replacements {
            require_live(
                &tx,
                "SELECT archived_at FROM arrangement WHERE id = ?1",
                replacement.id.entity(),
                RecordKind::Arrangement,
                "arrangement",
            )?;
            let (record, written) = revisions::replace_arrangement_in(
                &tx,
                ids,
                &mut steps,
                at,
                replacement.id,
                &replacement.content,
                replacement.expected_parent,
            )?;
            revisions_written.extend(written);
            replaced.push(record);
        }

        let missing = |kind, id| StorageError::NotFound { kind, id };
        let mut records = ImportCommittedRecords {
            choirs: Vec::with_capacity(choirs.len()),
            songs: Vec::with_capacity(songs.len()),
            arrangements: Vec::with_capacity(arrangements.len() + replaced.len()),
            revisions: revisions_written,
        };
        for id in choirs {
            records
                .choirs
                .push(read_choir(&tx, id)?.ok_or_else(|| missing(RecordKind::Choir, id.entity()))?);
        }
        for id in songs {
            records
                .songs
                .push(read_song(&tx, id)?.ok_or_else(|| missing(RecordKind::Song, id.entity()))?);
        }
        for id in arrangements {
            records.arrangements.push(
                read_arrangement(&tx, id)?
                    .ok_or_else(|| missing(RecordKind::Arrangement, id.entity()))?,
            );
        }
        records.arrangements.extend(replaced);
        tx.commit().map_err(map_error)?;
        Ok(records)
    }
}

#[allow(clippy::too_many_arguments)]
fn write_new_arrangement(
    tx: &Transaction<'_>,
    ids: &mut crate::worker::Sources,
    steps: &mut WriteSteps,
    at: Timestamp,
    created: &str,
    imported: &ImportedArrangement,
    first_kind: RevisionKind,
    written: &mut Vec<RevisionRecord>,
) -> Result<(), StorageError> {
    let content = &imported.content;
    require_free(
        tx,
        "SELECT 1 FROM arrangement WHERE id = ?1",
        imported.id.entity(),
        RecordKind::Arrangement,
    )?;
    require_live(
        tx,
        "SELECT archived_at FROM song WHERE id = ?1",
        imported.song.entity(),
        RecordKind::Song,
        "song",
    )?;
    for choir in &content.choirs {
        require_choir(tx, *choir)?;
    }
    for section in &content.sections {
        require_free(
            tx,
            "SELECT 1 FROM arrangement_section WHERE id = ?1",
            section.id.entity(),
            RecordKind::Section,
        )?;
    }
    for occurrence in &content.sequence {
        require_free(
            tx,
            "SELECT 1 FROM section_occurrence WHERE id = ?1",
            occurrence.id.entity(),
            RecordKind::Occurrence,
        )?;
    }

    let text = imported.id.to_string();
    insert_arrangement_row(
        tx,
        steps,
        &text,
        &imported.song.to_string(),
        ArrangementFields {
            title: content.title.as_deref(),
            notes: content.notes.as_deref(),
            rendition_attribution: content.rendition_attribution.as_deref(),
        },
        created,
    )?;
    insert_arrangement_links(
        tx,
        steps,
        &text,
        &content.languages,
        &content.choirs,
        &content.occasions,
    )?;
    for (position, section) in content.sections.iter().enumerate() {
        insert_section_row(
            tx,
            steps,
            &section.id.to_string(),
            &text,
            position,
            section.label.as_deref(),
            &section.lyrics,
            section.pagination,
            created,
        )?;
    }
    for (position, occurrence) in content.sequence.iter().enumerate() {
        insert_occurrence_row(
            tx,
            steps,
            &occurrence.id.to_string(),
            &text,
            &occurrence.section.to_string(),
            position,
            created,
        )?;
    }
    written.push(revisions::append_first(
        tx,
        ids,
        steps,
        at,
        RevisionEntity::Arrangement(imported.id),
        first_kind,
        &RevisionContent::Arrangement(content.clone()),
    )?);
    Ok(())
}
