//! Consistent reads for parish records.
//!
//! Every multi-query read runs inside one deferred transaction, so a caller
//! sees one committed state even while another connection writes. Reads never
//! open a write transaction and never mutate the database.

use std::str::FromStr;

use domain::identity::{ArrangementId, ChoirId, SongId, Timestamp};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::worker::Core;
use crate::{RecordKind, StorageError, map_error};

use super::records::{
    ArrangementRecord, ChoirRecord, OccurrenceRecord, ParishRecord, SectionRecord, SongRecord,
};

/// A stable error for a stored value the store cannot parse.
fn corrupt(column: &'static str) -> StorageError {
    StorageError::Sqlite(column.to_owned())
}

/// Parses one stored identity without panicking on corrupt text.
fn entity<T: FromStr>(text: &str, column: &'static str) -> Result<T, StorageError> {
    text.parse().map_err(|_| corrupt(column))
}

/// Parses one stored timestamp without panicking on corrupt text.
fn time(text: &str, column: &'static str) -> Result<Timestamp, StorageError> {
    Timestamp::parse(text).map_err(|_| corrupt(column))
}

/// Reads one text column for all rows of a query, in the query's order.
fn texts(tx: &Transaction<'_>, sql: &'static str, id: &str) -> Result<Vec<String>, StorageError> {
    let mut statement = tx.prepare(sql).map_err(map_error)?;
    let rows = statement
        .query_map(params![id], |row| row.get::<_, String>(0))
        .map_err(map_error)?;
    let mut values = Vec::new();
    for row in rows {
        values.push(row.map_err(map_error)?);
    }
    Ok(values)
}

fn read_parish(conn: &mut Connection) -> Result<Option<ParishRecord>, StorageError> {
    let tx = conn.transaction().map_err(map_error)?;
    let row = tx
        .query_row(
            "SELECT id, name, created_at FROM parish LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(map_error)?;
    let Some((id, name, created_at)) = row else {
        return Ok(None);
    };
    Ok(Some(ParishRecord {
        id: entity(&id, "corrupt id")?,
        name,
        created_at: time(&created_at, "corrupt created_at")?,
    }))
}

fn read_choirs(conn: &mut Connection) -> Result<Vec<ChoirRecord>, StorageError> {
    let tx = conn.transaction().map_err(map_error)?;
    let mut statement = tx
        .prepare("SELECT id, name, created_at, archived_at FROM choir ORDER BY created_at, id")
        .map_err(map_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(map_error)?;
    let mut choirs = Vec::new();
    for row in rows {
        let (id, name, created_at, archived_at) = row.map_err(map_error)?;
        choirs.push(ChoirRecord {
            id: entity(&id, "corrupt id")?,
            name,
            created_at: time(&created_at, "corrupt created_at")?,
            archived_at: match archived_at {
                Some(text) => Some(time(&text, "corrupt archived_at")?),
                None => None,
            },
        });
    }
    Ok(choirs)
}

fn read_song(tx: &Transaction<'_>, id: SongId) -> Result<Option<SongRecord>, StorageError> {
    let row = tx
        .query_row(
            "SELECT title, original_attribution, provenance, rights_notes, created_at, \
             archived_at FROM song WHERE id = ?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()
        .map_err(map_error)?;
    let Some((title, original_attribution, provenance, rights_notes, created_at, archived_at)) =
        row
    else {
        return Ok(None);
    };
    let id_text = id.to_string();
    let aliases = texts(
        tx,
        "SELECT alias FROM song_alias WHERE song_id = ?1 ORDER BY position",
        &id_text,
    )?;
    let liturgical_moments = texts(
        tx,
        "SELECT moment FROM song_liturgical_moment WHERE song_id = ?1 ORDER BY moment",
        &id_text,
    )?;
    let occasions = texts(
        tx,
        "SELECT occasion FROM song_occasion WHERE song_id = ?1 ORDER BY occasion",
        &id_text,
    )?;
    Ok(Some(SongRecord {
        id,
        title,
        aliases,
        original_attribution,
        provenance,
        rights_notes,
        liturgical_moments,
        occasions,
        created_at: time(&created_at, "corrupt created_at")?,
        archived_at: match archived_at {
            Some(text) => Some(time(&text, "corrupt archived_at")?),
            None => None,
        },
    }))
}

fn read_songs(conn: &mut Connection) -> Result<Vec<SongRecord>, StorageError> {
    let tx = conn.transaction().map_err(map_error)?;
    let ids = {
        let mut statement = tx
            .prepare("SELECT id FROM song ORDER BY created_at, id")
            .map_err(map_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(map_error)?;
        let mut ids = Vec::new();
        for row in rows {
            let text = row.map_err(map_error)?;
            ids.push(entity::<SongId>(&text, "corrupt id")?);
        }
        ids
    };
    let mut songs = Vec::with_capacity(ids.len());
    for id in ids {
        let song = read_song(&tx, id)?.ok_or(StorageError::NotFound {
            kind: RecordKind::Song,
            id: id.entity(),
        })?;
        songs.push(song);
    }
    Ok(songs)
}

fn read_arrangement(
    tx: &Transaction<'_>,
    id: ArrangementId,
) -> Result<Option<ArrangementRecord>, StorageError> {
    let row = tx
        .query_row(
            "SELECT song_id, title, rendition_attribution, created_at, archived_at \
             FROM arrangement WHERE id = ?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(map_error)?;
    let Some((song_id, title, rendition_attribution, created_at, archived_at)) = row else {
        return Ok(None);
    };
    let id_text = id.to_string();
    let languages = texts(
        tx,
        "SELECT language FROM arrangement_language WHERE arrangement_id = ?1 ORDER BY position",
        &id_text,
    )?;
    let mut choirs = Vec::new();
    {
        let mut statement = tx
            .prepare(
                "SELECT choir_id FROM arrangement_choir WHERE arrangement_id = ?1 \
                 ORDER BY choir_id",
            )
            .map_err(map_error)?;
        let rows = statement
            .query_map(params![id_text], |row| row.get::<_, String>(0))
            .map_err(map_error)?;
        for row in rows {
            let text = row.map_err(map_error)?;
            choirs.push(entity::<ChoirId>(&text, "corrupt id")?);
        }
    }
    let occasions = texts(
        tx,
        "SELECT occasion FROM arrangement_occasion WHERE arrangement_id = ?1 ORDER BY occasion",
        &id_text,
    )?;
    let mut sections = Vec::new();
    {
        let mut statement = tx
            .prepare(
                "SELECT id, label, lyrics, created_at FROM arrangement_section \
                 WHERE arrangement_id = ?1 ORDER BY position",
            )
            .map_err(map_error)?;
        let rows = statement
            .query_map(params![id_text], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(map_error)?;
        for row in rows {
            let (section_id, label, lyrics, section_created) = row.map_err(map_error)?;
            sections.push(SectionRecord {
                id: entity(&section_id, "corrupt id")?,
                label,
                lyrics,
                created_at: time(&section_created, "corrupt created_at")?,
            });
        }
    }
    let mut sequence = Vec::new();
    {
        let mut statement = tx
            .prepare(
                "SELECT id, section_id, created_at FROM section_occurrence \
                 WHERE arrangement_id = ?1 ORDER BY position",
            )
            .map_err(map_error)?;
        let rows = statement
            .query_map(params![id_text], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(map_error)?;
        for row in rows {
            let (occurrence_id, section_id, occurrence_created) = row.map_err(map_error)?;
            sequence.push(OccurrenceRecord {
                id: entity(&occurrence_id, "corrupt id")?,
                section: entity(&section_id, "corrupt section_id")?,
                created_at: time(&occurrence_created, "corrupt created_at")?,
            });
        }
    }
    Ok(Some(ArrangementRecord {
        id,
        song: entity(&song_id, "corrupt song_id")?,
        title,
        languages,
        choirs,
        rendition_attribution,
        occasions,
        sections,
        sequence,
        created_at: time(&created_at, "corrupt created_at")?,
        archived_at: match archived_at {
            Some(text) => Some(time(&text, "corrupt archived_at")?),
            None => None,
        },
    }))
}

fn arrangement_by_id(
    conn: &mut Connection,
    id: ArrangementId,
) -> Result<ArrangementRecord, StorageError> {
    let tx = conn.transaction().map_err(map_error)?;
    read_arrangement(&tx, id)?.ok_or(StorageError::NotFound {
        kind: RecordKind::Arrangement,
        id: id.entity(),
    })
}

fn song_by_id(conn: &mut Connection, id: SongId) -> Result<SongRecord, StorageError> {
    let tx = conn.transaction().map_err(map_error)?;
    read_song(&tx, id)?.ok_or(StorageError::NotFound {
        kind: RecordKind::Song,
        id: id.entity(),
    })
}

fn arrangements_for_song(
    conn: &mut Connection,
    song: SongId,
) -> Result<Vec<ArrangementRecord>, StorageError> {
    let tx = conn.transaction().map_err(map_error)?;
    let found = tx
        .query_row(
            "SELECT 1 FROM song WHERE id = ?1",
            params![song.to_string()],
            |_row| Ok(()),
        )
        .optional()
        .map_err(map_error)?
        .is_some();
    if !found {
        return Err(StorageError::NotFound {
            kind: RecordKind::Song,
            id: song.entity(),
        });
    }
    let ids = {
        let mut statement = tx
            .prepare("SELECT id FROM arrangement WHERE song_id = ?1 ORDER BY created_at, id")
            .map_err(map_error)?;
        let rows = statement
            .query_map(params![song.to_string()], |row| row.get::<_, String>(0))
            .map_err(map_error)?;
        let mut ids = Vec::new();
        for row in rows {
            let text = row.map_err(map_error)?;
            ids.push(entity::<ArrangementId>(&text, "corrupt id")?);
        }
        ids
    };
    let mut arrangements = Vec::with_capacity(ids.len());
    for id in ids {
        let arrangement = read_arrangement(&tx, id)?.ok_or(StorageError::NotFound {
            kind: RecordKind::Arrangement,
            id: id.entity(),
        })?;
        arrangements.push(arrangement);
    }
    Ok(arrangements)
}

impl Core {
    /// Reads the Parish, if one has been created.
    pub(crate) fn parish(&mut self) -> Result<Option<ParishRecord>, StorageError> {
        let conn = self.connection_mut()?;
        read_parish(conn)
    }

    /// Reads every Choir ordered by creation time and identity.
    pub(crate) fn choirs(&mut self) -> Result<Vec<ChoirRecord>, StorageError> {
        let conn = self.connection_mut()?;
        read_choirs(conn)
    }

    /// Reads one Song or reports `NotFound`.
    pub(crate) fn song(&mut self, id: SongId) -> Result<SongRecord, StorageError> {
        let conn = self.connection_mut()?;
        song_by_id(conn, id)
    }

    /// Reads every Song ordered by creation time and identity.
    pub(crate) fn songs(&mut self) -> Result<Vec<SongRecord>, StorageError> {
        let conn = self.connection_mut()?;
        read_songs(conn)
    }

    /// Reads one Arrangement or reports `NotFound`.
    pub(crate) fn arrangement(
        &mut self,
        id: ArrangementId,
    ) -> Result<ArrangementRecord, StorageError> {
        let conn = self.connection_mut()?;
        arrangement_by_id(conn, id)
    }

    /// Reads a Song's Arrangements ordered by creation time and identity.
    pub(crate) fn arrangements_for_song(
        &mut self,
        song: SongId,
    ) -> Result<Vec<ArrangementRecord>, StorageError> {
        let conn = self.connection_mut()?;
        arrangements_for_song(conn, song)
    }
}
