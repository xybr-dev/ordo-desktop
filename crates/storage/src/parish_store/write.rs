//! Validation and atomic writes for parish records.
//!
//! Every create validates its whole input before it opens a transaction, then
//! runs one immediate transaction and commits only after every row has been
//! written. Any error returns before the commit, so the dropped transaction
//! rolls the whole create back. Identities come from the worker's id source and
//! every row in one create shares the operation's single creation time.

use domain::identity::{
    ArrangementId, ChoirId, EntityId, ManagedAssetId, OccurrenceId, ParishId, SectionId, SongId,
    ThemeId, Timestamp,
};
use domain::revisions::RevisionEntity;
use rusqlite::{Connection, OptionalExtension, Params, Transaction, TransactionBehavior, params};

use crate::assets::{self, AssetLayout};
use crate::revisions::{
    self, ArrangementContent, OccurrenceContent, RevisionContent, SectionContent, SongContent,
};
use crate::worker::{AssetWriteParts, Core, Sources};
use crate::{RecordKind, StorageError, map_error};

use super::records::{
    ArrangementRecord, ChoirRecord, ManagedAssetRecord, NewArrangement, NewChoir, NewParish,
    NewSong, NewTheme, OccurrenceRecord, ParishRecord, SectionRecord, SongRecord, ThemeRecord,
};

/// How many identities one entity insert may try before giving up.
const MAX_ID_ATTEMPTS: usize = 8;

/// A failed insert that may be retried with a different identity.
fn is_identity_collision(error: &rusqlite::Error) -> bool {
    matches!(error.sqlite_extended_error_code(), Some(1555))
}

/// Bookkeeping for one write transaction.
///
/// `served` counts write statements so the fault hook can fail exactly its
/// requested step; `collision` records whether the last statement failed with
/// a primary-key constraint, the only failure a fresh identity may resolve.
/// The scripted failure is taken from the worker when the transaction opens, so
/// only a create that reached its write transaction consumes it.
pub(crate) struct WriteSteps {
    fault: Option<usize>,
    served: usize,
    collision: bool,
}

impl WriteSteps {
    /// Starts counting at zero for one transaction.
    pub(crate) fn new(fault: &mut Option<usize>) -> Self {
        Self {
            fault: fault.take(),
            served: 0,
            collision: false,
        }
    }

    /// Reports and clears whether the last statement was an identity
    /// collision.
    fn take_collision(&mut self) -> bool {
        std::mem::take(&mut self.collision)
    }
}

/// Runs one write statement, after the optional scripted failure at this step.
pub(crate) fn write(
    tx: &Transaction<'_>,
    step: &mut WriteSteps,
    sql: &'static str,
    params: impl Params,
) -> Result<usize, StorageError> {
    step.served += 1;
    step.collision = false;
    if step.fault == Some(step.served) {
        step.fault = None;
        return Err(StorageError::Sqlite("injected write failure".to_owned()));
    }
    tx.execute(sql, params).map_err(|error| {
        step.collision = is_identity_collision(&error);
        map_error(error)
    })
}

/// Inserts one entity row, regenerating its identity after a collision.
///
/// A failed statement does not end a SQLite transaction, so a retry stays
/// inside the same transaction. Child rows with identities use this too.
pub(crate) fn insert_entity(
    ids: &mut Sources,
    steps: &mut WriteSteps,
    kind: RecordKind,
    mut insert: impl FnMut(EntityId, &mut WriteSteps) -> Result<usize, StorageError>,
) -> Result<EntityId, StorageError> {
    for _ in 0..MAX_ID_ATTEMPTS {
        let id = EntityId::from_random_bytes(ids.next_bytes());
        match insert(id, steps) {
            Ok(_) => return Ok(id),
            Err(error) => {
                if steps.take_collision() {
                    continue;
                }
                return Err(error);
            }
        }
    }
    Err(StorageError::IdentityExhausted { kind })
}

/// The one canonical text form of the operation's creation time.
pub(crate) fn timestamp_text(at: Timestamp) -> Result<String, StorageError> {
    at.canonical_text().ok_or(StorageError::InvalidInput {
        field: "clock",
        reason: "out of range",
    })
}

/// Rejects text that is blank after trimming.
pub(crate) fn require_present(field: &'static str, value: &str) -> Result<(), StorageError> {
    if value.trim().is_empty() {
        Err(StorageError::InvalidInput {
            field,
            reason: "blank",
        })
    } else {
        Ok(())
    }
}

/// Rejects a repeated value in a set-shaped input.
pub(crate) fn require_distinct<T: PartialEq>(
    field: &'static str,
    values: &[T],
) -> Result<(), StorageError> {
    for (index, value) in values.iter().enumerate() {
        if values[..index].contains(value) {
            return Err(StorageError::InvalidInput {
                field,
                reason: "duplicate",
            });
        }
    }
    Ok(())
}

/// Validates the Song before any statement runs.
fn validate_song(song: &NewSong) -> Result<(), StorageError> {
    validate_song_fields(
        &song.title,
        &song.aliases,
        &song.liturgical_moments,
        &song.occasions,
    )
}

/// The Song field rules shared by create and by revision commits.
pub(crate) fn validate_song_fields(
    title: &str,
    aliases: &[String],
    liturgical_moments: &[String],
    occasions: &[String],
) -> Result<(), StorageError> {
    require_present("title", title)?;
    for alias in aliases {
        require_present("alias", alias)?;
    }
    for moment in liturgical_moments {
        require_present("liturgical_moment", moment)?;
    }
    require_distinct("liturgical_moment", liturgical_moments)?;
    for occasion in occasions {
        require_present("occasion", occasion)?;
    }
    require_distinct("occasion", occasions)?;
    Ok(())
}

/// Validates the Arrangement before any statement runs.
fn validate_arrangement(arrangement: &NewArrangement) -> Result<(), StorageError> {
    validate_arrangement_fields(
        &arrangement.title,
        &arrangement.languages,
        &arrangement.choirs,
        &arrangement.occasions,
        arrangement
            .sections
            .iter()
            .map(|section| section.label.as_deref()),
    )?;
    if arrangement
        .sequence
        .iter()
        .any(|index| *index >= arrangement.sections.len())
    {
        return Err(StorageError::InvalidInput {
            field: "sequence",
            reason: "unknown section",
        });
    }
    Ok(())
}

/// The Arrangement field rules shared by create and by revision commits.
pub(crate) fn validate_arrangement_fields<'a>(
    title: &str,
    languages: &[String],
    choirs: &[ChoirId],
    occasions: &[String],
    labels: impl Iterator<Item = Option<&'a str>>,
) -> Result<(), StorageError> {
    require_present("title", title)?;
    for language in languages {
        require_present("language", language)?;
    }
    require_distinct("language", languages)?;
    require_distinct("choir", choirs)?;
    for occasion in occasions {
        require_present("occasion", occasion)?;
    }
    require_distinct("occasion", occasions)?;
    for label in labels.flatten() {
        require_present("label", label)?;
    }
    Ok(())
}

fn insert_parish(
    conn: &mut Connection,
    ids: &mut Sources,
    at: Timestamp,
    fault: &mut Option<usize>,
    parish: NewParish,
) -> Result<ParishRecord, StorageError> {
    require_present("name", &parish.name)?;
    let created = timestamp_text(at)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_error)?;
    let mut steps = WriteSteps::new(fault);

    let exists = tx
        .query_row("SELECT 1 FROM parish LIMIT 1", [], |_row| Ok(()))
        .optional()
        .map_err(map_error)?
        .is_some();
    if exists {
        return Err(StorageError::ParishExists);
    }

    let id = insert_entity(ids, &mut steps, RecordKind::Parish, |id, steps| {
        write(
            &tx,
            steps,
            "INSERT INTO parish (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![id.to_string(), parish.name, created],
        )
    })?;
    tx.commit().map_err(map_error)?;
    Ok(ParishRecord {
        id: ParishId::from_entity(id),
        name: parish.name,
        created_at: at,
    })
}

fn insert_choir(
    conn: &mut Connection,
    ids: &mut Sources,
    at: Timestamp,
    fault: &mut Option<usize>,
    choir: NewChoir,
) -> Result<ChoirRecord, StorageError> {
    require_present("name", &choir.name)?;
    let created = timestamp_text(at)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_error)?;
    let mut steps = WriteSteps::new(fault);

    let id = insert_entity(ids, &mut steps, RecordKind::Choir, |id, steps| {
        write(
            &tx,
            steps,
            "INSERT INTO choir (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![id.to_string(), choir.name, created],
        )
    })?;
    tx.commit().map_err(map_error)?;
    Ok(ChoirRecord {
        id: ChoirId::from_entity(id),
        name: choir.name,
        created_at: at,
        archived_at: None,
    })
}

fn insert_song(
    conn: &mut Connection,
    ids: &mut Sources,
    at: Timestamp,
    fault: &mut Option<usize>,
    song: NewSong,
) -> Result<SongRecord, StorageError> {
    validate_song(&song)?;
    let created = timestamp_text(at)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_error)?;
    let mut steps = WriteSteps::new(fault);

    let id = insert_entity(ids, &mut steps, RecordKind::Song, |id, steps| {
        write(
            &tx,
            steps,
            "INSERT INTO song (id, title, original_attribution, provenance, rights_notes, \
             created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id.to_string(),
                song.title,
                song.original_attribution.as_deref(),
                song.provenance.as_deref(),
                song.rights_notes.as_deref(),
                created
            ],
        )
    })?;
    let song_text = id.to_string();

    for (position, alias) in song.aliases.iter().enumerate() {
        write(
            &tx,
            &mut steps,
            "INSERT INTO song_alias (song_id, position, alias) VALUES (?1, ?2, ?3)",
            params![song_text, position as i64, alias],
        )?;
    }
    for moment in &song.liturgical_moments {
        write(
            &tx,
            &mut steps,
            "INSERT INTO song_liturgical_moment (song_id, moment) VALUES (?1, ?2)",
            params![song_text, moment],
        )?;
    }
    for occasion in &song.occasions {
        write(
            &tx,
            &mut steps,
            "INSERT INTO song_occasion (song_id, occasion) VALUES (?1, ?2)",
            params![song_text, occasion],
        )?;
    }
    revisions::append_creation(
        &tx,
        ids,
        &mut steps,
        at,
        RevisionEntity::Song(SongId::from_entity(id)),
        &RevisionContent::Song(SongContent {
            title: song.title.clone(),
            aliases: song.aliases.clone(),
            original_attribution: song.original_attribution.clone(),
            provenance: song.provenance.clone(),
            rights_notes: song.rights_notes.clone(),
            liturgical_moments: song.liturgical_moments.clone(),
            occasions: song.occasions.clone(),
        }),
    )?;
    tx.commit().map_err(map_error)?;

    let mut liturgical_moments = song.liturgical_moments;
    liturgical_moments.sort_unstable();
    let mut occasions = song.occasions;
    occasions.sort_unstable();
    Ok(SongRecord {
        id: SongId::from_entity(id),
        title: song.title,
        aliases: song.aliases,
        original_attribution: song.original_attribution,
        provenance: song.provenance,
        rights_notes: song.rights_notes,
        liturgical_moments,
        occasions,
        created_at: at,
        archived_at: None,
    })
}

fn insert_arrangement(
    conn: &mut Connection,
    ids: &mut Sources,
    at: Timestamp,
    fault: &mut Option<usize>,
    arrangement: NewArrangement,
) -> Result<ArrangementRecord, StorageError> {
    validate_arrangement(&arrangement)?;
    let created = timestamp_text(at)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_error)?;
    let mut steps = WriteSteps::new(fault);

    let song_found = tx
        .query_row(
            "SELECT 1 FROM song WHERE id = ?1",
            params![arrangement.song.to_string()],
            |_row| Ok(()),
        )
        .optional()
        .map_err(map_error)?
        .is_some();
    if !song_found {
        return Err(StorageError::MissingReference {
            kind: RecordKind::Song,
            id: arrangement.song.entity(),
        });
    }
    for choir in &arrangement.choirs {
        let choir_found = tx
            .query_row(
                "SELECT 1 FROM choir WHERE id = ?1",
                params![choir.to_string()],
                |_row| Ok(()),
            )
            .optional()
            .map_err(map_error)?
            .is_some();
        if !choir_found {
            return Err(StorageError::MissingReference {
                kind: RecordKind::Choir,
                id: choir.entity(),
            });
        }
    }

    let id = insert_entity(ids, &mut steps, RecordKind::Arrangement, |id, steps| {
        write(
            &tx,
            steps,
            "INSERT INTO arrangement (id, song_id, title, rendition_attribution, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id.to_string(),
                arrangement.song.to_string(),
                arrangement.title,
                arrangement.rendition_attribution.as_deref(),
                created
            ],
        )
    })?;
    let arrangement_text = id.to_string();

    for (position, language) in arrangement.languages.iter().enumerate() {
        write(
            &tx,
            &mut steps,
            "INSERT INTO arrangement_language (arrangement_id, position, language) \
             VALUES (?1, ?2, ?3)",
            params![arrangement_text, position as i64, language],
        )?;
    }
    for choir in &arrangement.choirs {
        write(
            &tx,
            &mut steps,
            "INSERT INTO arrangement_choir (arrangement_id, choir_id) VALUES (?1, ?2)",
            params![arrangement_text, choir.to_string()],
        )?;
    }
    for occasion in &arrangement.occasions {
        write(
            &tx,
            &mut steps,
            "INSERT INTO arrangement_occasion (arrangement_id, occasion) VALUES (?1, ?2)",
            params![arrangement_text, occasion],
        )?;
    }

    let mut section_ids = Vec::with_capacity(arrangement.sections.len());
    for (position, section) in arrangement.sections.iter().enumerate() {
        let section_id = insert_entity(ids, &mut steps, RecordKind::Section, |id, steps| {
            write(
                &tx,
                steps,
                "INSERT INTO arrangement_section (id, arrangement_id, position, label, lyrics, \
                 created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id.to_string(),
                    arrangement_text,
                    position as i64,
                    section.label.as_deref(),
                    section.lyrics,
                    created
                ],
            )
        })?;
        section_ids.push(SectionId::from_entity(section_id));
    }

    let mut occurrence_ids = Vec::with_capacity(arrangement.sequence.len());
    for (position, section_index) in arrangement.sequence.iter().enumerate() {
        let section_id = section_ids[*section_index];
        let occurrence_id = insert_entity(ids, &mut steps, RecordKind::Occurrence, |id, steps| {
            write(
                &tx,
                steps,
                "INSERT INTO section_occurrence (id, arrangement_id, section_id, position, \
                 created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id.to_string(),
                    arrangement_text,
                    section_id.to_string(),
                    position as i64,
                    created
                ],
            )
        })?;
        occurrence_ids.push(occurrence_id);
    }
    revisions::append_creation(
        &tx,
        ids,
        &mut steps,
        at,
        RevisionEntity::Arrangement(ArrangementId::from_entity(id)),
        &RevisionContent::Arrangement(ArrangementContent {
            title: arrangement.title.clone(),
            languages: arrangement.languages.clone(),
            choirs: arrangement.choirs.clone(),
            rendition_attribution: arrangement.rendition_attribution.clone(),
            occasions: arrangement.occasions.clone(),
            sections: arrangement
                .sections
                .iter()
                .zip(&section_ids)
                .map(|(section, id)| SectionContent {
                    id: *id,
                    label: section.label.clone(),
                    lyrics: section.lyrics.clone(),
                })
                .collect(),
            sequence: arrangement
                .sequence
                .iter()
                .zip(&occurrence_ids)
                .map(|(&section_index, id)| OccurrenceContent {
                    id: OccurrenceId::from_entity(*id),
                    section: section_ids[section_index],
                })
                .collect(),
        }),
    )?;
    tx.commit().map_err(map_error)?;

    let sequence: Vec<OccurrenceRecord> = arrangement
        .sequence
        .iter()
        .zip(occurrence_ids)
        .map(|(&section_index, id)| OccurrenceRecord {
            id: OccurrenceId::from_entity(id),
            section: section_ids[section_index],
            created_at: at,
        })
        .collect();
    let sections: Vec<SectionRecord> = arrangement
        .sections
        .into_iter()
        .zip(section_ids)
        .map(|(section, id)| SectionRecord {
            id,
            label: section.label,
            lyrics: section.lyrics,
            created_at: at,
        })
        .collect();
    let mut choirs = arrangement.choirs;
    choirs.sort_unstable();
    let mut occasions = arrangement.occasions;
    occasions.sort_unstable();

    Ok(ArrangementRecord {
        id: ArrangementId::from_entity(id),
        song: arrangement.song,
        title: arrangement.title,
        languages: arrangement.languages,
        choirs,
        rendition_attribution: arrangement.rendition_attribution,
        occasions,
        sections,
        sequence,
        created_at: at,
        archived_at: None,
    })
}

/// Validates the Theme before any statement runs or any byte is hashed.
fn validate_theme(theme: &NewTheme) -> Result<(), StorageError> {
    require_present("name", &theme.name)?;
    if let Some(upload) = &theme.background {
        if !matches!(upload.media_type.as_str(), "image/png" | "image/jpeg") {
            return Err(StorageError::InvalidInput {
                field: "media_type",
                reason: "unsupported",
            });
        }
        if upload.bytes.is_empty() {
            return Err(StorageError::InvalidInput {
                field: "background",
                reason: "empty",
            });
        }
        if i64::try_from(upload.bytes.len()).is_err() {
            return Err(StorageError::InvalidInput {
                field: "background",
                reason: "too large",
            });
        }
    }
    Ok(())
}

/// Draws an asset identity whose row and file are both absent.
fn draw_asset_id(
    tx: &Transaction<'_>,
    ids: &mut Sources,
    layout: &AssetLayout,
) -> Result<EntityId, StorageError> {
    for _ in 0..MAX_ID_ATTEMPTS {
        let id = EntityId::from_random_bytes(ids.next_bytes());
        let row_exists = tx
            .query_row(
                "SELECT 1 FROM managed_asset WHERE id = ?1",
                params![id.to_string()],
                |_row| Ok(()),
            )
            .optional()
            .map_err(map_error)?
            .is_some();
        if !row_exists && !assets::file_exists(layout, id)? {
            return Ok(id);
        }
    }
    Err(StorageError::IdentityExhausted {
        kind: RecordKind::ManagedAsset,
    })
}

/// Inserts the asset row, then the Theme row that references it.
fn insert_asset_and_theme(
    tx: &Transaction<'_>,
    ids: &mut Sources,
    steps: &mut WriteSteps,
    asset: &ManagedAssetRecord,
    name: &str,
    created: &str,
) -> Result<EntityId, StorageError> {
    let byte_len = i64::try_from(asset.byte_len).map_err(|_| StorageError::InvalidInput {
        field: "background",
        reason: "too large",
    })?;
    let asset_text = asset.id.to_string();
    write(
        tx,
        steps,
        "INSERT INTO managed_asset (id, sha256, byte_len, media_type, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            asset_text,
            asset.sha256.to_string(),
            byte_len,
            asset.media_type,
            created
        ],
    )?;
    insert_theme_row(tx, ids, steps, name, Some(&asset_text), created)
}

/// Inserts the Theme row, regenerating its identity after a collision.
fn insert_theme_row(
    tx: &Transaction<'_>,
    ids: &mut Sources,
    steps: &mut WriteSteps,
    name: &str,
    background: Option<&str>,
    created: &str,
) -> Result<EntityId, StorageError> {
    insert_entity(ids, steps, RecordKind::Theme, |id, steps| {
        write(
            tx,
            steps,
            "INSERT INTO theme (id, name, background_asset_id, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![id.to_string(), name, background, created],
        )
    })
}

/// Creates a Theme; a background image is written and committed with it.
///
/// Order: validate, hash, open the immediate transaction, draw an asset id
/// (row and file absent), stage and sync the file, rename it into place,
/// insert the asset row, insert the Theme row, commit. Any failure from the
/// file write onward removes the staging and final files, and the dropped
/// transaction rolls the rows back.
fn insert_theme(
    parts: AssetWriteParts<'_>,
    at: Timestamp,
    theme: NewTheme,
) -> Result<ThemeRecord, StorageError> {
    let AssetWriteParts {
        conn,
        ids,
        fault,
        layout,
        asset_fault,
    } = parts;
    validate_theme(&theme)?;
    let created = timestamp_text(at)?;
    let hashed = match &theme.background {
        Some(upload) => {
            let byte_len =
                u64::try_from(upload.bytes.len()).map_err(|_| StorageError::InvalidInput {
                    field: "background",
                    reason: "too large",
                })?;
            Some((assets::hash(&upload.bytes), byte_len))
        }
        None => None,
    };
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_error)?;
    let mut steps = WriteSteps::new(fault);

    let (Some(upload), Some((sha256, byte_len))) = (theme.background, hashed) else {
        let id = insert_theme_row(&tx, ids, &mut steps, &theme.name, None, &created)?;
        tx.commit().map_err(map_error)?;
        return Ok(ThemeRecord {
            id: ThemeId::from_entity(id),
            name: theme.name,
            background: None,
            created_at: at,
            archived_at: None,
        });
    };

    let asset_id = draw_asset_id(&tx, ids, layout)?;
    let asset = ManagedAssetRecord {
        id: ManagedAssetId::from_entity(asset_id),
        sha256,
        byte_len,
        media_type: upload.media_type,
        created_at: at,
    };
    let fail_midway = std::mem::take(asset_fault);
    if let Err(error) = assets::write_file(layout, asset_id, &upload.bytes, fail_midway) {
        assets::remove_files(layout, asset_id);
        return Err(error);
    }
    let theme_id = match insert_asset_and_theme(&tx, ids, &mut steps, &asset, &theme.name, &created)
    {
        Ok(id) => id,
        Err(error) => {
            assets::remove_files(layout, asset_id);
            return Err(error);
        }
    };
    if let Err(error) = tx.commit() {
        // A commit error can still mean the commit landed. Then the caller gets
        // an error although the Theme exists, and a retry would create a second
        // Theme. Keep the file when its row is there, or when that cannot be
        // checked: an unreferenced file is swept on the next open, a referenced
        // missing file is a loss.
        let landed = conn
            .query_row(
                "SELECT 1 FROM managed_asset WHERE id = ?1",
                params![asset_id.to_string()],
                |_row| Ok(()),
            )
            .optional();
        if matches!(landed, Ok(None)) {
            assets::remove_files(layout, asset_id);
        }
        return Err(map_error(error));
    }
    Ok(ThemeRecord {
        id: ThemeId::from_entity(theme_id),
        name: theme.name,
        background: Some(asset),
        created_at: at,
        archived_at: None,
    })
}

impl Core {
    /// Creates the Parish and its identity.
    pub(crate) fn create_parish(
        &mut self,
        parish: NewParish,
    ) -> Result<ParishRecord, StorageError> {
        let at = self.timestamp();
        let (conn, ids, fault) = self.write_parts()?;
        insert_parish(conn, ids, at, fault, parish)
    }

    /// Creates one Choir and its identity.
    pub(crate) fn create_choir(&mut self, choir: NewChoir) -> Result<ChoirRecord, StorageError> {
        let at = self.timestamp();
        let (conn, ids, fault) = self.write_parts()?;
        insert_choir(conn, ids, at, fault, choir)
    }

    /// Creates one Song with its aliases, moments, and occasions.
    pub(crate) fn create_song(&mut self, song: NewSong) -> Result<SongRecord, StorageError> {
        let at = self.timestamp();
        let (conn, ids, fault) = self.write_parts()?;
        insert_song(conn, ids, at, fault, song)
    }

    /// Creates one Arrangement with its sections, Sequence, and links.
    pub(crate) fn create_arrangement(
        &mut self,
        arrangement: NewArrangement,
    ) -> Result<ArrangementRecord, StorageError> {
        let at = self.timestamp();
        let (conn, ids, fault) = self.write_parts()?;
        insert_arrangement(conn, ids, at, fault, arrangement)
    }

    /// Creates one Theme; with a background, its asset file and rows commit together.
    /// If COMMIT reports failure but the rows landed, the caller gets an error
    /// although the Theme exists, and a retry would create a second Theme.
    pub(crate) fn create_theme(&mut self, theme: NewTheme) -> Result<ThemeRecord, StorageError> {
        let at = self.timestamp();
        let parts = self.asset_write_parts()?;
        insert_theme(parts, at, theme)
    }
}
