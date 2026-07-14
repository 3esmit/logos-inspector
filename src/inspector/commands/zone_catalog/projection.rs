use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Context as _, Result, bail};

use crate::{
    inspection::{
        CatalogVerificationState, NetworkScope, ZoneProjectionSnapshot, ZoneSummary,
        catalog::{
            CatalogSnapshot, ChannelSourceAttestationWarning, ChannelSourceConfigReport,
            ZONE_CATALOG_REPORT_SCHEMA_VERSION, ZoneCatalogServiceReport, ZoneDetailReport,
            ZoneDetailRequest, ZoneSummaryChanges, ZonesSummaryReport, ZonesSummaryRequest,
        },
        l2::ZoneL2RuntimeFacts,
    },
    source_routing::channel_sources::{ChannelSourceConfig, ChannelSourceMonitorSnapshot},
};

const DEFAULT_SUMMARY_PAGE_SIZE: usize = 200;
const MAX_SUMMARY_PAGE_SIZE: usize = 500;
const MAX_SUMMARY_JOURNAL_REVISIONS: usize = 64;
const MAX_SUMMARY_JOURNAL_CHANGES: usize = 4_096;
const MAX_SUMMARY_CURSORS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectionKey {
    source_revision: u64,
    network_scope: Option<NetworkScope>,
    catalog_revision: u64,
    source_config_epoch: u64,
    observation_revision: u64,
    verification: CatalogVerificationState,
}

#[derive(Default)]
pub(super) struct ZoneProjectionLedger {
    projection_key: Option<ProjectionKey>,
    source_config_epoch: u64,
    projection: ZoneProjectionSnapshot,
    summary_revision: u64,
    delta_floor_revision: u64,
    detail_revisions: BTreeMap<String, u64>,
    journal: VecDeque<SummaryJournalEntry>,
    journal_change_count: usize,
    cursors: BTreeMap<String, SummaryCursor>,
    cursor_order: VecDeque<String>,
}

impl ZoneProjectionLedger {
    pub(super) fn refresh(
        &mut self,
        service: &ZoneCatalogServiceReport,
        configs: Vec<ChannelSourceConfig>,
        observations: ChannelSourceMonitorSnapshot,
    ) -> Result<()> {
        if self.projection.configs() != configs {
            self.source_config_epoch = self
                .source_config_epoch
                .checked_add(1)
                .context("Channel source configuration epoch overflow")?;
        }
        let network_scope = service
            .catalog
            .as_deref()
            .map(|catalog| catalog.metadata.network_scope.clone());
        let catalog_revision = service
            .catalog
            .as_deref()
            .map_or(0, |catalog| catalog.metadata.catalog_revision);
        let key = ProjectionKey {
            source_revision: service.source_revision,
            network_scope,
            catalog_revision,
            source_config_epoch: self.source_config_epoch,
            observation_revision: observations.observation_revision,
            verification: service.verification_state,
        };
        let next_projection = ZoneProjectionSnapshot::project(
            service.catalog.as_deref(),
            configs,
            observations,
            service.verification_state,
        );
        let next_rows = next_projection.summary_map();
        if self.projection_key.as_ref() == Some(&key) {
            self.projection = next_projection;
            return Ok(());
        }
        let initial_empty = self.projection_key.is_none()
            && key.source_revision == 0
            && key.network_scope.is_none()
            && key.catalog_revision == 0
            && key.source_config_epoch == 0
            && key.observation_revision == 0
            && next_rows.is_empty();
        let identity_changed = self.projection_key.as_ref().map_or(
            key.source_revision != 0 || key.network_scope.is_some(),
            |previous| {
                previous.source_revision != key.source_revision
                    || previous.network_scope != key.network_scope
            },
        );
        self.projection_key = Some(key.clone());
        if initial_empty {
            self.projection = next_projection;
            return Ok(());
        }

        let next_revision = self
            .summary_revision
            .checked_add(1)
            .context("Zone summary revision overflow")?;
        if identity_changed {
            self.delta_floor_revision = next_revision;
        }
        let upserts = next_rows
            .iter()
            .filter(|(channel_id, row)| self.projection.summary(channel_id) != Some(*row))
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>();
        let removed_zone_ids = self
            .projection
            .summaries()
            .map(|summary| &summary.channel_id)
            .filter(|channel_id| !next_rows.contains_key(*channel_id))
            .cloned()
            .collect::<Vec<_>>();
        for channel_id in next_rows.keys() {
            let revision = self.detail_revisions.entry(channel_id.clone()).or_insert(0);
            *revision = revision
                .checked_add(1)
                .context("Zone detail revision overflow")?;
        }
        for channel_id in &removed_zone_ids {
            self.detail_revisions.remove(channel_id);
        }
        let change_count = upserts.len().saturating_add(removed_zone_ids.len());
        self.journal.push_back(SummaryJournalEntry {
            revision: next_revision,
            source_revision: key.source_revision,
            network_scope: key.network_scope,
            upserts,
            removed_zone_ids,
        });
        self.journal_change_count = self.journal_change_count.saturating_add(change_count);
        self.trim_journal();
        self.summary_revision = next_revision;
        self.projection = next_projection;
        Ok(())
    }

    #[must_use]
    pub(super) const fn source_config_epoch(&self) -> u64 {
        self.source_config_epoch
    }

    #[must_use]
    pub(super) const fn summary_revision(&self) -> u64 {
        self.summary_revision
    }

    #[must_use]
    pub(super) fn observation_revision(&self) -> u64 {
        self.projection.observations().observation_revision
    }

    #[must_use]
    pub(super) fn contains_channel(&self, channel_id: &str) -> bool {
        self.projection.summary(channel_id).is_some()
    }

    pub(super) fn summaries(&mut self, request: ZonesSummaryRequest) -> Result<ZonesSummaryReport> {
        let limit = summary_page_limit(request.limit)?;
        let page = if let Some(cursor) = request.cursor.as_deref() {
            let cursor = self
                .cursors
                .remove(cursor)
                .context("Zone summary cursor is invalid or expired")?;
            validate_summary_request_fences(
                request.source_revision,
                request.network_scope.as_ref(),
                &cursor.snapshot,
            )?;
            CursorPage {
                snapshot: cursor.snapshot,
                offset: cursor.offset,
            }
        } else {
            self.validate_current_summary_fences(&request)?;
            CursorPage {
                snapshot: self.summary_snapshot(request.after_summary_revision),
                offset: 0,
            }
        };
        let (changes, next_offset) = page.snapshot.page(page.offset, limit);
        let next_cursor = next_offset
            .map(|offset| self.insert_cursor(page.snapshot.clone(), offset))
            .transpose()?;
        Ok(page.snapshot.report(changes, next_cursor))
    }

    pub(super) fn detail_report(
        &self,
        request: ZoneDetailRequest,
        service: &ZoneCatalogServiceReport,
        catalog: &CatalogSnapshot,
    ) -> Result<ZoneDetailReport> {
        self.validate_detail_fences(&request, service, catalog)?;
        let detail_revision = self
            .detail_revisions
            .get(&request.channel_id)
            .copied()
            .context("Zone does not exist in current catalog projection")?;
        let detail = self
            .projection
            .detail(&request.channel_id, detail_revision)
            .context("Zone does not exist in current catalog projection")?;
        Ok(ZoneDetailReport {
            report_kind: "zones.zone_detail",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: service.source_revision,
            network_scope: catalog.metadata.network_scope.clone(),
            catalog_revision: catalog.metadata.catalog_revision,
            source_config_epoch: self.source_config_epoch,
            observation_revision: self.observation_revision(),
            summary_revision: self.summary_revision,
            detail,
        })
    }

    pub(super) fn source_config_report(
        &self,
        service: &ZoneCatalogServiceReport,
        config: ChannelSourceConfig,
        attestation_warning: Option<ChannelSourceAttestationWarning>,
    ) -> Result<ChannelSourceConfigReport> {
        let summary = self
            .projection
            .summary(&config.channel_id)
            .context("updated Channel disappeared from Zone projection")?;
        let sources = self
            .projection
            .sources(&config.channel_id)
            .context("updated Channel source projection disappeared")?;
        Ok(ChannelSourceConfigReport {
            report_kind: "zones.channel_source_config",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: service.source_revision,
            catalog_revision: service
                .catalog
                .as_deref()
                .map_or(0, |catalog| catalog.metadata.catalog_revision),
            source_config_epoch: self.source_config_epoch,
            observation_revision: self.observation_revision(),
            summary_revision: self.summary_revision,
            active_zone_context_fields: summary.active_zone_context_fields.clone(),
            config,
            observations: sources.observations.clone(),
            agreement: sources.agreement.clone(),
            attestation_warning,
        })
    }

    #[must_use]
    pub(super) fn runtime_facts(&self) -> ZoneL2RuntimeFacts {
        ZoneL2RuntimeFacts {
            network_scope: self.projection.network_scope().cloned(),
            verification: self.projection.verification(),
            summaries: self.projection.summary_map(),
            configs: self.projection.configs().to_vec(),
            observations: self.projection.observations().clone(),
        }
    }

    fn trim_journal(&mut self) {
        while self.journal.len() > MAX_SUMMARY_JOURNAL_REVISIONS
            || self.journal_change_count > MAX_SUMMARY_JOURNAL_CHANGES
        {
            let Some(removed) = self.journal.pop_front() else {
                break;
            };
            self.journal_change_count = self
                .journal_change_count
                .saturating_sub(removed.change_count());
        }
    }

    fn summary_snapshot(&self, after_revision: Option<u64>) -> SummarySnapshot {
        let key = self.projection_key.clone().unwrap_or(ProjectionKey {
            source_revision: 0,
            network_scope: None,
            catalog_revision: 0,
            source_config_epoch: 0,
            observation_revision: 0,
            verification: CatalogVerificationState::Empty,
        });
        let changes = after_revision
            .and_then(|revision| self.delta_since(revision, &key))
            .unwrap_or_else(|| SummarySnapshotChanges::Reset {
                rows: self.projection.summaries().cloned().collect(),
            });
        SummarySnapshot {
            source_revision: key.source_revision,
            network_scope: key.network_scope,
            catalog_revision: key.catalog_revision,
            source_config_epoch: key.source_config_epoch,
            observation_revision: key.observation_revision,
            summary_revision: self.summary_revision,
            changes,
        }
    }

    fn delta_since(
        &self,
        after_revision: u64,
        key: &ProjectionKey,
    ) -> Option<SummarySnapshotChanges> {
        if after_revision > self.summary_revision || after_revision < self.delta_floor_revision {
            return None;
        }
        if after_revision == self.summary_revision {
            return Some(SummarySnapshotChanges::Delta {
                upserts: Vec::new(),
                removed_zone_ids: Vec::new(),
            });
        }
        let first_required = after_revision.checked_add(1)?;
        let first = self.journal.iter().find(|entry| {
            entry.revision == first_required
                && entry.source_revision == key.source_revision
                && entry.network_scope == key.network_scope
        })?;
        let mut expected = first.revision;
        let mut upserts = BTreeMap::new();
        let mut removed = BTreeSet::new();
        for entry in self.journal.iter().filter(|entry| {
            entry.revision >= first_required && entry.revision <= self.summary_revision
        }) {
            if entry.revision != expected
                || entry.source_revision != key.source_revision
                || entry.network_scope != key.network_scope
            {
                return None;
            }
            for channel_id in &entry.removed_zone_ids {
                upserts.remove(channel_id);
                removed.insert(channel_id.clone());
            }
            for row in &entry.upserts {
                removed.remove(&row.channel_id);
                upserts.insert(row.channel_id.clone(), row.clone());
            }
            expected = expected.checked_add(1)?;
        }
        if expected != self.summary_revision.checked_add(1)? {
            return None;
        }
        Some(SummarySnapshotChanges::Delta {
            upserts: upserts.into_values().collect(),
            removed_zone_ids: removed.into_iter().collect(),
        })
    }

    fn insert_cursor(&mut self, snapshot: SummarySnapshot, offset: usize) -> Result<String> {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).context("failed to create Zone summary cursor")?;
        let token = format!("zsc1_{}", hex::encode(random));
        self.cursors
            .insert(token.clone(), SummaryCursor { snapshot, offset });
        self.cursor_order.push_back(token.clone());
        while self.cursor_order.len() > MAX_SUMMARY_CURSORS {
            if let Some(expired) = self.cursor_order.pop_front() {
                self.cursors.remove(&expired);
            }
        }
        Ok(token)
    }

    fn validate_current_summary_fences(&self, request: &ZonesSummaryRequest) -> Result<()> {
        let key = self
            .projection_key
            .as_ref()
            .context("Zone summary projection is unavailable")?;
        if request.source_revision != key.source_revision {
            bail!("Zone summary source revision is stale");
        }
        if request.network_scope.as_ref() != key.network_scope.as_ref() {
            bail!("Zone summary network scope is stale");
        }
        if request
            .after_summary_revision
            .is_some_and(|revision| revision > self.summary_revision)
        {
            bail!("Zone summary revision is newer than current state");
        }
        Ok(())
    }

    fn validate_detail_fences(
        &self,
        request: &ZoneDetailRequest,
        service: &ZoneCatalogServiceReport,
        catalog: &CatalogSnapshot,
    ) -> Result<()> {
        if request.source_revision != service.source_revision
            || request.network_scope != catalog.metadata.network_scope
            || request.catalog_revision != catalog.metadata.catalog_revision
            || request.summary_revision != self.summary_revision
            || request.observation_revision != self.observation_revision()
        {
            bail!("Zone detail request fences are stale");
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct SummaryJournalEntry {
    revision: u64,
    source_revision: u64,
    network_scope: Option<NetworkScope>,
    upserts: Vec<ZoneSummary>,
    removed_zone_ids: Vec<String>,
}

impl SummaryJournalEntry {
    fn change_count(&self) -> usize {
        self.upserts
            .len()
            .saturating_add(self.removed_zone_ids.len())
    }
}

#[derive(Debug, Clone)]
struct SummarySnapshot {
    source_revision: u64,
    network_scope: Option<NetworkScope>,
    catalog_revision: u64,
    source_config_epoch: u64,
    observation_revision: u64,
    summary_revision: u64,
    changes: SummarySnapshotChanges,
}

impl SummarySnapshot {
    fn page(&self, offset: usize, limit: usize) -> (ZoneSummaryChanges, Option<usize>) {
        match &self.changes {
            SummarySnapshotChanges::Reset { rows } => {
                let end = offset.saturating_add(limit).min(rows.len());
                let page = rows.get(offset..end).unwrap_or_default().to_vec();
                let next = (end < rows.len()).then_some(end);
                (ZoneSummaryChanges::Reset { rows: page }, next)
            }
            SummarySnapshotChanges::Delta {
                upserts,
                removed_zone_ids,
            } => {
                let total = upserts.len().saturating_add(removed_zone_ids.len());
                let end = offset.saturating_add(limit).min(total);
                let upsert_start = offset.min(upserts.len());
                let upsert_end = end.min(upserts.len());
                let removed_start = offset.saturating_sub(upserts.len());
                let removed_end = end.saturating_sub(upserts.len());
                let page_upserts = upserts
                    .get(upsert_start..upsert_end)
                    .unwrap_or_default()
                    .to_vec();
                let page_removed = removed_zone_ids
                    .get(removed_start..removed_end)
                    .unwrap_or_default()
                    .to_vec();
                let next = (end < total).then_some(end);
                (
                    ZoneSummaryChanges::Delta {
                        upserts: page_upserts,
                        removed_zone_ids: page_removed,
                    },
                    next,
                )
            }
        }
    }

    fn report(
        &self,
        changes: ZoneSummaryChanges,
        next_cursor: Option<String>,
    ) -> ZonesSummaryReport {
        ZonesSummaryReport {
            report_kind: "zones.summary",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: self.source_revision,
            network_scope: self.network_scope.clone(),
            catalog_revision: self.catalog_revision,
            source_config_epoch: self.source_config_epoch,
            observation_revision: self.observation_revision,
            summary_revision: self.summary_revision,
            changes,
            next_cursor,
        }
    }
}

#[derive(Debug, Clone)]
enum SummarySnapshotChanges {
    Reset {
        rows: Vec<ZoneSummary>,
    },
    Delta {
        upserts: Vec<ZoneSummary>,
        removed_zone_ids: Vec<String>,
    },
}

#[derive(Debug, Clone)]
struct SummaryCursor {
    snapshot: SummarySnapshot,
    offset: usize,
}

struct CursorPage {
    snapshot: SummarySnapshot,
    offset: usize,
}

fn summary_page_limit(limit: Option<u16>) -> Result<usize> {
    let limit = limit.map_or(DEFAULT_SUMMARY_PAGE_SIZE, usize::from);
    if limit == 0 || limit > MAX_SUMMARY_PAGE_SIZE {
        bail!("Zone summary page limit must be between 1 and {MAX_SUMMARY_PAGE_SIZE}");
    }
    Ok(limit)
}

fn validate_summary_request_fences(
    source_revision: u64,
    network_scope: Option<&NetworkScope>,
    snapshot: &SummarySnapshot,
) -> Result<()> {
    if source_revision != snapshot.source_revision
        || network_scope != snapshot.network_scope.as_ref()
    {
        bail!("Zone summary cursor belongs to another source or network");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};

    use super::*;

    fn key(source_revision: u64) -> ProjectionKey {
        ProjectionKey {
            source_revision,
            network_scope: None,
            catalog_revision: 7,
            source_config_epoch: 3,
            observation_revision: 5,
            verification: CatalogVerificationState::Verified,
        }
    }

    #[test]
    fn summary_delta_does_not_cross_projection_identity_floor() -> Result<()> {
        let projection_key = key(1);
        let mut ledger = ZoneProjectionLedger {
            projection_key: Some(projection_key.clone()),
            summary_revision: 2,
            delta_floor_revision: 1,
            ..ZoneProjectionLedger::default()
        };
        ledger.journal.push_back(SummaryJournalEntry {
            revision: 1,
            source_revision: 1,
            network_scope: None,
            upserts: Vec::new(),
            removed_zone_ids: vec!["channel-a".to_owned()],
        });
        ledger.journal.push_back(SummaryJournalEntry {
            revision: 2,
            source_revision: 1,
            network_scope: None,
            upserts: Vec::new(),
            removed_zone_ids: vec!["channel-b".to_owned()],
        });

        if !matches!(
            ledger.summary_snapshot(Some(0)).changes,
            SummarySnapshotChanges::Reset { .. }
        ) {
            bail!("delta crossed the projection identity floor");
        }
        let delta = ledger.summary_snapshot(Some(1));
        let SummarySnapshotChanges::Delta {
            upserts,
            removed_zone_ids,
        } = delta.changes
        else {
            bail!("in-range revision did not produce a delta");
        };
        if !upserts.is_empty() || removed_zone_ids != ["channel-b"] {
            bail!("unexpected coalesced delta: {removed_zone_ids:?}");
        }
        Ok(())
    }

    #[test]
    fn summary_cursor_keeps_immutable_snapshot_and_is_single_use() -> Result<()> {
        let mut ledger = ZoneProjectionLedger {
            projection_key: Some(key(1)),
            summary_revision: 9,
            ..ZoneProjectionLedger::default()
        };
        let cursor = ledger.insert_cursor(
            SummarySnapshot {
                source_revision: 1,
                network_scope: None,
                catalog_revision: 7,
                source_config_epoch: 3,
                observation_revision: 5,
                summary_revision: 7,
                changes: SummarySnapshotChanges::Delta {
                    upserts: Vec::new(),
                    removed_zone_ids: vec!["channel-a".to_owned(), "channel-b".to_owned()],
                },
            },
            1,
        )?;
        let request = ZonesSummaryRequest {
            source_revision: 1,
            network_scope: None,
            after_summary_revision: None,
            cursor: Some(cursor.clone()),
            limit: Some(1),
        };

        let report = ledger.summaries(request.clone())?;
        if report.summary_revision != 7 {
            bail!("cursor observed mutable ledger revision");
        }
        let ZoneSummaryChanges::Delta {
            upserts,
            removed_zone_ids,
        } = report.changes
        else {
            bail!("cursor did not preserve its delta page");
        };
        if !upserts.is_empty() || removed_zone_ids != ["channel-b"] {
            bail!("unexpected cursor page: {removed_zone_ids:?}");
        }
        let Err(error) = ledger.summaries(request) else {
            bail!("consumed cursor unexpectedly remained valid");
        };
        if !error.to_string().contains("invalid or expired") {
            bail!("unexpected consumed cursor error: {error:#}");
        }
        Ok(())
    }
}
