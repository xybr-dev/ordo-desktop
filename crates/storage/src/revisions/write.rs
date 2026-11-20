//! Durable Edit revisions: commits, restorations and creation revisions.
//!
//! A content replacement and its revision append are one immediate
//! transaction, so any error leaves both content and history unchanged. All
//! SQL is a literal with parameters, run through the shared fault-injecting
//! `write` helper.

use std::collections::HashMap;

use domain::identity::{
    ArrangementId, HistorySessionId, OccurrenceId, RevisionId, SectionId, SongId, Timestamp,
};
use domain::revisions::{RevisionEntity, RevisionKind};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::parish_store::write::{
    WriteSteps, insert_entity, require_distinct, timestamp_text, validate_arrangement_fields,
    validate_song_fields, write,
};
use crate::parish_store::{ArrangementRecord, SongRecord, read_arrangement, read_song};
use crate::worker::{Core, Sources};
use crate::{RecordKind, StorageError, map_error};

use super::RevisionRecord;
use super::content::{self, ArrangementContent, RevisionContent, SongContent};
use super::read::{latest_in, load_content};

/// How a new revision links to the chain.
struct Link {
    /// The latest revision the edit was based on.
    parent: Option<RevisionId>,
    kind: RevisionKind,
    session: Option<HistorySessionId>,
    restored_from: Option<RevisionId>,
}

/// Appends one revision row; the caller has already checked the parent.
fn append_revision(
    tx: &Transaction<'_>,
    ids: &mut Sources,
    steps: &mut WriteSteps,
    at: Timestamp,
    entity: RevisionEntity,
    link: &Link,
    revision: &RevisionContent,
) -> Result<RevisionRecord, StorageError> {
    let created = timestamp_text(at)?;
    let encoded = content::encode(revision)?;
    let (song, arrangement) = match entity {
        RevisionEntity::Song(id) => (Some(id.to_string()), None),
        RevisionEntity::Arrangement(id) => (None, Some(id.to_string())),
    };
    let sha256 = encoded.sha256.to_string();
    let id = insert_entity(ids, steps, RecordKind::Revision, |id, steps| {
        write(
            tx,
            steps,
            "INSERT INTO edit_revision (id, song_id, arrangement_id, parent_id, kind, \
             session_id, restored_from_id, content, content_sha256, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id.to_string(),
                song,
                arrangement,
                link.parent.map(|parent| parent.to_string()),
                link.kind.as_str(),
                link.session.map(|session| session.to_string()),
                link.restored_from.map(|from| from.to_string()),
                encoded.text,
                sha256,
                created
            ],
        )
    })?;
    Ok(RevisionRecord {
        id: RevisionId::from_entity(id),
        entity,
        parent: link.parent,
        kind: link.kind,
        session: link.session,
        restored_from: link.restored_from,
        content_sha256: encoded.sha256,
        created_at: at,
    })
}

/// Appends the creation revision (no parent) inside the caller's transaction.
pub(crate) fn append_creation(
    tx: &Transaction<'_>,
    ids: &mut Sources,
    steps: &mut WriteSteps,
    at: Timestamp,
    entity: RevisionEntity,
    revision: &RevisionContent,
) -> Result<(), StorageError> {
    let link = Link {
        parent: None,
        kind: RevisionKind::Creation,
        session: None,
        restored_from: None,
    };
    append_revision(tx, ids, steps, at, entity, &link, revision).map(|_| ())
}

fn exists(tx: &Transaction<'_>, entity: RevisionEntity) -> Result<bool, StorageError> {
    let (sql, key) = match entity {
        RevisionEntity::Song(id) => ("SELECT 1 FROM song WHERE id = ?1", id.to_string()),
        RevisionEntity::Arrangement(id) => {
            ("SELECT 1 FROM arrangement WHERE id = ?1", id.to_string())
        }
    };
    Ok(tx
        .query_row(sql, params![key], |_row| Ok(()))
        .optional()
        .map_err(map_error)?
        .is_some())
}

fn require_entity(tx: &Transaction<'_>, entity: RevisionEntity) -> Result<(), StorageError> {
    if exists(tx, entity)? {
        return Ok(());
    }
    let (kind, id) = match entity {
        RevisionEntity::Song(id) => (RecordKind::Song, id.entity()),
        RevisionEntity::Arrangement(id) => (RecordKind::Arrangement, id.entity()),
    };
    Err(StorageError::NotFound { kind, id })
}

/// Refuses a commit whose base is not the entity's latest revision.
fn require_current(
    tx: &Transaction<'_>,
    entity: RevisionEntity,
    expected: RevisionId,
) -> Result<(), StorageError> {
    match latest_in(tx, entity)? {
        None => Err(StorageError::InvalidInput {
            field: "expected_parent",
            reason: "entity has no history",
        }),
        Some(latest) if latest != expected => Err(StorageError::StaleRevision {
            entity_latest: latest,
        }),
        Some(_) => Ok(()),
    }
}

/// The kind and session rules, checked before any transaction opens.
/// `Replacement` replaces a whole Arrangement (Decision 140), so a Song
/// commit refuses it.
fn validate_link(
    kind: RevisionKind,
    session: Option<HistorySessionId>,
    allow_replacement: bool,
) -> Result<(), StorageError> {
    if kind == RevisionKind::Replacement && !allow_replacement {
        return Err(StorageError::InvalidInput {
            field: "kind",
            reason: "not allowed",
        });
    }
    match (kind, session) {
        (RevisionKind::Edit, Some(_)) => Ok(()),
        (RevisionKind::Edit, None) => Err(StorageError::InvalidInput {
            field: "session",
            reason: "required",
        }),
        (RevisionKind::Import | RevisionKind::Replacement, None) => Ok(()),
        (RevisionKind::Import | RevisionKind::Replacement, Some(_)) => {
            Err(StorageError::InvalidInput {
                field: "session",
                reason: "not allowed",
            })
        }
        (RevisionKind::Creation | RevisionKind::Restoration, _) => {
            Err(StorageError::InvalidInput {
                field: "kind",
                reason: "not allowed",
            })
        }
    }
}

fn validate_song_content(song: &SongContent) -> Result<(), StorageError> {
    validate_song_fields(
        &song.title,
        &song.aliases,
        song.original_attribution.as_deref(),
        song.provenance.as_deref(),
        song.rights_notes.as_deref(),
        &song.liturgical_moments,
        &song.occasions,
    )
}

fn validate_arrangement_content(arrangement: &ArrangementContent) -> Result<(), StorageError> {
    validate_arrangement_fields(
        &arrangement.title,
        &arrangement.languages,
        &arrangement.choirs,
        arrangement.rendition_attribution.as_deref(),
        &arrangement.occasions,
        arrangement
            .sections
            .iter()
            .map(|section| section.label.as_deref()),
    )?;
    let section_ids: Vec<SectionId> = arrangement.sections.iter().map(|s| s.id).collect();
    require_distinct("section", &section_ids)?;
    let occurrence_ids: Vec<OccurrenceId> = arrangement.sequence.iter().map(|o| o.id).collect();
    require_distinct("occurrence", &occurrence_ids)?;
    for occurrence in &arrangement.sequence {
        if !section_ids.contains(&occurrence.section) {
            return Err(StorageError::InvalidInput {
                field: "sequence",
                reason: "unknown section",
            });
        }
    }
    Ok(())
}

fn commit_song_in(
    tx: &Transaction<'_>,
    ids: &mut Sources,
    steps: &mut WriteSteps,
    at: Timestamp,
    song: SongId,
    revision: &SongContent,
    link: &Link,
) -> Result<(SongRecord, RevisionRecord), StorageError> {
    let entity = RevisionEntity::Song(song);
    let key = song.to_string();

    write(
        tx,
        steps,
        "UPDATE song SET title = ?2, original_attribution = ?3, provenance = ?4, \
         rights_notes = ?5 WHERE id = ?1",
        params![
            key,
            revision.title,
            revision.original_attribution.as_deref(),
            revision.provenance.as_deref(),
            revision.rights_notes.as_deref()
        ],
    )?;
    write(
        tx,
        steps,
        "DELETE FROM song_alias WHERE song_id = ?1",
        params![key],
    )?;
    write(
        tx,
        steps,
        "DELETE FROM song_liturgical_moment WHERE song_id = ?1",
        params![key],
    )?;
    write(
        tx,
        steps,
        "DELETE FROM song_occasion WHERE song_id = ?1",
        params![key],
    )?;
    for (position, alias) in revision.aliases.iter().enumerate() {
        write(
            tx,
            steps,
            "INSERT INTO song_alias (song_id, position, alias) VALUES (?1, ?2, ?3)",
            params![key, position as i64, alias],
        )?;
    }
    for moment in &revision.liturgical_moments {
        write(
            tx,
            steps,
            "INSERT INTO song_liturgical_moment (song_id, moment) VALUES (?1, ?2)",
            params![key, moment],
        )?;
    }
    for occasion in &revision.occasions {
        write(
            tx,
            steps,
            "INSERT INTO song_occasion (song_id, occasion) VALUES (?1, ?2)",
            params![key, occasion],
        )?;
    }
    let record = append_revision(
        tx,
        ids,
        steps,
        at,
        entity,
        link,
        &RevisionContent::Song(revision.clone()),
    )?;
    let stored = read_song(tx, song)?.ok_or(StorageError::NotFound {
        kind: RecordKind::Song,
        id: song.entity(),
    })?;
    Ok((stored, record))
}

/// Existing identities of an Arrangement with their original creation times.
fn created_times(
    tx: &Transaction<'_>,
    sql: &'static str,
    arrangement: &str,
) -> Result<HashMap<String, String>, StorageError> {
    let mut statement = tx.prepare(sql).map_err(map_error)?;
    let rows = statement
        .query_map(params![arrangement], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(map_error)?;
    let mut found = HashMap::new();
    for row in rows {
        let (id, created) = row.map_err(map_error)?;
        found.insert(id, created);
    }
    Ok(found)
}

/// Refuses an identity that already belongs to a different Arrangement.
fn require_not_foreign(
    tx: &Transaction<'_>,
    sql: &'static str,
    id: String,
    arrangement: &str,
    field: &'static str,
) -> Result<(), StorageError> {
    let owner = tx
        .query_row(sql, params![id], |row| row.get::<_, String>(0))
        .optional()
        .map_err(map_error)?;
    match owner {
        Some(owner) if owner != arrangement => Err(StorageError::InvalidInput {
            field,
            reason: "foreign id",
        }),
        _ => Ok(()),
    }
}

fn commit_arrangement_in(
    tx: &Transaction<'_>,
    ids: &mut Sources,
    steps: &mut WriteSteps,
    at: Timestamp,
    arrangement: ArrangementId,
    revision: &ArrangementContent,
    link: &Link,
) -> Result<(ArrangementRecord, RevisionRecord), StorageError> {
    let entity = RevisionEntity::Arrangement(arrangement);
    let key = arrangement.to_string();
    let now = timestamp_text(at)?;

    for choir in &revision.choirs {
        let found = tx
            .query_row(
                "SELECT 1 FROM choir WHERE id = ?1",
                params![choir.to_string()],
                |_row| Ok(()),
            )
            .optional()
            .map_err(map_error)?
            .is_some();
        if !found {
            return Err(StorageError::MissingReference {
                kind: RecordKind::Choir,
                id: choir.entity(),
            });
        }
    }
    for section in &revision.sections {
        require_not_foreign(
            tx,
            "SELECT arrangement_id FROM arrangement_section WHERE id = ?1",
            section.id.to_string(),
            &key,
            "section",
        )?;
    }
    for occurrence in &revision.sequence {
        require_not_foreign(
            tx,
            "SELECT arrangement_id FROM section_occurrence WHERE id = ?1",
            occurrence.id.to_string(),
            &key,
            "occurrence",
        )?;
    }
    let section_times = created_times(
        tx,
        "SELECT id, created_at FROM arrangement_section WHERE arrangement_id = ?1",
        &key,
    )?;
    let occurrence_times = created_times(
        tx,
        "SELECT id, created_at FROM section_occurrence WHERE arrangement_id = ?1",
        &key,
    )?;

    write(
        tx,
        steps,
        "UPDATE arrangement SET title = ?2, rendition_attribution = ?3 WHERE id = ?1",
        params![
            key,
            revision.title,
            revision.rendition_attribution.as_deref()
        ],
    )?;
    write(
        tx,
        steps,
        "DELETE FROM section_occurrence WHERE arrangement_id = ?1",
        params![key],
    )?;
    write(
        tx,
        steps,
        "DELETE FROM arrangement_section WHERE arrangement_id = ?1",
        params![key],
    )?;
    write(
        tx,
        steps,
        "DELETE FROM arrangement_language WHERE arrangement_id = ?1",
        params![key],
    )?;
    write(
        tx,
        steps,
        "DELETE FROM arrangement_choir WHERE arrangement_id = ?1",
        params![key],
    )?;
    write(
        tx,
        steps,
        "DELETE FROM arrangement_occasion WHERE arrangement_id = ?1",
        params![key],
    )?;
    for (position, language) in revision.languages.iter().enumerate() {
        write(
            tx,
            steps,
            "INSERT INTO arrangement_language (arrangement_id, position, language) \
             VALUES (?1, ?2, ?3)",
            params![key, position as i64, language],
        )?;
    }
    for choir in &revision.choirs {
        write(
            tx,
            steps,
            "INSERT INTO arrangement_choir (arrangement_id, choir_id) VALUES (?1, ?2)",
            params![key, choir.to_string()],
        )?;
    }
    for occasion in &revision.occasions {
        write(
            tx,
            steps,
            "INSERT INTO arrangement_occasion (arrangement_id, occasion) VALUES (?1, ?2)",
            params![key, occasion],
        )?;
    }
    for (position, section) in revision.sections.iter().enumerate() {
        let id = section.id.to_string();
        let created = section_times.get(&id).map_or(now.as_str(), String::as_str);
        write(
            tx,
            steps,
            "INSERT INTO arrangement_section (id, arrangement_id, position, label, lyrics, \
             created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                key,
                position as i64,
                section.label.as_deref(),
                section.lyrics,
                created
            ],
        )?;
    }
    for (position, occurrence) in revision.sequence.iter().enumerate() {
        let id = occurrence.id.to_string();
        let created = occurrence_times
            .get(&id)
            .map_or(now.as_str(), String::as_str);
        write(
            tx,
            steps,
            "INSERT INTO section_occurrence (id, arrangement_id, section_id, position, \
             created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                key,
                occurrence.section.to_string(),
                position as i64,
                created
            ],
        )?;
    }
    let record = append_revision(
        tx,
        ids,
        steps,
        at,
        entity,
        link,
        &RevisionContent::Arrangement(revision.clone()),
    )?;
    let stored = read_arrangement(tx, arrangement)?.ok_or(StorageError::NotFound {
        kind: RecordKind::Arrangement,
        id: arrangement.entity(),
    })?;
    Ok((stored, record))
}

impl Core {
    /// Replaces a Song's content and appends the revision in one transaction.
    pub(crate) fn commit_song_edit(
        &mut self,
        song: SongId,
        revision: SongContent,
        kind: RevisionKind,
        session: Option<HistorySessionId>,
        expected_parent: RevisionId,
    ) -> Result<(SongRecord, RevisionRecord), StorageError> {
        validate_link(kind, session, false)?;
        validate_song_content(&revision)?;
        let at = self.timestamp();
        timestamp_text(at)?;
        let (conn, ids, fault) = self.write_parts()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_error)?;
        let mut steps = WriteSteps::new(fault);
        require_entity(&tx, RevisionEntity::Song(song))?;
        require_current(&tx, RevisionEntity::Song(song), expected_parent)?;
        let link = Link {
            parent: Some(expected_parent),
            kind,
            session,
            restored_from: None,
        };
        let done = commit_song_in(&tx, ids, &mut steps, at, song, &revision, &link)?;
        tx.commit().map_err(map_error)?;
        Ok(done)
    }

    /// Replaces an Arrangement's content and appends the revision in one
    /// transaction.
    pub(crate) fn commit_arrangement_edit(
        &mut self,
        arrangement: ArrangementId,
        revision: ArrangementContent,
        kind: RevisionKind,
        session: Option<HistorySessionId>,
        expected_parent: RevisionId,
    ) -> Result<(ArrangementRecord, RevisionRecord), StorageError> {
        validate_link(kind, session, true)?;
        validate_arrangement_content(&revision)?;
        let at = self.timestamp();
        timestamp_text(at)?;
        let (conn, ids, fault) = self.write_parts()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_error)?;
        let mut steps = WriteSteps::new(fault);
        let entity = RevisionEntity::Arrangement(arrangement);
        require_entity(&tx, entity)?;
        require_current(&tx, entity, expected_parent)?;
        let link = Link {
            parent: Some(expected_parent),
            kind,
            session,
            restored_from: None,
        };
        let done = commit_arrangement_in(&tx, ids, &mut steps, at, arrangement, &revision, &link)?;
        tx.commit().map_err(map_error)?;
        Ok(done)
    }

    /// Commits an earlier revision's content as a new `restoration` revision.
    pub(crate) fn restore_revision(
        &mut self,
        revision: RevisionId,
        expected_parent: RevisionId,
    ) -> Result<(RevisionContent, RevisionRecord), StorageError> {
        let (entity, restored) = {
            let conn = self.connection_mut()?;
            let tx = conn.transaction().map_err(map_error)?;
            load_content(&tx, revision)?
        };
        // This content passed validation when it was committed, so a failure
        // now means the stored document was altered, not that the caller
        // supplied bad input.
        let damaged = |_| StorageError::RevisionIntegrity { id: revision };
        match &restored {
            RevisionContent::Song(song) => validate_song_content(song).map_err(damaged)?,
            RevisionContent::Arrangement(arrangement) => {
                validate_arrangement_content(arrangement).map_err(damaged)?;
            }
        }
        let at = self.timestamp();
        timestamp_text(at)?;
        let (conn, ids, fault) = self.write_parts()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_error)?;
        let mut steps = WriteSteps::new(fault);
        require_entity(&tx, entity)?;
        require_current(&tx, entity, expected_parent)?;
        let link = Link {
            parent: Some(expected_parent),
            kind: RevisionKind::Restoration,
            session: None,
            restored_from: Some(revision),
        };
        let record = match (&restored, entity) {
            (RevisionContent::Song(song), RevisionEntity::Song(id)) => {
                commit_song_in(&tx, ids, &mut steps, at, id, song, &link)?.1
            }
            (RevisionContent::Arrangement(arrangement), RevisionEntity::Arrangement(id)) => {
                commit_arrangement_in(&tx, ids, &mut steps, at, id, arrangement, &link)?.1
            }
            _ => return Err(StorageError::RevisionIntegrity { id: revision }),
        };
        tx.commit().map_err(map_error)?;
        Ok((restored, record))
    }

    /// The entity's latest revision, first backfilling a `creation` revision of
    /// its current content when it has none (a T2.1-era row).
    pub(crate) fn latest_revision(
        &mut self,
        entity: RevisionEntity,
    ) -> Result<RevisionId, StorageError> {
        {
            // Reading needs no write lock; only a backfill takes one.
            let conn = self.connection_mut()?;
            let tx = conn.transaction().map_err(map_error)?;
            require_entity(&tx, entity)?;
            if let Some(latest) = latest_in(&tx, entity)? {
                return Ok(latest);
            }
        }
        let at = self.timestamp();
        timestamp_text(at)?;
        let (conn, ids, fault) = self.write_parts()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_error)?;
        // Re-check under the write lock: a backfill may have landed meanwhile.
        require_entity(&tx, entity)?;
        if let Some(latest) = latest_in(&tx, entity)? {
            return Ok(latest);
        }
        let mut steps = WriteSteps::new(fault);
        let current = match entity {
            RevisionEntity::Song(id) => {
                read_song(&tx, id)?.map(|record| RevisionContent::Song(SongContent::of(&record)))
            }
            RevisionEntity::Arrangement(id) => read_arrangement(&tx, id)?
                .map(|record| RevisionContent::Arrangement(ArrangementContent::of(&record))),
        };
        let current = current.ok_or(StorageError::NotFound {
            kind: match entity {
                RevisionEntity::Song(_) => RecordKind::Song,
                RevisionEntity::Arrangement(_) => RecordKind::Arrangement,
            },
            id: match entity {
                RevisionEntity::Song(id) => id.entity(),
                RevisionEntity::Arrangement(id) => id.entity(),
            },
        })?;
        let link = Link {
            parent: None,
            kind: RevisionKind::Creation,
            session: None,
            restored_from: None,
        };
        let record = append_revision(&tx, ids, &mut steps, at, entity, &link, &current)?;
        tx.commit().map_err(map_error)?;
        Ok(record.id)
    }
}
