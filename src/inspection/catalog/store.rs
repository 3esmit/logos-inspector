use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use redb::{
    CommitError, Database, DatabaseError, Durability, ReadOnlyDatabase, ReadableDatabase,
    ReadableTable, StorageError, TableDefinition, TableError, TransactionError,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::model::{
    CATALOG_RECORD_VERSION, CATALOG_SCHEMA_VERSION, CatalogBatch, CatalogError,
    CatalogInvalidationReason, CatalogMetadata, CatalogResult, CatalogSchemaMetadata,
    CatalogSnapshot, CoverageGap, CoverageSegment, ZoneCatalogRecord, ZoneEvidenceReference,
    validate_evidence, validate_frontier, validate_gap, validate_hex_id, validate_local_id,
    validate_metadata, validate_segment, validate_traversal, validate_zone,
};

const METADATA_TABLE: TableDefinition<'static, &str, &[u8]> =
    TableDefinition::new("zone_catalog_metadata_v1");
const ZONES_TABLE: TableDefinition<'static, &str, &[u8]> =
    TableDefinition::new("zone_catalog_zones_v1");
const EVIDENCE_TABLE: TableDefinition<'static, &str, &[u8]> =
    TableDefinition::new("zone_catalog_evidence_v1");
const SEGMENTS_TABLE: TableDefinition<'static, &str, &[u8]> =
    TableDefinition::new("zone_catalog_segments_v1");
const GAPS_TABLE: TableDefinition<'static, &str, &[u8]> =
    TableDefinition::new("zone_catalog_gaps_v1");

const SCHEMA_KEY: &str = "schema";
const CATALOG_KEY: &str = "catalog";
const FRONTIER_KEY: &str = "frontier";
const TRAVERSAL_KEY: &str = "traversal";

#[derive(Serialize, serde::Deserialize)]
struct VersionedRecord<T> {
    record_version: u32,
    value: T,
}

enum CatalogDatabase {
    Writable(Database),
    ReadOnly(ReadOnlyDatabase),
}

pub(super) struct ZoneCatalogStore {
    database: CatalogDatabase,
}

impl ZoneCatalogStore {
    pub(super) fn create(path: &Path, metadata: CatalogMetadata) -> CatalogResult<Self> {
        validate_metadata(&metadata)?;
        if metadata.catalog_revision != 0 {
            return Err(CatalogError::invalid_input(
                "new catalog revision must be zero",
            ));
        }
        if metadata.updated_at_unix < metadata.created_at_unix {
            return Err(CatalogError::invalid_input(
                "catalog update time precedes creation time",
            ));
        }
        if path.try_exists().map_err(CatalogError::storage)? {
            return Err(CatalogError::invalid_input("catalog path already exists"));
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(CatalogError::storage)?;
        }

        let database = Database::create(path).map_err(map_database_error)?;
        let mut transaction = database.begin_write().map_err(map_transaction_error)?;
        transaction
            .set_durability(Durability::Immediate)
            .map_err(CatalogError::storage)?;

        {
            let mut table = transaction
                .open_table(METADATA_TABLE)
                .map_err(map_table_error)?;
            insert_raw(&mut table, SCHEMA_KEY, &CatalogSchemaMetadata::current())?;
            insert_versioned(&mut table, CATALOG_KEY, &metadata)?;
        }
        create_empty_table(&transaction, ZONES_TABLE)?;
        create_empty_table(&transaction, EVIDENCE_TABLE)?;
        create_empty_table(&transaction, SEGMENTS_TABLE)?;
        create_empty_table(&transaction, GAPS_TABLE)?;
        transaction.commit().map_err(map_commit_error)?;

        let store = Self {
            database: CatalogDatabase::Writable(database),
        };
        store.snapshot()?;
        Ok(store)
    }

    pub(super) fn open(path: &Path) -> CatalogResult<Self> {
        let store = Self {
            database: CatalogDatabase::Writable(Database::open(path).map_err(map_database_error)?),
        };
        store.snapshot()?;
        Ok(store)
    }

    pub(super) fn open_read_only(path: &Path) -> CatalogResult<Self> {
        let store = Self {
            database: CatalogDatabase::ReadOnly(
                ReadOnlyDatabase::open(path).map_err(map_database_error)?,
            ),
        };
        store.snapshot()?;
        Ok(store)
    }

    pub(super) const fn is_read_only(&self) -> bool {
        matches!(self.database, CatalogDatabase::ReadOnly(_))
    }

    pub(super) fn snapshot(&self) -> CatalogResult<CatalogSnapshot> {
        match &self.database {
            CatalogDatabase::Writable(database) => snapshot_from_database(database),
            CatalogDatabase::ReadOnly(database) => snapshot_from_database(database),
        }
    }

    pub(super) fn commit_batch(&self, batch: CatalogBatch) -> CatalogResult<CatalogSnapshot> {
        self.commit_batch_with_hook(batch, || Ok(()))
    }

    fn commit_batch_with_hook<F>(
        &self,
        batch: CatalogBatch,
        before_commit: F,
    ) -> CatalogResult<CatalogSnapshot>
    where
        F: FnOnce() -> CatalogResult<()>,
    {
        validate_batch(&batch)?;
        let CatalogDatabase::Writable(database) = &self.database else {
            return Err(CatalogError::invalid_input(
                "cannot commit through a read-only catalog",
            ));
        };

        let mut transaction = database.begin_write().map_err(map_transaction_error)?;
        transaction
            .set_durability(Durability::Immediate)
            .map_err(CatalogError::storage)?;

        let mut metadata = {
            let table = transaction
                .open_table(METADATA_TABLE)
                .map_err(map_table_error)?;
            validate_schema(&table)?;
            read_required_versioned(&table, CATALOG_KEY, "catalog metadata")?
        };
        validate_persisted(validate_metadata(&metadata), "catalog metadata")?;
        if metadata.catalog_revision != batch.expected_catalog_revision {
            return Err(CatalogError::RevisionConflict {
                expected: batch.expected_catalog_revision,
                current: metadata.catalog_revision,
            });
        }
        if batch.updated_at_unix < metadata.updated_at_unix {
            return Err(CatalogError::invalid_input(
                "catalog update time moves backwards",
            ));
        }

        apply_batch_records(&transaction, &batch)?;
        metadata.catalog_revision = metadata
            .catalog_revision
            .checked_add(1)
            .ok_or_else(|| CatalogError::invalid_input("catalog revision exhausted"))?;
        metadata.updated_at_unix = batch.updated_at_unix;
        {
            let mut table = transaction
                .open_table(METADATA_TABLE)
                .map_err(map_table_error)?;
            insert_versioned(&mut table, CATALOG_KEY, &metadata)?;
            replace_optional_versioned(&mut table, FRONTIER_KEY, batch.frontier.as_ref())?;
            replace_optional_versioned(&mut table, TRAVERSAL_KEY, batch.traversal.as_ref())?;
        }

        let snapshot = snapshot_from_write_transaction(&transaction).map_err(map_staged_error)?;
        before_commit()?;
        transaction.commit().map_err(map_commit_error)?;
        Ok(snapshot)
    }
}

fn create_empty_table(
    transaction: &redb::WriteTransaction,
    definition: TableDefinition<'static, &str, &[u8]>,
) -> CatalogResult<()> {
    drop(
        transaction
            .open_table(definition)
            .map_err(map_table_error)?,
    );
    Ok(())
}

fn snapshot_from_database(database: &impl ReadableDatabase) -> CatalogResult<CatalogSnapshot> {
    let transaction = database.begin_read().map_err(map_transaction_error)?;
    let metadata = transaction
        .open_table(METADATA_TABLE)
        .map_err(map_table_error)?;
    let zones = transaction
        .open_table(ZONES_TABLE)
        .map_err(map_table_error)?;
    let evidence = transaction
        .open_table(EVIDENCE_TABLE)
        .map_err(map_table_error)?;
    let segments = transaction
        .open_table(SEGMENTS_TABLE)
        .map_err(map_table_error)?;
    let gaps = transaction
        .open_table(GAPS_TABLE)
        .map_err(map_table_error)?;
    read_snapshot(&metadata, &zones, &evidence, &segments, &gaps)
}

fn snapshot_from_write_transaction(
    transaction: &redb::WriteTransaction,
) -> CatalogResult<CatalogSnapshot> {
    let metadata = transaction
        .open_table(METADATA_TABLE)
        .map_err(map_table_error)?;
    let zones = transaction
        .open_table(ZONES_TABLE)
        .map_err(map_table_error)?;
    let evidence = transaction
        .open_table(EVIDENCE_TABLE)
        .map_err(map_table_error)?;
    let segments = transaction
        .open_table(SEGMENTS_TABLE)
        .map_err(map_table_error)?;
    let gaps = transaction
        .open_table(GAPS_TABLE)
        .map_err(map_table_error)?;
    read_snapshot(&metadata, &zones, &evidence, &segments, &gaps)
}

fn read_snapshot<M, Z, E, S, G>(
    metadata_table: &M,
    zones_table: &Z,
    evidence_table: &E,
    segments_table: &S,
    gaps_table: &G,
) -> CatalogResult<CatalogSnapshot>
where
    M: ReadableTable<&'static str, &'static [u8]>,
    Z: ReadableTable<&'static str, &'static [u8]>,
    E: ReadableTable<&'static str, &'static [u8]>,
    S: ReadableTable<&'static str, &'static [u8]>,
    G: ReadableTable<&'static str, &'static [u8]>,
{
    validate_schema(metadata_table)?;
    let metadata = read_required_versioned(metadata_table, CATALOG_KEY, "catalog metadata")?;
    let frontier = read_optional_versioned(metadata_table, FRONTIER_KEY, "catalog frontier")?;
    let traversal = read_optional_versioned(metadata_table, TRAVERSAL_KEY, "catalog traversal")?;
    let zones = read_records(zones_table, "Zone", |record: &ZoneCatalogRecord| {
        record.channel_id.as_str()
    })?;
    let evidence = read_records(
        evidence_table,
        "evidence reference",
        |reference: &ZoneEvidenceReference| reference.evidence_id.as_str(),
    )?;
    let segments = read_records(
        segments_table,
        "coverage segment",
        |segment: &CoverageSegment| segment.segment_id.as_str(),
    )?;
    let gaps = read_records(gaps_table, "coverage gap", |gap: &CoverageGap| {
        gap.gap_id.as_str()
    })?;

    let snapshot = CatalogSnapshot {
        metadata,
        frontier,
        traversal,
        zones,
        evidence,
        segments,
        gaps,
    };
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn validate_schema<T>(table: &T) -> CatalogResult<()>
where
    T: ReadableTable<&'static str, &'static [u8]>,
{
    let value = table
        .get(SCHEMA_KEY)
        .map_err(map_storage_error)?
        .ok_or_else(|| {
            CatalogError::invalidated(
                CatalogInvalidationReason::SchemaMissing,
                "catalog schema metadata is missing",
            )
        })?;
    let schema: CatalogSchemaMetadata = serde_json::from_slice(value.value()).map_err(|error| {
        CatalogError::invalidated(
            CatalogInvalidationReason::RecordDecode,
            format!("catalog schema metadata cannot be decoded: {error}"),
        )
    })?;
    if schema.schema_version != CATALOG_SCHEMA_VERSION {
        return Err(CatalogError::invalidated(
            CatalogInvalidationReason::SchemaVersion,
            format!(
                "catalog schema version {} is incompatible with {}",
                schema.schema_version, CATALOG_SCHEMA_VERSION
            ),
        ));
    }
    if schema.record_version != CATALOG_RECORD_VERSION {
        return Err(CatalogError::invalidated(
            CatalogInvalidationReason::RecordVersion,
            format!(
                "catalog record version {} is incompatible with {}",
                schema.record_version, CATALOG_RECORD_VERSION
            ),
        ));
    }
    Ok(())
}

fn read_required_versioned<T, R>(table: &T, key: &str, label: &str) -> CatalogResult<R>
where
    T: ReadableTable<&'static str, &'static [u8]>,
    R: DeserializeOwned,
{
    let value = table.get(key).map_err(map_storage_error)?.ok_or_else(|| {
        CatalogError::invalidated(
            CatalogInvalidationReason::RecordInvariant,
            format!("{label} is missing"),
        )
    })?;
    decode_versioned(value.value(), label)
}

fn read_optional_versioned<T, R>(table: &T, key: &str, label: &str) -> CatalogResult<Option<R>>
where
    T: ReadableTable<&'static str, &'static [u8]>,
    R: DeserializeOwned,
{
    table
        .get(key)
        .map_err(map_storage_error)?
        .map(|value| decode_versioned(value.value(), label))
        .transpose()
}

fn read_records<T, R, F>(table: &T, label: &str, key_of: F) -> CatalogResult<Vec<R>>
where
    T: ReadableTable<&'static str, &'static [u8]>,
    R: DeserializeOwned,
    F: Fn(&R) -> &str,
{
    let mut records = Vec::new();
    let iterator = table.iter().map_err(map_storage_error)?;
    for item in iterator {
        let (key, value) = item.map_err(map_storage_error)?;
        let record: R = decode_versioned(value.value(), label)?;
        if key.value() != key_of(&record) {
            return Err(CatalogError::invalidated(
                CatalogInvalidationReason::RecordInvariant,
                format!("{label} table key does not match its record id"),
            ));
        }
        records.push(record);
    }
    Ok(records)
}

fn decode_versioned<T: DeserializeOwned>(bytes: &[u8], label: &str) -> CatalogResult<T> {
    let record: VersionedRecord<T> = serde_json::from_slice(bytes).map_err(|error| {
        CatalogError::invalidated(
            CatalogInvalidationReason::RecordDecode,
            format!("{label} cannot be decoded: {error}"),
        )
    })?;
    if record.record_version != CATALOG_RECORD_VERSION {
        return Err(CatalogError::invalidated(
            CatalogInvalidationReason::RecordVersion,
            format!(
                "{label} record version {} is incompatible with {}",
                record.record_version, CATALOG_RECORD_VERSION
            ),
        ));
    }
    Ok(record.value)
}

fn validate_snapshot(snapshot: &CatalogSnapshot) -> CatalogResult<()> {
    validate_persisted(validate_metadata(&snapshot.metadata), "catalog metadata")?;
    if snapshot.metadata.updated_at_unix < snapshot.metadata.created_at_unix {
        return persisted_invariant("catalog update time precedes creation time");
    }
    if let Some(frontier) = snapshot.frontier.as_ref() {
        validate_persisted(validate_frontier(frontier), "catalog frontier")?;
    }
    if let Some(traversal) = snapshot.traversal.as_ref() {
        validate_persisted(validate_traversal(traversal), "catalog traversal")?;
    }

    let segments_by_id: HashMap<&str, &CoverageSegment> = snapshot
        .segments
        .iter()
        .map(|segment| {
            validate_persisted(validate_segment(segment), "coverage segment")?;
            Ok((segment.segment_id.as_str(), segment))
        })
        .collect::<CatalogResult<_>>()?;
    let evidence_by_id: HashMap<&str, &ZoneEvidenceReference> = snapshot
        .evidence
        .iter()
        .map(|reference| {
            validate_persisted(validate_evidence(reference), "evidence reference")?;
            let Some(segment) = segments_by_id.get(reference.coverage_segment_id.as_str()) else {
                return persisted_invariant(format!(
                    "evidence {} references missing coverage segment {}",
                    reference.evidence_id, reference.coverage_segment_id
                ));
            };
            if reference.l1_slot < segment.floor.slot || reference.l1_slot > segment.frontier.slot {
                return persisted_invariant(format!(
                    "evidence {} falls outside coverage segment {}",
                    reference.evidence_id, reference.coverage_segment_id
                ));
            }
            Ok((reference.evidence_id.as_str(), reference))
        })
        .collect::<CatalogResult<_>>()?;
    let mut evidence_counts = HashMap::<&str, u64>::new();
    for reference in &snapshot.evidence {
        let count = evidence_counts
            .entry(reference.channel_id.as_str())
            .or_default();
        *count = count.checked_add(1).ok_or_else(|| {
            CatalogError::invalidated(
                CatalogInvalidationReason::RecordInvariant,
                "Zone evidence count overflow",
            )
        })?;
    }
    let mut zone_ids = HashSet::new();
    for zone in &snapshot.zones {
        validate_persisted(validate_zone(zone), "Zone")?;
        zone_ids.insert(zone.channel_id.as_str());
        let Some(segment) =
            segments_by_id.get(zone.snapshot_provenance.coverage_segment_id.as_str())
        else {
            return persisted_invariant(format!(
                "Zone {} references missing coverage segment {}",
                zone.channel_id, zone.snapshot_provenance.coverage_segment_id
            ));
        };
        if zone.snapshot_provenance.observed_slot < segment.floor.slot
            || zone.snapshot_provenance.observed_slot > segment.frontier.slot
        {
            return persisted_invariant(format!(
                "Zone {} snapshot falls outside coverage segment {}",
                zone.channel_id, zone.snapshot_provenance.coverage_segment_id
            ));
        }
        let Some(latest_evidence) = evidence_by_id.get(zone.latest_evidence_id.as_str()) else {
            return persisted_invariant(format!(
                "Zone {} references missing latest evidence {}",
                zone.channel_id, zone.latest_evidence_id
            ));
        };
        if latest_evidence.channel_id != zone.channel_id {
            return persisted_invariant(format!(
                "Zone {} latest evidence belongs to Channel {}",
                zone.channel_id, latest_evidence.channel_id
            ));
        }
        let persisted_count = evidence_counts
            .get(zone.channel_id.as_str())
            .copied()
            .unwrap_or_default();
        if persisted_count != zone.evidence_count {
            return persisted_invariant(format!(
                "Zone {} evidence count is {}, but {} references are persisted",
                zone.channel_id, zone.evidence_count, persisted_count
            ));
        }
    }
    if let Some(orphan_channel_id) = evidence_counts
        .keys()
        .find(|channel_id| !zone_ids.contains(**channel_id))
    {
        return persisted_invariant(format!(
            "evidence exists for missing Zone {orphan_channel_id}"
        ));
    }
    for gap in &snapshot.gaps {
        validate_persisted(validate_gap(gap), "coverage gap")?;
        let Some(lower_segment) = segments_by_id.get(gap.lower_segment_id.as_str()) else {
            return persisted_invariant(format!(
                "coverage gap {} references missing lower segment {}",
                gap.gap_id, gap.lower_segment_id
            ));
        };
        let Some(upper_segment) = segments_by_id.get(gap.upper_segment_id.as_str()) else {
            return persisted_invariant(format!(
                "coverage gap {} references missing upper segment {}",
                gap.gap_id, gap.upper_segment_id
            ));
        };
        if lower_segment.frontier != gap.lower_checkpoint
            || upper_segment.floor.slot != gap.upper_block.slot
            || upper_segment.floor.block_id != gap.upper_block.block_id
            || upper_segment.floor.parent_id != gap.required_parent_id
        {
            return persisted_invariant(format!(
                "coverage gap {} does not match its segment boundaries",
                gap.gap_id
            ));
        }
    }
    Ok(())
}

fn validate_persisted(result: CatalogResult<()>, label: &str) -> CatalogResult<()> {
    result.map_err(|error| {
        CatalogError::invalidated(
            CatalogInvalidationReason::RecordInvariant,
            format!("persisted {label} is invalid: {error}"),
        )
    })
}

fn persisted_invariant<T>(detail: impl Into<String>) -> CatalogResult<T> {
    Err(CatalogError::invalidated(
        CatalogInvalidationReason::RecordInvariant,
        detail,
    ))
}

fn map_staged_error(error: CatalogError) -> CatalogError {
    match error {
        CatalogError::Invalidated(invalidation) => CatalogError::invalid_input(format!(
            "catalog batch violates persisted invariant: {}",
            invalidation.detail
        )),
        other => other,
    }
}

fn validate_batch(batch: &CatalogBatch) -> CatalogResult<()> {
    if let Some(frontier) = batch.frontier.as_ref() {
        validate_frontier(frontier)?;
    }
    if let Some(traversal) = batch.traversal.as_ref() {
        validate_traversal(traversal)?;
    }
    validate_record_keys(
        &batch.upsert_zones,
        &batch.delete_zone_ids,
        |record| record.channel_id.as_str(),
        validate_zone,
        |key| validate_hex_id(key, "deleted Channel id"),
        "Zone",
    )?;
    validate_record_keys(
        &batch.upsert_evidence,
        &batch.delete_evidence_ids,
        |reference| reference.evidence_id.as_str(),
        validate_evidence,
        |key| validate_local_id(key, "deleted evidence id"),
        "evidence reference",
    )?;
    validate_record_keys(
        &batch.upsert_segments,
        &batch.delete_segment_ids,
        |segment| segment.segment_id.as_str(),
        validate_segment,
        |key| validate_local_id(key, "deleted coverage segment id"),
        "coverage segment",
    )?;
    validate_record_keys(
        &batch.upsert_gaps,
        &batch.delete_gap_ids,
        |gap| gap.gap_id.as_str(),
        validate_gap,
        |key| validate_local_id(key, "deleted coverage gap id"),
        "coverage gap",
    )
}

fn validate_record_keys<T, K, V, D>(
    upserts: &[T],
    deletes: &[String],
    key_of: K,
    validate: V,
    validate_delete: D,
    label: &str,
) -> CatalogResult<()>
where
    K: Fn(&T) -> &str,
    V: Fn(&T) -> CatalogResult<()>,
    D: Fn(&str) -> CatalogResult<()>,
{
    let mut upsert_keys = HashSet::new();
    for record in upserts {
        validate(record)?;
        let key = key_of(record);
        if !upsert_keys.insert(key) {
            return Err(CatalogError::invalid_input(format!(
                "duplicate {label} upsert key {key}"
            )));
        }
    }
    let mut delete_keys = HashSet::new();
    for key in deletes {
        validate_delete(key)?;
        if !delete_keys.insert(key.as_str()) {
            return Err(CatalogError::invalid_input(format!(
                "duplicate {label} delete key {key}"
            )));
        }
        if upsert_keys.contains(key.as_str()) {
            return Err(CatalogError::invalid_input(format!(
                "{label} key {key} is both upserted and deleted"
            )));
        }
    }
    Ok(())
}

fn apply_batch_records(
    transaction: &redb::WriteTransaction,
    batch: &CatalogBatch,
) -> CatalogResult<()> {
    {
        let mut table = transaction
            .open_table(ZONES_TABLE)
            .map_err(map_table_error)?;
        upsert_records(&mut table, &batch.upsert_zones, |record| {
            record.channel_id.as_str()
        })?;
        delete_records(&mut table, &batch.delete_zone_ids)?;
    }
    {
        let mut table = transaction
            .open_table(EVIDENCE_TABLE)
            .map_err(map_table_error)?;
        upsert_records(&mut table, &batch.upsert_evidence, |reference| {
            reference.evidence_id.as_str()
        })?;
        delete_records(&mut table, &batch.delete_evidence_ids)?;
    }
    {
        let mut table = transaction
            .open_table(SEGMENTS_TABLE)
            .map_err(map_table_error)?;
        upsert_records(&mut table, &batch.upsert_segments, |segment| {
            segment.segment_id.as_str()
        })?;
        delete_records(&mut table, &batch.delete_segment_ids)?;
    }
    {
        let mut table = transaction
            .open_table(GAPS_TABLE)
            .map_err(map_table_error)?;
        upsert_records(&mut table, &batch.upsert_gaps, |gap| gap.gap_id.as_str())?;
        delete_records(&mut table, &batch.delete_gap_ids)?;
    }
    Ok(())
}

fn upsert_records<T, F>(
    table: &mut redb::Table<'_, &str, &[u8]>,
    records: &[T],
    key_of: F,
) -> CatalogResult<()>
where
    T: Serialize,
    F: Fn(&T) -> &str,
{
    for record in records {
        insert_versioned(table, key_of(record), record)?;
    }
    Ok(())
}

fn delete_records(table: &mut redb::Table<'_, &str, &[u8]>, keys: &[String]) -> CatalogResult<()> {
    for key in keys {
        drop(table.remove(key.as_str()).map_err(map_storage_error)?);
    }
    Ok(())
}

fn replace_optional_versioned<T: Serialize>(
    table: &mut redb::Table<'_, &str, &[u8]>,
    key: &str,
    value: Option<&T>,
) -> CatalogResult<()> {
    if let Some(value) = value {
        insert_versioned(table, key, value)
    } else {
        drop(table.remove(key).map_err(map_storage_error)?);
        Ok(())
    }
}

fn insert_versioned<T: Serialize>(
    table: &mut redb::Table<'_, &str, &[u8]>,
    key: &str,
    value: &T,
) -> CatalogResult<()> {
    let record = VersionedRecord {
        record_version: CATALOG_RECORD_VERSION,
        value,
    };
    insert_raw(table, key, &record)
}

fn insert_raw<T: Serialize>(
    table: &mut redb::Table<'_, &str, &[u8]>,
    key: &str,
    value: &T,
) -> CatalogResult<()> {
    let bytes = serde_json::to_vec(value).map_err(CatalogError::storage)?;
    drop(
        table
            .insert(key, bytes.as_slice())
            .map_err(map_storage_error)?,
    );
    Ok(())
}

fn map_database_error(error: DatabaseError) -> CatalogError {
    match error {
        DatabaseError::Storage(storage) => map_storage_error(storage),
        DatabaseError::RepairAborted | DatabaseError::UpgradeRequired(_) => {
            CatalogError::invalidated(
                CatalogInvalidationReason::DatabaseUnreadable,
                error.to_string(),
            )
        }
        other => CatalogError::storage(other),
    }
}

fn map_table_error(error: TableError) -> CatalogError {
    match error {
        TableError::Storage(storage) => map_storage_error(storage),
        other @ (TableError::TableTypeMismatch { .. }
        | TableError::TableIsMultimap(_)
        | TableError::TableIsNotMultimap(_)
        | TableError::TypeDefinitionChanged { .. }
        | TableError::TableDoesNotExist(_)) => {
            CatalogError::invalidated(CatalogInvalidationReason::TableSchema, other.to_string())
        }
        other => CatalogError::storage(other),
    }
}

fn map_storage_error(error: StorageError) -> CatalogError {
    if matches!(&error, StorageError::Corrupted(_))
        || matches!(
            &error,
            StorageError::Io(io_error)
                if matches!(
                    io_error.kind(),
                    std::io::ErrorKind::InvalidData | std::io::ErrorKind::UnexpectedEof
                )
        )
    {
        CatalogError::invalidated(
            CatalogInvalidationReason::DatabaseUnreadable,
            error.to_string(),
        )
    } else {
        CatalogError::storage(error)
    }
}

fn map_transaction_error(error: TransactionError) -> CatalogError {
    match error {
        TransactionError::Storage(storage) => map_storage_error(storage),
        other => CatalogError::storage(other),
    }
}

fn map_commit_error(error: CommitError) -> CatalogError {
    match error {
        CommitError::Storage(storage) => map_storage_error(storage),
        other => CatalogError::storage(other),
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result, bail, ensure};
    use redb::TableDefinition;

    use super::*;
    use crate::inspection::catalog::model::{
        CatalogBlockCheckpoint, CatalogBlockReference, CatalogEvidenceUse, CatalogFrontier,
        CatalogSnapshotOrigin, CatalogSnapshotProvenance, CatalogTraversal,
        ZoneClassificationCounters, ZoneEvidenceKind,
    };
    use crate::inspection::zones::{
        CatalogCoverageStatus, CoveragePrefixStatus, L1ChannelSummary, L1FinalityState,
        NetworkScope, SequencerCommitteeSummary,
    };

    #[test]
    fn create_writes_current_schema_and_reopens() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        let metadata = test_metadata()?;
        let store = ZoneCatalogStore::create(&path, metadata.clone())?;

        let snapshot = store.snapshot()?;
        ensure!(metadata == snapshot.metadata, "metadata changed on create");
        ensure!(snapshot.zones.is_empty(), "new catalog contains Zones");
        ensure!(
            snapshot.evidence.is_empty(),
            "new catalog contains evidence"
        );
        ensure!(
            snapshot.segments.is_empty(),
            "new catalog contains coverage segments"
        );
        ensure!(snapshot.gaps.is_empty(), "new catalog contains gaps");
        drop(store);

        let schema_bytes = read_metadata_bytes(&path, SCHEMA_KEY)?;
        let schema: CatalogSchemaMetadata = serde_json::from_slice(&schema_bytes)?;
        ensure!(
            CatalogSchemaMetadata::current() == schema,
            "new catalog schema is not current"
        );

        let reopened = ZoneCatalogStore::open(&path)?;
        ensure!(
            snapshot == reopened.snapshot()?,
            "reopened snapshot changed"
        );
        Ok(())
    }

    #[test]
    fn durable_batch_commits_records_coverage_and_cursor_together() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        let store = ZoneCatalogStore::create(&path, test_metadata()?)?;

        let committed = store.commit_batch(test_batch())?;
        ensure!(
            committed.metadata.catalog_revision == 1,
            "batch did not advance catalog revision"
        );
        ensure!(
            committed.metadata.updated_at_unix == 101,
            "batch did not persist update time"
        );
        ensure!(committed.zones.len() == 1, "Zone row was not committed");
        ensure!(
            committed.evidence.len() == 1,
            "evidence row was not committed"
        );
        ensure!(
            committed.segments.len() == 1,
            "coverage segment was not committed"
        );
        ensure!(committed.frontier.is_some(), "frontier was not committed");
        ensure!(committed.traversal.is_some(), "cursor was not committed");
        drop(store);

        let reopened = ZoneCatalogStore::open(&path)?;
        ensure!(
            committed == reopened.snapshot()?,
            "durable snapshot changed after reopen"
        );
        Ok(())
    }

    #[test]
    fn stale_revision_rejects_batch_without_mutation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        let store = ZoneCatalogStore::create(&path, test_metadata()?)?;
        let committed = store.commit_batch(test_batch())?;

        let mut stale_batch = test_batch();
        stale_batch.updated_at_unix = 102;
        let result = store.commit_batch(stale_batch);
        ensure!(
            matches!(
                result,
                Err(CatalogError::RevisionConflict {
                    expected: 0,
                    current: 1
                })
            ),
            "stale batch did not return revision conflict"
        );
        ensure!(
            committed == store.snapshot()?,
            "stale batch mutated catalog"
        );
        Ok(())
    }

    #[test]
    fn failure_before_commit_rolls_back_rows_coverage_cursor_and_revision() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        let store = ZoneCatalogStore::create(&path, test_metadata()?)?;

        let result = store.commit_batch_with_hook(test_batch(), || {
            Err(CatalogError::storage("injected pre-commit failure"))
        });
        ensure!(
            matches!(result, Err(CatalogError::Storage(_))),
            "injected failure did not escape"
        );

        let snapshot = store.snapshot()?;
        ensure!(
            snapshot.metadata.catalog_revision == 0,
            "failed batch advanced revision"
        );
        ensure!(snapshot.zones.is_empty(), "failed batch left Zone rows");
        ensure!(
            snapshot.evidence.is_empty(),
            "failed batch left evidence rows"
        );
        ensure!(
            snapshot.segments.is_empty(),
            "failed batch left coverage segments"
        );
        ensure!(snapshot.frontier.is_none(), "failed batch left frontier");
        ensure!(snapshot.traversal.is_none(), "failed batch left cursor");
        Ok(())
    }

    #[test]
    fn relationally_invalid_batch_is_rejected_without_invalidating_catalog() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        let store = ZoneCatalogStore::create(&path, test_metadata()?)?;
        let mut batch = test_batch();
        batch.upsert_evidence.clear();

        ensure!(
            matches!(
                store.commit_batch(batch),
                Err(CatalogError::InvalidInput(_))
            ),
            "invalid batch did not return input error"
        );
        let snapshot = store.snapshot()?;
        ensure!(
            snapshot.metadata.catalog_revision == 0,
            "invalid batch advanced revision"
        );
        ensure!(snapshot.zones.is_empty(), "invalid batch left Zone rows");
        ensure!(
            snapshot.segments.is_empty(),
            "invalid batch left coverage rows"
        );
        Ok(())
    }

    #[test]
    fn read_only_catalog_exposes_snapshot_and_rejects_writes() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        let writable = ZoneCatalogStore::create(&path, test_metadata()?)?;
        let committed = writable.commit_batch(test_batch())?;
        drop(writable);

        let read_only = ZoneCatalogStore::open_read_only(&path)?;
        ensure!(read_only.is_read_only(), "catalog did not open read-only");
        ensure!(
            committed == read_only.snapshot()?,
            "read-only snapshot changed"
        );

        let mut batch = test_batch();
        batch.expected_catalog_revision = 1;
        batch.updated_at_unix = 102;
        ensure!(
            matches!(
                read_only.commit_batch(batch),
                Err(CatalogError::InvalidInput(_))
            ),
            "read-only catalog accepted a write"
        );
        Ok(())
    }

    #[test]
    fn missing_schema_invalidates_catalog() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        drop(ZoneCatalogStore::create(&path, test_metadata()?)?);

        let database = Database::open(&path)?;
        let transaction = database.begin_write()?;
        {
            let mut table = transaction.open_table(METADATA_TABLE)?;
            drop(table.remove(SCHEMA_KEY)?);
        }
        transaction.commit()?;
        drop(database);

        assert_invalidation(
            ZoneCatalogStore::open(&path),
            CatalogInvalidationReason::SchemaMissing,
        )?;
        Ok(())
    }

    #[test]
    fn incompatible_schema_version_invalidates_catalog() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        drop(ZoneCatalogStore::create(&path, test_metadata()?)?);

        write_raw_metadata(
            &path,
            SCHEMA_KEY,
            &CatalogSchemaMetadata {
                schema_version: CATALOG_SCHEMA_VERSION + 1,
                record_version: CATALOG_RECORD_VERSION,
            },
        )?;
        assert_invalidation(
            ZoneCatalogStore::open(&path),
            CatalogInvalidationReason::SchemaVersion,
        )?;
        Ok(())
    }

    #[test]
    fn incompatible_record_version_invalidates_catalog() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        let metadata = test_metadata()?;
        drop(ZoneCatalogStore::create(&path, metadata.clone())?);

        write_raw_metadata(
            &path,
            CATALOG_KEY,
            &VersionedRecord {
                record_version: CATALOG_RECORD_VERSION + 1,
                value: metadata,
            },
        )?;
        assert_invalidation(
            ZoneCatalogStore::open(&path),
            CatalogInvalidationReason::RecordVersion,
        )?;
        Ok(())
    }

    #[test]
    fn undecodable_record_invalidates_catalog() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        drop(ZoneCatalogStore::create(&path, test_metadata()?)?);

        let database = Database::open(&path)?;
        let transaction = database.begin_write()?;
        {
            let mut table = transaction.open_table(METADATA_TABLE)?;
            drop(table.insert(CATALOG_KEY, b"not-json".as_slice())?);
        }
        transaction.commit()?;
        drop(database);

        assert_invalidation(
            ZoneCatalogStore::open(&path),
            CatalogInvalidationReason::RecordDecode,
        )?;
        Ok(())
    }

    #[test]
    fn incompatible_table_definition_invalidates_catalog() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        drop(ZoneCatalogStore::create(&path, test_metadata()?)?);

        let database = Database::open(&path)?;
        let transaction = database.begin_write()?;
        ensure!(
            transaction.delete_table(ZONES_TABLE)?,
            "expected Zones table to exist"
        );
        let wrong_table: TableDefinition<'static, u64, u64> =
            TableDefinition::new("zone_catalog_zones_v1");
        drop(transaction.open_table(wrong_table)?);
        transaction.commit()?;
        drop(database);

        assert_invalidation(
            ZoneCatalogStore::open(&path),
            CatalogInvalidationReason::TableSchema,
        )?;
        Ok(())
    }

    #[test]
    fn corrupt_database_invalidates_catalog() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        fs::write(&path, b"not-a-redb-database")?;

        assert_invalidation(
            ZoneCatalogStore::open(&path),
            CatalogInvalidationReason::DatabaseUnreadable,
        )?;
        Ok(())
    }

    #[test]
    fn dangling_record_relationship_invalidates_catalog() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        let store = ZoneCatalogStore::create(&path, test_metadata()?)?;
        let committed = store.commit_batch(test_batch())?;
        let mut zone = committed
            .zones
            .first()
            .cloned()
            .context("committed Zone should exist")?;
        zone.evidence_count = 2;
        drop(store);

        let database = Database::open(&path)?;
        let transaction = database.begin_write()?;
        {
            let mut table = transaction.open_table(ZONES_TABLE)?;
            insert_versioned(&mut table, &zone.channel_id, &zone)?;
        }
        transaction.commit()?;
        drop(database);

        assert_invalidation(
            ZoneCatalogStore::open(&path),
            CatalogInvalidationReason::RecordInvariant,
        )?;
        Ok(())
    }

    fn test_metadata() -> CatalogResult<CatalogMetadata> {
        CatalogMetadata::new(
            NetworkScope::GenesisId {
                genesis_id: hex_id('a'),
            },
            100,
        )
    }

    fn test_batch() -> CatalogBatch {
        let channel_id = hex_id('3');
        let segment_id = "segment-1".to_owned();
        let evidence_id = "evidence-1".to_owned();
        let block_id = hex_id('2');
        let segment = CoverageSegment {
            segment_id: segment_id.clone(),
            floor: CatalogBlockCheckpoint {
                slot: 0,
                block_id: hex_id('0'),
                parent_id: hex_id('f'),
            },
            frontier: CatalogBlockReference {
                slot: 10,
                block_id: block_id.clone(),
            },
            reaches_target_lib: true,
        };
        let evidence = ZoneEvidenceReference {
            evidence_id: evidence_id.clone(),
            channel_id: channel_id.clone(),
            coverage_segment_id: segment_id.clone(),
            l1_slot: 10,
            block_id: block_id.clone(),
            transaction_hash: Some(hex_id('4')),
            operation_index: 1,
            message_id: Some("message-1".to_owned()),
            evidence_kind: ZoneEvidenceKind::SequencerBlock,
            evidence_use: CatalogEvidenceUse::ReplayContribution,
        };
        let zone = ZoneCatalogRecord {
            channel_id,
            observed_label: Some("Test Zone".to_owned()),
            l1_channel: L1ChannelSummary {
                tip_slot: Some(10),
                tip_hash: Some(block_id.clone()),
                lib_slot: Some(10),
                balance: Some("1000".to_owned()),
                key_count: Some(1),
                withdraw_threshold: Some("1".to_owned()),
                operation_count: 1,
                finality_state: L1FinalityState::Final,
            },
            sequencer_committee: Some(SequencerCommitteeSummary {
                members: vec!["committee-key-1".to_owned()],
                active_member: Some("committee-key-1".to_owned()),
                observed_at_slot: Some(10),
            }),
            classification: ZoneClassificationCounters {
                channel_operations: 1,
                recognized_l2_blocks: 1,
                raw_inscriptions: 0,
                conflicting_evidence: false,
            },
            first_seen_slot: 3,
            last_seen_slot: 10,
            latest_evidence_id: evidence_id,
            evidence_count: 1,
            snapshot_provenance: CatalogSnapshotProvenance {
                origin: CatalogSnapshotOrigin::ReplayDerived,
                coverage_segment_id: segment_id,
                observed_slot: 10,
                source_revision: 7,
            },
            updated_at_unix: 101,
        };
        let checkpoint = CatalogBlockCheckpoint {
            slot: 10,
            block_id: block_id.clone(),
            parent_id: hex_id('1'),
        };
        let target = CatalogBlockReference { slot: 10, block_id };
        CatalogBatch {
            expected_catalog_revision: 0,
            updated_at_unix: 101,
            upsert_zones: vec![zone],
            delete_zone_ids: Vec::new(),
            upsert_evidence: vec![evidence],
            delete_evidence_ids: Vec::new(),
            upsert_segments: vec![segment],
            delete_segment_ids: Vec::new(),
            upsert_gaps: Vec::new(),
            delete_gap_ids: Vec::new(),
            frontier: Some(CatalogFrontier {
                scanned_through_slot: Some(10),
                checkpoint: Some(checkpoint),
                observed_lib: Some(target.clone()),
                coverage_floor: Some(0),
                prefix_status: CoveragePrefixStatus::Complete,
                coverage_status: CatalogCoverageStatus::Complete,
            }),
            traversal: Some(CatalogTraversal {
                target_lib: Some(target.clone()),
                ingestion_cursor: Some(target),
            }),
        }
    }

    fn hex_id(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn read_metadata_bytes(path: &Path, key: &str) -> Result<Vec<u8>> {
        let database = Database::open(path)?;
        let transaction = database.begin_read()?;
        let table = transaction.open_table(METADATA_TABLE)?;
        let value = table
            .get(key)?
            .with_context(|| format!("missing metadata key {key}"))?;
        Ok(value.value().to_vec())
    }

    fn write_raw_metadata<T: Serialize>(path: &Path, key: &str, value: &T) -> Result<()> {
        let database = Database::open(path)?;
        let transaction = database.begin_write()?;
        {
            let mut table = transaction.open_table(METADATA_TABLE)?;
            let bytes = serde_json::to_vec(value)?;
            drop(table.insert(key, bytes.as_slice())?);
        }
        transaction.commit()?;
        Ok(())
    }

    fn assert_invalidation(
        result: CatalogResult<ZoneCatalogStore>,
        expected: CatalogInvalidationReason,
    ) -> Result<()> {
        match result {
            Err(CatalogError::Invalidated(invalidation)) => {
                ensure!(
                    expected == invalidation.reason,
                    "expected {expected:?} invalidation, got {:?}",
                    invalidation.reason
                );
                Ok(())
            }
            Err(error) => bail!("expected catalog invalidation, got {error}"),
            Ok(_) => bail!("expected catalog invalidation, catalog opened"),
        }
    }
}
