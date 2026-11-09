//! Consistent reads for parish records.
//!
//! Every multi-query read runs inside one deferred transaction, so a caller
//! sees one committed state even while another connection writes. Reads never
//! open a write transaction and never mutate the database.

use std::str::FromStr;

use domain::identity::{ArrangementId, ChoirId, ManagedAssetId, SongId, ThemeId, Timestamp};
use domain::presentation::ContentHash;
use domain::song_library::PaginationState;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::assets;
use crate::revisions;
use crate::worker::Core;
use crate::{RecordKind, StorageError, map_error};

use super::records::{
    ArrangementRecord, AssetReport, ChoirRecord, LibraryRecords, ManagedAssetRecord,
    OccurrenceRecord, ParishRecord, ReservedIds, SectionRecord, SongRecord, ThemeRecord,
};

/// A stable error for a stored value the store cannot parse.
fn corrupt(message: &'static str) -> StorageError {
    StorageError::Sqlite(message.to_owned())
}

/// Parses one stored identity without panicking on corrupt text.
fn entity<T: FromStr>(text: &str, message: &'static str) -> Result<T, StorageError> {
    text.parse().map_err(|_| corrupt(message))
}

/// Parses one stored timestamp without panicking on corrupt text.
fn time(text: &str, message: &'static str) -> Result<Timestamp, StorageError> {
    Timestamp::parse(text).map_err(|_| corrupt(message))
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

/// Parses a stored pagination state without panicking on corrupt text.
fn pagination(text: &str) -> Result<PaginationState, StorageError> {
    match text {
        "draft" => Ok(PaginationState::Draft),
        "approved" => Ok(PaginationState::Approved),
        "needs_review" => Ok(PaginationState::NeedsReview),
        _ => Err(corrupt("corrupt pagination_state")),
    }
}

/// Parses an optional stored archive time.
fn archived(text: Option<String>) -> Result<Option<Timestamp>, StorageError> {
    match text {
        Some(text) => Ok(Some(time(&text, "corrupt archived_at")?)),
        None => Ok(None),
    }
}

pub(crate) fn read_choir(
    tx: &Transaction<'_>,
    id: ChoirId,
) -> Result<Option<ChoirRecord>, StorageError> {
    let row = tx
        .query_row(
            "SELECT name, short_label, notes, created_at, archived_at FROM choir WHERE id = ?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(map_error)?;
    let Some((name, short_label, notes, created_at, archived_at)) = row else {
        return Ok(None);
    };
    let languages = texts(
        tx,
        "SELECT language FROM choir_language WHERE choir_id = ?1 ORDER BY position",
        &id.to_string(),
    )?;
    Ok(Some(ChoirRecord {
        id,
        name,
        short_label,
        languages,
        notes,
        created_at: time(&created_at, "corrupt created_at")?,
        archived_at: archived(archived_at)?,
    }))
}

/// Every Choir in one transaction, ordered by creation time and identity.
fn choirs_in(tx: &Transaction<'_>) -> Result<Vec<ChoirRecord>, StorageError> {
    let ids = {
        let mut statement = tx
            .prepare("SELECT id FROM choir ORDER BY created_at, id")
            .map_err(map_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(map_error)?;
        let mut ids = Vec::new();
        for row in rows {
            let text = row.map_err(map_error)?;
            ids.push(entity::<ChoirId>(&text, "corrupt id")?);
        }
        ids
    };
    let mut choirs = Vec::with_capacity(ids.len());
    for id in ids {
        let choir = read_choir(tx, id)?.ok_or(StorageError::NotFound {
            kind: RecordKind::Choir,
            id: id.entity(),
        })?;
        choirs.push(choir);
    }
    Ok(choirs)
}

fn read_choirs(conn: &mut Connection) -> Result<Vec<ChoirRecord>, StorageError> {
    let tx = conn.transaction().map_err(map_error)?;
    choirs_in(&tx)
}

pub(crate) fn read_song(
    tx: &Transaction<'_>,
    id: SongId,
) -> Result<Option<SongRecord>, StorageError> {
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
    songs_in(&tx)
}

/// Every Song in one transaction, ordered by creation time and identity.
fn songs_in(tx: &Transaction<'_>) -> Result<Vec<SongRecord>, StorageError> {
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
        let song = read_song(tx, id)?.ok_or(StorageError::NotFound {
            kind: RecordKind::Song,
            id: id.entity(),
        })?;
        songs.push(song);
    }
    Ok(songs)
}

/// Every Arrangement in one transaction, ordered by creation time and identity.
fn arrangements_in(tx: &Transaction<'_>) -> Result<Vec<ArrangementRecord>, StorageError> {
    let ids = {
        let mut statement = tx
            .prepare("SELECT id FROM arrangement ORDER BY created_at, id")
            .map_err(map_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
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
        let arrangement = read_arrangement(tx, id)?.ok_or(StorageError::NotFound {
            kind: RecordKind::Arrangement,
            id: id.entity(),
        })?;
        arrangements.push(arrangement);
    }
    Ok(arrangements)
}

fn read_library(conn: &mut Connection) -> Result<LibraryRecords, StorageError> {
    let tx = conn.transaction().map_err(map_error)?;
    let songs = songs_in(&tx)?;
    let arrangements = arrangements_in(&tx)?;
    let choirs = choirs_in(&tx)?;
    let mut reserved = Vec::with_capacity(arrangements.len());
    for arrangement in &arrangements {
        // Every identity the Arrangement ever used: its stored revisions plus
        // its current rows (a T2.1-era row may have no revision yet).
        let (mut sections, mut occurrences) = revisions::revision_ids(&tx, arrangement.id)?;
        sections.extend(arrangement.sections.iter().map(|section| section.id));
        occurrences.extend(arrangement.sequence.iter().map(|occurrence| occurrence.id));
        reserved.push(ReservedIds {
            arrangement: arrangement.id,
            sections: sections.into_iter().collect(),
            occurrences: occurrences.into_iter().collect(),
        });
    }
    Ok(LibraryRecords {
        songs,
        arrangements,
        choirs,
        reserved,
    })
}

pub(crate) fn read_arrangement(
    tx: &Transaction<'_>,
    id: ArrangementId,
) -> Result<Option<ArrangementRecord>, StorageError> {
    let row = tx
        .query_row(
            "SELECT song_id, title, rendition_attribution, created_at, archived_at, \
             pagination_state FROM arrangement WHERE id = ?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(map_error)?;
    let Some((song_id, title, rendition_attribution, created_at, archived_at, pagination_text)) =
        row
    else {
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
        pagination: pagination(&pagination_text)?,
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

/// One `managed_asset` row, parsed.
fn asset_row(
    id: String,
    sha256: &str,
    byte_len: i64,
    media_type: String,
    created_at: &str,
) -> Result<ManagedAssetRecord, StorageError> {
    Ok(ManagedAssetRecord {
        id: entity(&id, "corrupt id")?,
        sha256: sha256
            .parse::<ContentHash>()
            .map_err(|_| corrupt("corrupt sha256"))?,
        byte_len: u64::try_from(byte_len).map_err(|_| corrupt("corrupt byte_len"))?,
        media_type,
        created_at: time(created_at, "corrupt created_at")?,
    })
}

/// Reads one asset row, or `None` when it does not exist.
fn read_asset_row(
    tx: &Transaction<'_>,
    id: ManagedAssetId,
) -> Result<Option<ManagedAssetRecord>, StorageError> {
    let row = tx
        .query_row(
            "SELECT id, sha256, byte_len, media_type, created_at FROM managed_asset \
             WHERE id = ?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()
        .map_err(map_error)?;
    row.map(|(id, sha256, byte_len, media_type, created_at)| {
        asset_row(id, &sha256, byte_len, media_type, &created_at)
    })
    .transpose()
}

fn theme_by_id(conn: &mut Connection, id: ThemeId) -> Result<ThemeRecord, StorageError> {
    let tx = conn.transaction().map_err(map_error)?;
    let row = tx
        .query_row(
            "SELECT name, background_asset_id, created_at, archived_at FROM theme WHERE id = ?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()
        .map_err(map_error)?;
    let Some((name, background_id, created_at, archived_at)) = row else {
        return Err(StorageError::NotFound {
            kind: RecordKind::Theme,
            id: id.entity(),
        });
    };
    let background = match background_id {
        Some(text) => {
            let asset_id = entity::<ManagedAssetId>(&text, "corrupt background_asset_id")?;
            Some(
                read_asset_row(&tx, asset_id)?
                    .ok_or_else(|| corrupt("theme references a missing asset row"))?,
            )
        }
        None => None,
    };
    Ok(ThemeRecord {
        id,
        name,
        background,
        created_at: time(&created_at, "corrupt created_at")?,
        archived_at: match archived_at {
            Some(text) => Some(time(&text, "corrupt archived_at")?),
            None => None,
        },
    })
}

/// True when the bytes match the row's recorded length and hash.
fn intact(record: &ManagedAssetRecord, bytes: &[u8]) -> bool {
    u64::try_from(bytes.len()) == Ok(record.byte_len) && assets::hash(bytes) == record.sha256
}

fn read_asset_bytes(
    conn: &mut Connection,
    layout: &assets::AssetLayout,
    id: ManagedAssetId,
) -> Result<Vec<u8>, StorageError> {
    let record = {
        let tx = conn.transaction().map_err(map_error)?;
        read_asset_row(&tx, id)?.ok_or(StorageError::NotFound {
            kind: RecordKind::ManagedAsset,
            id: id.entity(),
        })?
    };
    match assets::read_file(layout, id.entity())? {
        Some(bytes) if intact(&record, &bytes) => Ok(bytes),
        _ => Err(StorageError::AssetIntegrity { id }),
    }
}

fn verify_all_assets(
    conn: &mut Connection,
    layout: &assets::AssetLayout,
) -> Result<AssetReport, StorageError> {
    let records = {
        let tx = conn.transaction().map_err(map_error)?;
        let mut statement = tx
            .prepare(
                "SELECT id, sha256, byte_len, media_type, created_at FROM managed_asset \
                 ORDER BY id",
            )
            .map_err(map_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(map_error)?;
        let mut records = Vec::new();
        for row in rows {
            let (id, sha256, byte_len, media_type, created_at) = row.map_err(map_error)?;
            records.push(asset_row(id, &sha256, byte_len, media_type, &created_at)?);
        }
        records
    };
    let mut report = AssetReport {
        checked: records.len(),
        missing: Vec::new(),
        corrupt: Vec::new(),
    };
    for record in &records {
        match assets::read_file(layout, record.id.entity())? {
            None => report.missing.push(record.id),
            Some(bytes) if !intact(record, &bytes) => report.corrupt.push(record.id),
            Some(_) => {}
        }
    }
    Ok(report)
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

    /// Reads every Song, Arrangement and Choir in one transaction.
    pub(crate) fn library(&mut self) -> Result<LibraryRecords, StorageError> {
        let conn = self.connection_mut()?;
        read_library(conn)
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

    /// Reads one Theme or reports `NotFound`.
    pub(crate) fn theme(&mut self, id: ThemeId) -> Result<ThemeRecord, StorageError> {
        let conn = self.connection_mut()?;
        theme_by_id(conn, id)
    }

    /// Reads and verifies one asset file; read-only, never repairs.
    pub(crate) fn read_asset(&mut self, id: ManagedAssetId) -> Result<Vec<u8>, StorageError> {
        let (conn, layout) = self.asset_read_parts()?;
        read_asset_bytes(conn, layout, id)
    }

    /// Re-hashes every asset file; read-only, never repairs. A read error
    /// other than a missing file fails the whole report.
    pub(crate) fn verify_assets(&mut self) -> Result<AssetReport, StorageError> {
        let (conn, layout) = self.asset_read_parts()?;
        verify_all_assets(conn, layout)
    }
}
