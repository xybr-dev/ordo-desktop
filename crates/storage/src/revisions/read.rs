//! Reads of the revision chain and verified revision content.
//!
//! Reads run in one deferred transaction and never mutate the database.
//! Content is re-hashed on every read; a mismatch or an unreadable document is
//! reported as `RevisionIntegrity`, never repaired.

use std::collections::HashMap;
use std::str::FromStr;

use domain::identity::{ArrangementId, RevisionId, SongId, Timestamp};
use domain::presentation::ContentHash;
use domain::revisions::{RevisionEntity, RevisionKind};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::worker::Core;
use crate::{RecordKind, StorageError, map_error};

use super::RevisionRecord;
use super::content::{self, RevisionContent};

/// A stable error for a stored value the store cannot parse.
fn corrupt(message: &'static str) -> StorageError {
    StorageError::Sqlite(message.to_owned())
}

fn parse<T: FromStr>(text: &str, message: &'static str) -> Result<T, StorageError> {
    text.parse().map_err(|_| corrupt(message))
}

/// One `edit_revision` metadata row as stored.
type MetaRow = (
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
);

/// Builds a record from a stored row. A defect in any other column is
/// `RevisionIntegrity` for the row, because its identity is known; only an
/// unreadable id is a generic stored-value error.
fn meta_record(entity: RevisionEntity, row: MetaRow) -> Result<RevisionRecord, StorageError> {
    let (id, parent, kind, session, restored_from, sha256, created_at) = row;
    let id: RevisionId = parse(&id, "corrupt revision id")?;
    let damaged = || StorageError::RevisionIntegrity { id };
    Ok(RevisionRecord {
        id,
        entity,
        parent: parent
            .as_deref()
            .map(|text| text.parse().map_err(|_| damaged()))
            .transpose()?,
        kind: RevisionKind::parse(&kind).ok_or_else(damaged)?,
        session: session
            .as_deref()
            .map(|text| text.parse().map_err(|_| damaged()))
            .transpose()?,
        restored_from: restored_from
            .as_deref()
            .map(|text| text.parse().map_err(|_| damaged()))
            .transpose()?,
        content_sha256: sha256.parse::<ContentHash>().map_err(|_| damaged())?,
        created_at: Timestamp::parse(&created_at).map_err(|_| damaged())?,
    })
}

fn meta_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MetaRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
    ))
}

/// The revision with no child, if the entity has any revision at all.
///
/// The query filters by the entity's own column, so a revision of another
/// entity can never be reported. Two heads mean a broken chain.
pub(crate) fn latest_in(
    tx: &Transaction<'_>,
    entity: RevisionEntity,
) -> Result<Option<RevisionId>, StorageError> {
    let (sql, key) = match entity {
        RevisionEntity::Song(id) => (
            "SELECT id FROM edit_revision WHERE song_id = ?1 AND NOT EXISTS \
             (SELECT 1 FROM edit_revision child WHERE child.parent_id = edit_revision.id) \
             ORDER BY id LIMIT 2",
            id.to_string(),
        ),
        RevisionEntity::Arrangement(id) => (
            "SELECT id FROM edit_revision WHERE arrangement_id = ?1 AND NOT EXISTS \
             (SELECT 1 FROM edit_revision child WHERE child.parent_id = edit_revision.id) \
             ORDER BY id LIMIT 2",
            id.to_string(),
        ),
    };
    let mut statement = tx.prepare(sql).map_err(map_error)?;
    let rows = statement
        .query_map(params![key], |row| row.get::<_, String>(0))
        .map_err(map_error)?;
    let mut heads: Vec<RevisionId> = Vec::new();
    for row in rows {
        let text = row.map_err(map_error)?;
        heads.push(parse(&text, "corrupt revision id")?);
    }
    match heads.as_slice() {
        [] => Ok(None),
        [only] => Ok(Some(*only)),
        [first, ..] => Err(StorageError::RevisionIntegrity { id: *first }),
    }
}

/// Reads, hashes and decodes one revision's content.
pub(crate) fn load_content(
    tx: &Transaction<'_>,
    id: RevisionId,
) -> Result<(RevisionEntity, RevisionContent), StorageError> {
    let row = tx
        .query_row(
            "SELECT song_id, arrangement_id, content, content_sha256 FROM edit_revision \
             WHERE id = ?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(map_error)?;
    let Some((song, arrangement, text, sha256)) = row else {
        return Err(StorageError::NotFound {
            kind: RecordKind::Revision,
            id: id.entity(),
        });
    };
    let damaged = || StorageError::RevisionIntegrity { id };
    let stored = sha256.parse::<ContentHash>().map_err(|_| damaged())?;
    if content::hash_of(&text) != stored {
        return Err(damaged());
    }
    let (entity, decoded) = match (song, arrangement) {
        (Some(song), None) => {
            let song = song.parse::<SongId>().map_err(|_| damaged())?;
            (
                RevisionEntity::Song(song),
                RevisionContent::Song(content::decode_song(&text, id)?),
            )
        }
        (None, Some(arrangement)) => {
            let arrangement = arrangement
                .parse::<ArrangementId>()
                .map_err(|_| damaged())?;
            (
                RevisionEntity::Arrangement(arrangement),
                RevisionContent::Arrangement(content::decode_arrangement(&text, id)?),
            )
        }
        _ => return Err(damaged()),
    };
    // Strict decoding: the stored bytes must be exactly what this build writes
    // for the decoded content. serde alone accepts a missing optional key,
    // reordered keys, whitespace and unsorted sets; none of those is canonical.
    if content::encode(&decoded)?.text != text {
        return Err(damaged());
    }
    Ok((entity, decoded))
}

/// Reads one entity's chain, oldest first, following child links.
fn chain(
    tx: &Transaction<'_>,
    entity: RevisionEntity,
) -> Result<Vec<RevisionRecord>, StorageError> {
    let (sql, key) = match entity {
        RevisionEntity::Song(id) => (
            "SELECT id, parent_id, kind, session_id, restored_from_id, content_sha256, \
             created_at FROM edit_revision WHERE song_id = ?1",
            id.to_string(),
        ),
        RevisionEntity::Arrangement(id) => (
            "SELECT id, parent_id, kind, session_id, restored_from_id, content_sha256, \
             created_at FROM edit_revision WHERE arrangement_id = ?1",
            id.to_string(),
        ),
    };
    let mut statement = tx.prepare(sql).map_err(map_error)?;
    let rows = statement
        .query_map(params![key], meta_row)
        .map_err(map_error)?;
    let mut records = Vec::new();
    for row in rows {
        records.push(meta_record(entity, row.map_err(map_error)?)?);
    }
    let Some(first) = records.first().map(|record| record.id) else {
        return Ok(Vec::new());
    };
    let broken = || StorageError::RevisionIntegrity { id: first };
    let total = records.len();
    // Exactly one root, and each revision the parent of at most one other.
    // Walking child links from the root must then visit every row once; a gap,
    // a cycle or a stray branch leaves rows unvisited and is reported.
    let mut root = None;
    let mut child_of: HashMap<RevisionId, usize> = HashMap::with_capacity(total);
    for (index, record) in records.iter().enumerate() {
        match record.parent {
            None => {
                if root.replace(index).is_some() {
                    return Err(broken());
                }
            }
            Some(parent) => {
                if child_of.insert(parent, index).is_some() {
                    return Err(broken());
                }
            }
        }
    }
    let mut slots: Vec<Option<RevisionRecord>> = records.into_iter().map(Some).collect();
    let mut ordered: Vec<RevisionRecord> = Vec::with_capacity(total);
    let mut next = root.ok_or_else(broken)?;
    loop {
        let record = slots
            .get_mut(next)
            .and_then(Option::take)
            .ok_or_else(broken)?;
        let id = record.id;
        ordered.push(record);
        match child_of.get(&id) {
            Some(&child) => next = child,
            None => break,
        }
    }
    if ordered.len() != total {
        return Err(broken());
    }
    Ok(ordered)
}

impl Core {
    /// Reads one entity's revisions, oldest first. An entity with no revision
    /// (a T2.1-era row) yields an empty list.
    pub(crate) fn revisions(
        &mut self,
        entity: RevisionEntity,
    ) -> Result<Vec<RevisionRecord>, StorageError> {
        let conn = self.connection_mut()?;
        let tx = conn.transaction().map_err(map_error)?;
        chain(&tx, entity)
    }

    /// Reads one revision's content after re-hashing it.
    pub(crate) fn revision_content(
        &mut self,
        revision: RevisionId,
    ) -> Result<RevisionContent, StorageError> {
        let conn = self.connection_mut()?;
        let tx = conn.transaction().map_err(map_error)?;
        load_content(&tx, revision).map(|(_, content)| content)
    }
}
