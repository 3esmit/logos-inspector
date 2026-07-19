use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
};

use anyhow::{Context as _, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tokio::{runtime::Runtime, sync::watch};

use crate::{
    inspection::{
        CatalogVerificationState, NetworkScope,
        catalog::{
            CatalogEvidencePayload, CatalogEvidencePayloadFormat, CatalogL1Block, CatalogL1Source,
            CatalogSnapshot, DirectCatalogL1Source, ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            ZoneCatalogService, ZoneCatalogServiceReport, ZoneCatalogSourceDescriptor,
            ZoneEvidenceDetailReport, ZoneEvidenceDetailRequest, ZoneEvidenceFilter,
            ZoneEvidenceFinality, ZoneEvidenceKind, ZoneEvidenceOperation, ZoneEvidencePageReport,
            ZoneEvidencePageRequest, ZoneEvidencePayloadChunkReport,
            ZoneEvidencePayloadChunkRequest, ZoneEvidencePayloadEncoding,
            ZoneEvidencePayloadReleaseReport, ZoneEvidencePayloadReleaseRequest,
            ZoneEvidencePayloadReport, ZoneEvidencePayloadWarning, ZoneEvidencePayloadWarningCode,
            ZoneEvidenceReference, ZoneEvidenceRow, ZoneEvidenceSegmentProvenance,
            ZoneEvidenceSourceKind, ZoneEvidenceSourceProvenance, extract_catalog_evidence_payload,
        },
    },
    source_routing::channel_sources::{
        FinalizedL1EvidenceBasis, SequencerAttestationBasis, SequencerLegacyAnchor,
        SequencerLegacyAnchorState,
    },
    support::{bridge_envelope::structured_bridge_error, time::now_millis},
};

const DEFAULT_EVIDENCE_PAGE_SIZE: usize = 25;
const MAX_EVIDENCE_PAGE_SIZE: usize = 100;
const MAX_EVIDENCE_CURSORS: usize = 64;
const EVIDENCE_INLINE_MAX_BYTES: usize = 256 * 1024;
const EVIDENCE_SESSION_MAX_BYTES: usize = 8 * 1024 * 1024;
const EVIDENCE_SESSIONS_MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_EVIDENCE_SESSIONS: usize = 4;
const EVIDENCE_SESSION_IDLE_MILLIS: u64 = 2 * 60 * 1_000;
const DEFAULT_EVIDENCE_CHUNK_BYTES: usize = 64 * 1024;
const MAX_EVIDENCE_CHUNK_BYTES: usize = 256 * 1024;
const EVIDENCE_PREVIEW_TEXT_BYTES: usize = 1_024;
const EVIDENCE_PREVIEW_BINARY_BYTES: usize = 256;

pub(super) type EvidenceBlockFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<CatalogL1Block>>> + Send + 'a>>;

pub(super) trait EvidenceBlockReader: Send + Sync {
    fn block<'a>(
        &'a self,
        source: ZoneCatalogSourceDescriptor,
        block_id: String,
    ) -> EvidenceBlockFuture<'a>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct DirectEvidenceBlockReader;

impl EvidenceBlockReader for DirectEvidenceBlockReader {
    fn block<'a>(
        &'a self,
        source: ZoneCatalogSourceDescriptor,
        block_id: String,
    ) -> EvidenceBlockFuture<'a> {
        Box::pin(async move {
            let source = DirectCatalogL1Source::for_evidence(source.endpoint())?;
            source.block(block_id).await.map_err(anyhow::Error::from)
        })
    }
}

#[derive(Clone)]
pub(super) struct ZoneEvidenceCommandInterface {
    reader: Arc<dyn EvidenceBlockReader>,
    state: Arc<Mutex<EvidenceState>>,
}

impl ZoneEvidenceCommandInterface {
    #[must_use]
    pub(super) fn new(reader: Arc<dyn EvidenceBlockReader>) -> Self {
        Self {
            reader,
            state: Arc::new(Mutex::new(EvidenceState::default())),
        }
    }

    pub(super) fn configure_source(
        &self,
        source_revision: u64,
        descriptor: ZoneCatalogSourceDescriptor,
    ) -> Result<()> {
        let mut state = self.lock_state()?;
        state.clear_transient();
        state.source = Some(EvidenceSourceBinding {
            source_revision,
            descriptor,
        });
        Ok(())
    }

    pub(super) fn rebind_source_revision(&self, source_revision: u64) -> Result<()> {
        let mut state = self.lock_state()?;
        state.clear_transient();
        let source = state
            .source
            .as_mut()
            .context("Zone evidence source is not configured")?;
        source.source_revision = source_revision;
        Ok(())
    }

    pub(super) fn reconcile(&self, report: &ZoneCatalogServiceReport) -> Result<()> {
        self.lock_state()?.reconcile(report);
        Ok(())
    }

    pub(super) fn page(
        &self,
        report: &ZoneCatalogServiceReport,
        request: ZoneEvidencePageRequest,
    ) -> Result<ZoneEvidencePageReport> {
        let limit = evidence_page_limit(request.limit)?;
        let mut state = self.lock_state()?;
        state.reconcile(report);
        require_verified_report(report)?;
        validate_report_identity(report, request.source_revision, &request.network_scope)?;

        let page = if let Some(cursor) = request.cursor.as_deref() {
            let cursor = state
                .cursors
                .remove(cursor)
                .context("Zone evidence cursor is invalid or expired")?;
            state
                .cursor_order
                .retain(|candidate| candidate != &cursor.token);
            validate_cursor_request(&request, &cursor.snapshot)?;
            EvidenceCursorPage {
                snapshot: cursor.snapshot,
                offset: cursor.offset,
            }
        } else {
            let catalog = report
                .catalog
                .as_deref()
                .context("verified Zone Catalog is unavailable")?;
            if request.catalog_revision != catalog.metadata.catalog_revision {
                return Err(stale_context_error(
                    "Zone evidence request belongs to a stale catalog revision",
                )?);
            }
            require_zone(catalog, &request.channel_id)?;
            let source = source_provenance(report)?;
            EvidenceCursorPage {
                snapshot: EvidencePageSnapshot {
                    source_revision: request.source_revision,
                    network_scope: request.network_scope.clone(),
                    catalog_revision: request.catalog_revision,
                    channel_id: request.channel_id.clone(),
                    filter: request.filter,
                    rows: evidence_rows(catalog, &request.channel_id, request.filter, source)?,
                },
                offset: 0,
            }
        };
        let end = page
            .offset
            .saturating_add(limit)
            .min(page.snapshot.rows.len());
        let rows = page
            .snapshot
            .rows
            .get(page.offset..end)
            .context("Zone evidence page bounds are invalid")?
            .to_vec();
        let next_cursor = if end < page.snapshot.rows.len() {
            Some(state.insert_cursor(page.snapshot.clone(), end)?)
        } else {
            None
        };
        Ok(ZoneEvidencePageReport {
            report_kind: "zones.evidence_page",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: page.snapshot.source_revision,
            network_scope: page.snapshot.network_scope,
            catalog_revision: page.snapshot.catalog_revision,
            channel_id: page.snapshot.channel_id,
            filter: page.snapshot.filter,
            rows,
            next_cursor,
        })
    }

    pub(super) fn detail(
        &self,
        runtime: &Runtime,
        service: &ZoneCatalogService,
        request: ZoneEvidenceDetailRequest,
    ) -> Result<ZoneEvidenceDetailReport> {
        let before = service.report();
        let plan = self.prepare_detail(&before, &request)?;
        let fetched = runtime.block_on(
            self.reader
                .block(plan.source.clone(), request.reference.block_id.clone()),
        );
        let block = match fetched {
            Ok(Some(block)) => block,
            Ok(None) | Err(_) => return Err(evidence_unavailable_error()?),
        };
        let after = service.report();
        self.finish_detail(&after, plan, block)
    }

    pub(super) fn sequencer_attestation_anchor(
        &self,
        service: &ZoneCatalogService,
        channel_id: &str,
    ) -> SequencerLegacyAnchorState {
        let interface = self.clone();
        let reports = service.report_receiver();
        let channel_id = channel_id.to_owned();
        SequencerLegacyAnchorState::deferred(move || {
            let interface = interface.clone();
            let reports = reports.clone();
            let channel_id = channel_id.clone();
            async move {
                interface
                    .load_sequencer_attestation_anchor(reports, channel_id)
                    .await
            }
        })
    }

    async fn load_sequencer_attestation_anchor(
        &self,
        reports: watch::Receiver<ZoneCatalogServiceReport>,
        channel_id: String,
    ) -> Result<SequencerLegacyAnchorState> {
        let before = reports.borrow().clone();
        require_verified_report(&before)?;
        let catalog = before
            .catalog
            .as_deref()
            .context("verified Zone Catalog is unavailable")?;
        let Some(reference) = catalog
            .evidence
            .iter()
            .filter(|reference| {
                reference.channel_id == channel_id
                    && reference.evidence_kind == ZoneEvidenceKind::SequencerBlock
            })
            .max_by(|left, right| {
                left.l1_slot
                    .cmp(&right.l1_slot)
                    .then_with(|| left.operation_index.cmp(&right.operation_index))
                    .then_with(|| left.evidence_id.cmp(&right.evidence_id))
            })
            .cloned()
        else {
            return Ok(SequencerLegacyAnchorState::Missing);
        };
        let request = ZoneEvidenceDetailRequest {
            source_revision: before.source_revision,
            network_scope: catalog.metadata.network_scope.clone(),
            catalog_revision: catalog.metadata.catalog_revision,
            channel_id: channel_id.clone(),
            reference,
        };
        let plan = self.prepare_detail(&before, &request)?;
        let fetched = self
            .reader
            .block(plan.source.clone(), request.reference.block_id.clone())
            .await;
        let block = match fetched {
            Ok(Some(block)) => block,
            Ok(None) | Err(_) => return Ok(SequencerLegacyAnchorState::Unavailable),
        };
        let after = reports.borrow().clone();
        let payload = validate_and_extract_payload(&after, &plan, &block)?;
        if payload.format != CatalogEvidencePayloadFormat::Bytes {
            return Err(evidence_unavailable_error()?);
        }
        let l2_block = match borsh::from_slice::<common::block::Block>(&payload.bytes) {
            Ok(block) => block,
            Err(_) => return Err(evidence_unavailable_error()?),
        };
        let transaction_hash = plan
            .row
            .reference
            .transaction_hash
            .clone()
            .context("Sequencer evidence has no transaction hash")?;
        let basis = SequencerAttestationBasis::UserTrustedFinalizedL1Evidence(Box::new(
            FinalizedL1EvidenceBasis {
                network_scope: plan.network_scope.clone(),
                catalog_source_fingerprint: plan.row.source.fingerprint.clone(),
                l1_slot: plan.row.reference.l1_slot,
                l1_block_id: plan.row.reference.block_id.clone(),
                transaction_hash,
                operation_index: plan.row.reference.operation_index,
                l2_block_id: l2_block.header.block_id,
                l2_header_hash: l2_block.header.hash.to_string(),
                l2_signature: l2_block.header.signature.to_string(),
            },
        ));

        let fence_plan = plan.clone();
        let fence_reports = reports.clone();
        let fence =
            Arc::new(move || evidence_plan_is_current(&fence_reports.borrow(), &fence_plan));
        Ok(SequencerLegacyAnchorState::Available(
            SequencerLegacyAnchor::new(channel_id, basis, payload.bytes, fence),
        ))
    }

    pub(super) fn payload_chunk(
        &self,
        report: &ZoneCatalogServiceReport,
        request: ZoneEvidencePayloadChunkRequest,
    ) -> Result<ZoneEvidencePayloadChunkReport> {
        let limit = evidence_chunk_limit(request.limit)?;
        let now = now_millis();
        let mut state = self.lock_state()?;
        state.reconcile(report);
        require_verified_report(report)?;
        validate_report_identity(report, request.source_revision, &request.network_scope)?;
        state.expire_sessions(now);

        let (encoding, bytes, next_offset, done) = {
            let session = state
                .sessions
                .get_mut(&request.session_id)
                .context("Zone evidence payload session is invalid or expired")?;
            validate_session_request(session, &request)?;
            let offset = usize::try_from(request.offset)
                .context("Zone evidence payload offset exceeds platform limits")?;
            if offset > session.bytes.len() {
                bail!("Zone evidence payload offset is beyond the payload");
            }
            let end = chunk_end(&session.bytes, session.encoding, offset, limit)?;
            session.last_access_millis = now;
            (
                session.encoding,
                session
                    .bytes
                    .get(offset..end)
                    .context("Zone evidence payload chunk bounds are invalid")?
                    .to_vec(),
                end,
                end == session.bytes.len(),
            )
        };
        state.touch_session(&request.session_id);
        let (text, base64) = match encoding {
            ZoneEvidencePayloadEncoding::Json | ZoneEvidencePayloadEncoding::Utf8 => (
                Some(
                    String::from_utf8(bytes)
                        .context("Zone evidence text payload session contains invalid UTF-8")?,
                ),
                None,
            ),
            ZoneEvidencePayloadEncoding::Binary => (None, Some(BASE64_STANDARD.encode(bytes))),
        };
        Ok(ZoneEvidencePayloadChunkReport {
            report_kind: "zones.evidence_payload_chunk",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            session_id: request.session_id,
            evidence_id: request.evidence_id,
            encoding,
            offset: request.offset,
            next_offset: u64::try_from(next_offset)
                .context("Zone evidence payload offset exceeds u64")?,
            done,
            text,
            base64,
        })
    }

    pub(super) fn release_payload(
        &self,
        report: &ZoneCatalogServiceReport,
        request: ZoneEvidencePayloadReleaseRequest,
    ) -> Result<ZoneEvidencePayloadReleaseReport> {
        let mut state = self.lock_state()?;
        state.reconcile(report);
        let released = state
            .sessions
            .get(&request.session_id)
            .is_some_and(|session| {
                session.source_revision == request.source_revision
                    && session.network_scope == request.network_scope
                    && session.channel_id == request.channel_id
                    && session.evidence_id == request.evidence_id
            })
            && state.remove_session(&request.session_id);
        Ok(ZoneEvidencePayloadReleaseReport {
            report_kind: "zones.evidence_payload_released",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            session_id: request.session_id,
            released,
        })
    }

    fn prepare_detail(
        &self,
        report: &ZoneCatalogServiceReport,
        request: &ZoneEvidenceDetailRequest,
    ) -> Result<EvidenceDetailPlan> {
        let mut state = self.lock_state()?;
        state.reconcile(report);
        require_verified_report(report)?;
        validate_report_identity(report, request.source_revision, &request.network_scope)?;
        let catalog = report
            .catalog
            .as_deref()
            .context("verified Zone Catalog is unavailable")?;
        if request.catalog_revision != catalog.metadata.catalog_revision {
            return Err(stale_context_error(
                "Zone evidence detail belongs to a stale catalog revision",
            )?);
        }
        if request.reference.channel_id != request.channel_id {
            return Err(stale_context_error(
                "Zone evidence reference belongs to another Channel",
            )?);
        }
        require_zone(catalog, &request.channel_id)?;
        let reference = catalog
            .evidence
            .iter()
            .find(|candidate| candidate.evidence_id == request.reference.evidence_id)
            .context("Zone evidence reference is not present in the current catalog")?;
        if reference != &request.reference {
            return Err(stale_context_error(
                "Zone evidence reference does not match current catalog membership",
            )?);
        }
        let row = evidence_row(catalog, reference, source_provenance(report)?)?;
        let source = state
            .source
            .as_ref()
            .filter(|source| source.source_revision == report.source_revision)
            .context("Zone evidence source is not configured")?
            .descriptor
            .clone();
        Ok(EvidenceDetailPlan {
            source,
            source_revision: request.source_revision,
            network_scope: request.network_scope.clone(),
            catalog_revision: request.catalog_revision,
            channel_id: request.channel_id.clone(),
            row,
        })
    }

    fn finish_detail(
        &self,
        report: &ZoneCatalogServiceReport,
        plan: EvidenceDetailPlan,
        block: CatalogL1Block,
    ) -> Result<ZoneEvidenceDetailReport> {
        let extracted = validate_and_extract_payload(report, &plan, &block)?;
        let opcode = extracted.opcode;
        let payload = self.payload_report(&plan, extracted)?;
        Ok(ZoneEvidenceDetailReport {
            report_kind: "zones.evidence_detail",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: plan.source_revision,
            network_scope: plan.network_scope,
            catalog_revision: plan.catalog_revision,
            channel_id: plan.channel_id,
            row: plan.row,
            operation: ZoneEvidenceOperation { opcode },
            payload,
        })
    }

    fn payload_report(
        &self,
        plan: &EvidenceDetailPlan,
        payload: CatalogEvidencePayload,
    ) -> Result<ZoneEvidencePayloadReport> {
        let byte_length = payload.bytes.len();
        let encoding = detect_payload_encoding(payload.format, &payload.bytes)?;
        let preview = payload_preview(encoding, &payload.bytes)?;
        let mut digest = Sha256::new();
        digest.update(&payload.bytes);
        let sha256 = format!("sha256:{}", hex::encode(digest.finalize()));
        let (inline_text, inline_base64) = if byte_length <= EVIDENCE_INLINE_MAX_BYTES {
            match encoding {
                ZoneEvidencePayloadEncoding::Json | ZoneEvidencePayloadEncoding::Utf8 => (
                    Some(
                        String::from_utf8(payload.bytes.clone())
                            .context("Zone evidence text payload contains invalid UTF-8")?,
                    ),
                    None,
                ),
                ZoneEvidencePayloadEncoding::Binary => {
                    (None, Some(BASE64_STANDARD.encode(&payload.bytes)))
                }
            }
        } else {
            (None, None)
        };
        let mut warning = None;
        let session_id = if byte_length > EVIDENCE_INLINE_MAX_BYTES
            && byte_length <= EVIDENCE_SESSION_MAX_BYTES
        {
            let session_id = random_token("evidence_payload")?;
            let session = PayloadSession {
                source_revision: plan.source_revision,
                network_scope: plan.network_scope.clone(),
                channel_id: plan.channel_id.clone(),
                evidence_id: plan.row.reference.evidence_id.clone(),
                encoding,
                bytes: payload.bytes,
                last_access_millis: now_millis(),
            };
            self.lock_state()?
                .insert_session(session_id.clone(), session);
            Some(session_id)
        } else {
            if byte_length > EVIDENCE_SESSION_MAX_BYTES {
                warning = Some(ZoneEvidencePayloadWarning {
                    code: ZoneEvidencePayloadWarningCode::EvidenceTooLarge,
                    message: "Evidence payload exceeds the bounded viewer session limit."
                        .to_owned(),
                });
            }
            None
        };
        Ok(ZoneEvidencePayloadReport {
            byte_length: u64::try_from(byte_length)
                .context("Zone evidence payload length exceeds u64")?,
            sha256,
            encoding,
            inline_text,
            inline_base64,
            preview: preview.text,
            preview_truncated: preview.truncated,
            inline_truncated: byte_length > EVIDENCE_INLINE_MAX_BYTES,
            session_id,
            warning,
        })
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, EvidenceState>> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Zone evidence state lock is poisoned"))
    }
}

#[derive(Default)]
struct EvidenceState {
    source: Option<EvidenceSourceBinding>,
    report_identity: Option<EvidenceReportIdentity>,
    cursors: BTreeMap<String, EvidenceCursor>,
    cursor_order: VecDeque<String>,
    sessions: BTreeMap<String, PayloadSession>,
    session_order: VecDeque<String>,
    session_bytes: usize,
}

impl EvidenceState {
    fn reconcile(&mut self, report: &ZoneCatalogServiceReport) {
        let identity = report
            .catalog
            .as_deref()
            .map(|catalog| EvidenceReportIdentity {
                source_revision: report.source_revision,
                network_scope: catalog.metadata.network_scope.clone(),
            });
        let identity_changed = self
            .report_identity
            .as_ref()
            .is_some_and(|current| identity.as_ref() != Some(current));
        if report.verification_state != CatalogVerificationState::Verified || identity_changed {
            self.clear_transient();
        }
        self.report_identity = identity;
    }

    fn clear_transient(&mut self) {
        self.cursors.clear();
        self.cursor_order.clear();
        self.sessions.clear();
        self.session_order.clear();
        self.session_bytes = 0;
    }

    fn insert_cursor(&mut self, snapshot: EvidencePageSnapshot, offset: usize) -> Result<String> {
        let token = random_token("evidence_cursor")?;
        while self.cursor_order.len() >= MAX_EVIDENCE_CURSORS {
            if let Some(expired) = self.cursor_order.pop_front() {
                self.cursors.remove(&expired);
            }
        }
        self.cursor_order.push_back(token.clone());
        self.cursors.insert(
            token.clone(),
            EvidenceCursor {
                token: token.clone(),
                snapshot,
                offset,
            },
        );
        Ok(token)
    }

    fn insert_session(&mut self, session_id: String, session: PayloadSession) {
        self.expire_sessions(session.last_access_millis);
        while self.sessions.len() >= MAX_EVIDENCE_SESSIONS
            || self.session_bytes.saturating_add(session.bytes.len()) > EVIDENCE_SESSIONS_MAX_BYTES
        {
            let Some(oldest) = self.session_order.front().cloned() else {
                break;
            };
            self.remove_session(&oldest);
        }
        self.session_bytes = self.session_bytes.saturating_add(session.bytes.len());
        self.session_order.push_back(session_id.clone());
        self.sessions.insert(session_id, session);
    }

    fn touch_session(&mut self, session_id: &str) {
        self.session_order
            .retain(|candidate| candidate != session_id);
        self.session_order.push_back(session_id.to_owned());
    }

    fn remove_session(&mut self, session_id: &str) -> bool {
        let Some(session) = self.sessions.remove(session_id) else {
            return false;
        };
        self.session_bytes = self.session_bytes.saturating_sub(session.bytes.len());
        self.session_order
            .retain(|candidate| candidate != session_id);
        true
    }

    fn expire_sessions(&mut self, now_millis: u64) {
        let expired = self
            .sessions
            .iter()
            .filter(|(_, session)| {
                now_millis.saturating_sub(session.last_access_millis)
                    >= EVIDENCE_SESSION_IDLE_MILLIS
            })
            .map(|(session_id, _)| session_id.clone())
            .collect::<Vec<_>>();
        for session_id in expired {
            self.remove_session(&session_id);
        }
    }
}

#[derive(Debug, Clone)]
struct EvidenceSourceBinding {
    source_revision: u64,
    descriptor: ZoneCatalogSourceDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceReportIdentity {
    source_revision: u64,
    network_scope: NetworkScope,
}

#[derive(Debug, Clone)]
struct EvidenceCursor {
    token: String,
    snapshot: EvidencePageSnapshot,
    offset: usize,
}

#[derive(Debug, Clone)]
struct EvidenceCursorPage {
    snapshot: EvidencePageSnapshot,
    offset: usize,
}

#[derive(Debug, Clone)]
struct EvidencePageSnapshot {
    source_revision: u64,
    network_scope: NetworkScope,
    catalog_revision: u64,
    channel_id: String,
    filter: ZoneEvidenceFilter,
    rows: Vec<ZoneEvidenceRow>,
}

#[derive(Debug, Clone)]
struct EvidenceDetailPlan {
    source: ZoneCatalogSourceDescriptor,
    source_revision: u64,
    network_scope: NetworkScope,
    catalog_revision: u64,
    channel_id: String,
    row: ZoneEvidenceRow,
}

struct PayloadSession {
    source_revision: u64,
    network_scope: NetworkScope,
    channel_id: String,
    evidence_id: String,
    encoding: ZoneEvidencePayloadEncoding,
    bytes: Vec<u8>,
    last_access_millis: u64,
}

struct PayloadPreview {
    text: String,
    truncated: bool,
}

fn validate_and_extract_payload(
    report: &ZoneCatalogServiceReport,
    plan: &EvidenceDetailPlan,
    block: &CatalogL1Block,
) -> Result<CatalogEvidencePayload> {
    require_verified_report(report)?;
    validate_report_identity(report, plan.source_revision, &plan.network_scope)?;
    let catalog = report
        .catalog
        .as_deref()
        .context("verified Zone Catalog is unavailable")?;
    if catalog.metadata.catalog_revision != plan.catalog_revision
        || !catalog
            .evidence
            .iter()
            .any(|reference| reference == &plan.row.reference)
    {
        return Err(stale_context_error(
            "Zone evidence changed while detail was loading",
        )?);
    }
    match extract_catalog_evidence_payload(block, &plan.row.reference) {
        Ok(extracted) => Ok(extracted),
        Err(_) => Err(evidence_unavailable_error()?),
    }
}

fn evidence_plan_is_current(report: &ZoneCatalogServiceReport, plan: &EvidenceDetailPlan) -> bool {
    if report.verification_state != CatalogVerificationState::Verified
        || report.source_revision != plan.source_revision
        || report.source_fingerprint.as_deref() != Some(plan.row.source.fingerprint.as_str())
    {
        return false;
    }
    let Some(catalog) = report.catalog.as_deref() else {
        return false;
    };
    catalog.metadata.network_scope == plan.network_scope
        && catalog.metadata.catalog_revision == plan.catalog_revision
        && catalog
            .evidence
            .iter()
            .any(|reference| reference == &plan.row.reference)
}

fn evidence_rows(
    catalog: &CatalogSnapshot,
    channel_id: &str,
    filter: ZoneEvidenceFilter,
    source: ZoneEvidenceSourceProvenance,
) -> Result<Vec<ZoneEvidenceRow>> {
    let mut rows = catalog
        .evidence
        .iter()
        .filter(|reference| {
            reference.channel_id == channel_id && evidence_matches_filter(reference, filter)
        })
        .map(|reference| evidence_row(catalog, reference, source.clone()))
        .collect::<Result<Vec<_>>>()?;
    rows.sort_by(|left, right| {
        right
            .reference
            .l1_slot
            .cmp(&left.reference.l1_slot)
            .then_with(|| {
                right
                    .reference
                    .operation_index
                    .cmp(&left.reference.operation_index)
            })
            .then_with(|| right.reference.evidence_id.cmp(&left.reference.evidence_id))
    });
    Ok(rows)
}

fn evidence_row(
    catalog: &CatalogSnapshot,
    reference: &ZoneEvidenceReference,
    source: ZoneEvidenceSourceProvenance,
) -> Result<ZoneEvidenceRow> {
    let segment = catalog
        .segments
        .iter()
        .find(|segment| segment.segment_id == reference.coverage_segment_id)
        .context("Zone evidence references a missing coverage segment")?;
    Ok(ZoneEvidenceRow {
        reference: reference.clone(),
        segment: ZoneEvidenceSegmentProvenance::from(segment),
        source,
        finality: ZoneEvidenceFinality::Final,
    })
}

fn evidence_matches_filter(reference: &ZoneEvidenceReference, filter: ZoneEvidenceFilter) -> bool {
    match filter {
        ZoneEvidenceFilter::All => true,
        ZoneEvidenceFilter::ChannelConfiguration => {
            reference.evidence_kind == ZoneEvidenceKind::ChannelConfiguration
        }
        ZoneEvidenceFilter::ChannelOperation => {
            reference.evidence_kind == ZoneEvidenceKind::ChannelOperation
        }
        ZoneEvidenceFilter::RawInscription => {
            reference.evidence_kind == ZoneEvidenceKind::RawInscription
        }
    }
}

fn source_provenance(report: &ZoneCatalogServiceReport) -> Result<ZoneEvidenceSourceProvenance> {
    Ok(ZoneEvidenceSourceProvenance {
        kind: ZoneEvidenceSourceKind::DirectHttp,
        fingerprint: report
            .source_fingerprint
            .clone()
            .context("Zone evidence source fingerprint is unavailable")?,
    })
}

fn require_verified_report(report: &ZoneCatalogServiceReport) -> Result<()> {
    if report.verification_state != CatalogVerificationState::Verified {
        return Err(stale_context_error("Zone Catalog is not verified")?);
    }
    Ok(())
}

fn validate_report_identity(
    report: &ZoneCatalogServiceReport,
    source_revision: u64,
    network_scope: &NetworkScope,
) -> Result<()> {
    let catalog = report
        .catalog
        .as_deref()
        .context("verified Zone Catalog is unavailable")?;
    if report.source_revision != source_revision || &catalog.metadata.network_scope != network_scope
    {
        return Err(stale_context_error(
            "Zone evidence request belongs to stale source context",
        )?);
    }
    Ok(())
}

fn validate_cursor_request(
    request: &ZoneEvidencePageRequest,
    snapshot: &EvidencePageSnapshot,
) -> Result<()> {
    if request.source_revision != snapshot.source_revision
        || request.network_scope != snapshot.network_scope
        || request.catalog_revision != snapshot.catalog_revision
        || request.channel_id != snapshot.channel_id
        || request.filter != snapshot.filter
    {
        return Err(stale_context_error(
            "Zone evidence cursor does not match the requested snapshot",
        )?);
    }
    Ok(())
}

fn require_zone(catalog: &CatalogSnapshot, channel_id: &str) -> Result<()> {
    if !catalog
        .zones
        .iter()
        .any(|zone| zone.channel_id == channel_id)
    {
        bail!("Zone does not exist in the current catalog");
    }
    Ok(())
}

fn validate_session_request(
    session: &PayloadSession,
    request: &ZoneEvidencePayloadChunkRequest,
) -> Result<()> {
    if session.source_revision != request.source_revision
        || session.network_scope != request.network_scope
        || session.channel_id != request.channel_id
        || session.evidence_id != request.evidence_id
    {
        return Err(stale_context_error(
            "Zone evidence payload session belongs to another context",
        )?);
    }
    Ok(())
}

fn evidence_page_limit(limit: Option<u16>) -> Result<usize> {
    let limit = usize::from(limit.unwrap_or(DEFAULT_EVIDENCE_PAGE_SIZE as u16));
    if limit == 0 || limit > MAX_EVIDENCE_PAGE_SIZE {
        bail!("Zone evidence page limit must be between 1 and {MAX_EVIDENCE_PAGE_SIZE}");
    }
    Ok(limit)
}

fn evidence_chunk_limit(limit: Option<u32>) -> Result<usize> {
    let limit = usize::try_from(limit.unwrap_or(DEFAULT_EVIDENCE_CHUNK_BYTES as u32))
        .context("Zone evidence payload chunk limit exceeds platform limits")?;
    if !(4..=MAX_EVIDENCE_CHUNK_BYTES).contains(&limit) {
        bail!("Zone evidence payload chunk limit must be between 4 and {MAX_EVIDENCE_CHUNK_BYTES}");
    }
    Ok(limit)
}

fn detect_payload_encoding(
    format: CatalogEvidencePayloadFormat,
    bytes: &[u8],
) -> Result<ZoneEvidencePayloadEncoding> {
    if format == CatalogEvidencePayloadFormat::Json {
        serde_json::from_slice::<Value>(bytes)
            .context("catalog JSON evidence payload is invalid")?;
        return Ok(ZoneEvidencePayloadEncoding::Json);
    }
    if serde_json::from_slice::<Value>(bytes).is_ok() {
        return Ok(ZoneEvidencePayloadEncoding::Json);
    }
    if std::str::from_utf8(bytes).is_ok() {
        return Ok(ZoneEvidencePayloadEncoding::Utf8);
    }
    Ok(ZoneEvidencePayloadEncoding::Binary)
}

fn payload_preview(encoding: ZoneEvidencePayloadEncoding, bytes: &[u8]) -> Result<PayloadPreview> {
    Ok(match encoding {
        ZoneEvidencePayloadEncoding::Json | ZoneEvidencePayloadEncoding::Utf8 => {
            let text = std::str::from_utf8(bytes)
                .context("Zone evidence text preview contains invalid UTF-8")?;
            let mut end = text.len().min(EVIDENCE_PREVIEW_TEXT_BYTES);
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            PayloadPreview {
                text: text
                    .get(..end)
                    .context("Zone evidence text preview bounds are invalid")?
                    .to_owned(),
                truncated: end < text.len(),
            }
        }
        ZoneEvidencePayloadEncoding::Binary => {
            let end = bytes.len().min(EVIDENCE_PREVIEW_BINARY_BYTES);
            PayloadPreview {
                text: hex::encode(
                    bytes
                        .get(..end)
                        .context("Zone evidence binary preview bounds are invalid")?,
                ),
                truncated: end < bytes.len(),
            }
        }
    })
}

fn chunk_end(
    bytes: &[u8],
    encoding: ZoneEvidencePayloadEncoding,
    offset: usize,
    limit: usize,
) -> Result<usize> {
    let tentative = offset.saturating_add(limit).min(bytes.len());
    if encoding == ZoneEvidencePayloadEncoding::Binary {
        return Ok(tentative);
    }
    let text = std::str::from_utf8(bytes).context("Zone evidence text payload is invalid")?;
    if !text.is_char_boundary(offset) {
        bail!("Zone evidence payload offset is not a text boundary");
    }
    let mut end = tentative;
    while end > offset && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end == offset && end < text.len() {
        end += 1;
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }
    }
    Ok(end)
}

fn random_token(prefix: &str) -> Result<String> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).context("failed to generate Zone evidence token")?;
    Ok(format!("{prefix}_{}", hex::encode(random)))
}

fn stale_context_error(message: &str) -> Result<anyhow::Error> {
    structured_bridge_error(
        message,
        json!({
            "code": "stale_context",
            "recovery": "refresh_zone",
        }),
    )
}

fn evidence_unavailable_error() -> Result<anyhow::Error> {
    structured_bridge_error(
        "Cataloged L1 evidence is unavailable from the current source",
        json!({
            "code": "evidence_unavailable",
            "recovery": "retry",
        }),
    )
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, ensure};

    use super::*;
    use crate::inspection::{
        L1ChannelSummary, L1FinalityState,
        catalog::{
            CatalogBlockCheckpoint, CatalogBlockReference, CatalogEvidenceUse, CatalogMetadata,
            CatalogSnapshot, CatalogSnapshotOrigin, CoverageSegment, ZoneCatalogRecord,
            ZoneClassificationCounters,
        },
    };

    #[test]
    fn evidence_rows_filter_and_order_without_payloads() -> Result<()> {
        let catalog = evidence_catalog();
        let source = ZoneEvidenceSourceProvenance {
            kind: ZoneEvidenceSourceKind::DirectHttp,
            fingerprint: "sha256:test".to_owned(),
        };
        let rows = evidence_rows(
            &catalog,
            &identity('a'),
            ZoneEvidenceFilter::All,
            source.clone(),
        )?;
        ensure!(
            rows.len() == 3
                && rows.first().is_some_and(|row| row.reference.l1_slot == 12)
                && rows
                    .get(1)
                    .is_some_and(|row| row.reference.operation_index == 2)
                && rows
                    .iter()
                    .all(|row| row.segment.segment_id == "segment-main"),
            "evidence rows are not deterministically ordered: {rows:?}"
        );
        let raw = evidence_rows(
            &catalog,
            &identity('a'),
            ZoneEvidenceFilter::RawInscription,
            source,
        )?;
        ensure!(
            raw.len() == 1
                && raw.first().is_some_and(|row| {
                    row.reference.evidence_kind == ZoneEvidenceKind::RawInscription
                }),
            "raw evidence filter leaked other kinds: {raw:?}"
        );
        for (filter, expected_kind) in [
            (
                ZoneEvidenceFilter::ChannelConfiguration,
                ZoneEvidenceKind::ChannelConfiguration,
            ),
            (
                ZoneEvidenceFilter::ChannelOperation,
                ZoneEvidenceKind::ChannelOperation,
            ),
        ] {
            let filtered = evidence_rows(
                &catalog,
                &identity('a'),
                filter,
                ZoneEvidenceSourceProvenance {
                    kind: ZoneEvidenceSourceKind::DirectHttp,
                    fingerprint: "sha256:test".to_owned(),
                },
            )?;
            ensure!(
                filtered.len() == 1
                    && filtered
                        .first()
                        .is_some_and(|row| row.reference.evidence_kind == expected_kind),
                "evidence filter {filter:?} leaked other kinds: {filtered:?}"
            );
        }
        let serialized = serde_json::to_value(&rows)?;
        ensure!(
            serialized.to_string().find("payload").is_none(),
            "evidence rows embedded payload data: {serialized}"
        );
        Ok(())
    }

    #[test]
    fn evidence_cursor_keeps_immutable_catalog_snapshot() -> Result<()> {
        let interface = ZoneEvidenceCommandInterface::new(Arc::new(NoopReader));
        interface.configure_source(
            4,
            ZoneCatalogSourceDescriptor::direct_http("https://l1.example")?,
        )?;
        let catalog = evidence_catalog();
        let report = verified_report(catalog.clone());
        let request = ZoneEvidencePageRequest {
            source_revision: 4,
            network_scope: scope('1'),
            catalog_revision: 9,
            channel_id: identity('a'),
            filter: ZoneEvidenceFilter::All,
            cursor: None,
            limit: Some(1),
        };
        let first = interface.page(&report, request.clone())?;
        let cursor = first
            .next_cursor
            .context("first evidence page has no cursor")?;

        let mut newer_catalog = catalog;
        newer_catalog.metadata.catalog_revision = 10;
        newer_catalog.evidence.clear();
        let mut cursor_request = request;
        cursor_request.cursor = Some(cursor);
        let second = interface.page(&verified_report(newer_catalog), cursor_request)?;
        ensure!(
            second.catalog_revision == 9
                && second.rows.len() == 1
                && second
                    .rows
                    .first()
                    .is_some_and(|row| row.reference.evidence_id == "evidence-operation"),
            "new publication mutated immutable evidence cursor: {second:?}"
        );
        Ok(())
    }

    #[test]
    fn unavailable_evidence_uses_typed_bridge_error() -> Result<()> {
        let response = crate::support::bridge_envelope::bridge_response_json(Err(
            evidence_unavailable_error()?,
        ));
        let value: Value = serde_json::from_str(&response)?;
        ensure!(
            value.pointer("/error_details/code").and_then(Value::as_str)
                == Some("evidence_unavailable")
                && value
                    .pointer("/error_details/recovery")
                    .and_then(Value::as_str)
                    == Some("retry"),
            "unavailable evidence error is not typed: {value}"
        );
        Ok(())
    }

    #[test]
    fn payload_sessions_are_bounded_chunked_and_released() -> Result<()> {
        let mut state = EvidenceState::default();
        let session_id = "evidence_payload_test".to_owned();
        state.insert_session(
            session_id.clone(),
            PayloadSession {
                source_revision: 4,
                network_scope: scope('1'),
                channel_id: identity('a'),
                evidence_id: "evidence-test".to_owned(),
                encoding: ZoneEvidencePayloadEncoding::Utf8,
                bytes: "a€b".repeat(100_000).into_bytes(),
                last_access_millis: 10,
            },
        );
        let session = state
            .sessions
            .get(&session_id)
            .context("payload session was not retained")?;
        let end = chunk_end(&session.bytes, session.encoding, 0, 65_537)?;
        ensure!(
            session
                .bytes
                .get(..end)
                .is_some_and(|chunk| std::str::from_utf8(chunk).is_ok())
                && end <= 65_540,
            "text chunk split a UTF-8 code point"
        );
        ensure!(
            state.remove_session(&session_id),
            "payload session was not released"
        );
        ensure!(
            state.session_bytes == 0,
            "released payload bytes remain charged"
        );
        Ok(())
    }

    #[test]
    fn payload_encoding_preview_and_hash_preserve_raw_bytes() -> Result<()> {
        ensure!(
            detect_payload_encoding(CatalogEvidencePayloadFormat::Bytes, br#"{"ok":true}"#)?
                == ZoneEvidencePayloadEncoding::Json,
            "JSON bytes were not detected"
        );
        ensure!(
            detect_payload_encoding(CatalogEvidencePayloadFormat::Bytes, b"plain text")?
                == ZoneEvidencePayloadEncoding::Utf8,
            "UTF-8 bytes were not detected"
        );
        ensure!(
            detect_payload_encoding(CatalogEvidencePayloadFormat::Bytes, &[0xff, 0x00, 0x7f])?
                == ZoneEvidencePayloadEncoding::Binary,
            "binary bytes were not detected"
        );
        ensure!(
            detect_payload_encoding(CatalogEvidencePayloadFormat::Json, b"not JSON").is_err(),
            "declared JSON accepted invalid bytes"
        );

        let interface = ZoneEvidenceCommandInterface::new(Arc::new(NoopReader));
        let report = interface.payload_report(
            &payload_plan()?,
            CatalogEvidencePayload {
                opcode: 0x11,
                bytes: vec![0xff, 0x00, 0x7f],
                format: CatalogEvidencePayloadFormat::Bytes,
            },
        )?;
        ensure!(
            report.encoding == ZoneEvidencePayloadEncoding::Binary
                && report.inline_text.is_none()
                && report.inline_base64.as_deref() == Some("/wB/")
                && report.preview == "ff007f"
                && report.sha256
                    == "sha256:36ef98b33b9466c6d1e56326f9df94a7e723676d2ad03e8ff2b632e58234f9ca",
            "binary payload metadata changed raw bytes: {report:?}"
        );

        let text = payload_preview(
            ZoneEvidencePayloadEncoding::Utf8,
            "€".repeat(400).as_bytes(),
        )?;
        ensure!(
            text.truncated && text.text.len() <= EVIDENCE_PREVIEW_TEXT_BYTES,
            "UTF-8 preview was not safely bounded"
        );
        Ok(())
    }

    #[test]
    fn payload_chunks_keep_offsets_and_transport_encoding_separate() -> Result<()> {
        let interface = ZoneEvidenceCommandInterface::new(Arc::new(NoopReader));
        let now = now_millis();
        {
            let mut state = interface.lock_state()?;
            state.insert_session(
                "text-session".to_owned(),
                payload_session("text-evidence", "a€b".as_bytes().to_vec(), now),
            );
            let mut binary = payload_session("binary-evidence", vec![0xff, 0x00, 0x7f, 0x01], now);
            binary.encoding = ZoneEvidencePayloadEncoding::Binary;
            state.insert_session("binary-session".to_owned(), binary);
        }
        let report = verified_report(evidence_catalog());
        let first = interface.payload_chunk(
            &report,
            ZoneEvidencePayloadChunkRequest {
                source_revision: 4,
                network_scope: scope('1'),
                channel_id: identity('a'),
                evidence_id: "text-evidence".to_owned(),
                session_id: "text-session".to_owned(),
                offset: 0,
                limit: Some(4),
            },
        )?;
        ensure!(
            first.text.as_deref() == Some("a€")
                && first.base64.is_none()
                && first.next_offset == 4
                && !first.done,
            "first text chunk split or cross-encoded payload: {first:?}"
        );
        let second = interface.payload_chunk(
            &report,
            ZoneEvidencePayloadChunkRequest {
                source_revision: 4,
                network_scope: scope('1'),
                channel_id: identity('a'),
                evidence_id: "text-evidence".to_owned(),
                session_id: "text-session".to_owned(),
                offset: first.next_offset,
                limit: Some(4),
            },
        )?;
        ensure!(
            second.text.as_deref() == Some("b")
                && second.base64.is_none()
                && second.next_offset == 5
                && second.done,
            "second text chunk has invalid offset or shape: {second:?}"
        );
        let binary = interface.payload_chunk(
            &report,
            ZoneEvidencePayloadChunkRequest {
                source_revision: 4,
                network_scope: scope('1'),
                channel_id: identity('a'),
                evidence_id: "binary-evidence".to_owned(),
                session_id: "binary-session".to_owned(),
                offset: 0,
                limit: Some(4),
            },
        )?;
        ensure!(
            binary.text.is_none()
                && binary.base64.as_deref() == Some("/wB/AQ==")
                && binary.next_offset == 4
                && binary.done,
            "binary chunk was not base64-only: {binary:?}"
        );
        Ok(())
    }

    #[test]
    fn payload_session_limits_use_lru_expiry_and_context_invalidation() -> Result<()> {
        let now = now_millis();
        let mut count_state = EvidenceState::default();
        for index in 0..MAX_EVIDENCE_SESSIONS {
            let session_id = format!("count-{index}");
            count_state.insert_session(
                session_id.clone(),
                payload_session(&session_id, vec![u8::try_from(index)?], now),
            );
        }
        count_state.touch_session("count-0");
        count_state.insert_session(
            "count-next".to_owned(),
            payload_session("count-next", vec![9], now),
        );
        ensure!(
            count_state.sessions.len() == MAX_EVIDENCE_SESSIONS
                && count_state.sessions.contains_key("count-0")
                && !count_state.sessions.contains_key("count-1")
                && count_state.sessions.contains_key("count-next"),
            "session count limit did not evict least-recently-used entry"
        );

        let mut byte_state = EvidenceState::default();
        byte_state.insert_session(
            "bytes-a".to_owned(),
            payload_session("bytes-a", vec![1; EVIDENCE_SESSION_MAX_BYTES], now),
        );
        byte_state.insert_session(
            "bytes-b".to_owned(),
            payload_session("bytes-b", vec![2; EVIDENCE_SESSION_MAX_BYTES], now),
        );
        byte_state.touch_session("bytes-a");
        byte_state.insert_session(
            "bytes-next".to_owned(),
            payload_session("bytes-next", vec![3], now),
        );
        ensure!(
            byte_state.session_bytes == EVIDENCE_SESSION_MAX_BYTES + 1
                && byte_state.sessions.contains_key("bytes-a")
                && !byte_state.sessions.contains_key("bytes-b")
                && byte_state.sessions.contains_key("bytes-next"),
            "session byte limit did not use LRU eviction"
        );

        let mut expiry_state = EvidenceState::default();
        expiry_state.insert_session("expired".to_owned(), payload_session("expired", vec![1], 1));
        expiry_state.expire_sessions(1 + EVIDENCE_SESSION_IDLE_MILLIS);
        ensure!(
            expiry_state.sessions.is_empty() && expiry_state.session_bytes == 0,
            "idle payload session did not expire"
        );

        let mut context_state = EvidenceState {
            report_identity: Some(EvidenceReportIdentity {
                source_revision: 4,
                network_scope: scope('1'),
            }),
            ..EvidenceState::default()
        };
        context_state.insert_session(
            "context".to_owned(),
            payload_session("context", vec![1], now),
        );
        context_state.insert_cursor(
            EvidencePageSnapshot {
                source_revision: 4,
                network_scope: scope('1'),
                catalog_revision: 9,
                channel_id: identity('a'),
                filter: ZoneEvidenceFilter::All,
                rows: Vec::new(),
            },
            0,
        )?;
        let mut changed_report = verified_report(evidence_catalog());
        changed_report.source_revision = 5;
        context_state.reconcile(&changed_report);
        ensure!(
            context_state.sessions.is_empty()
                && context_state.cursors.is_empty()
                && context_state.session_bytes == 0,
            "source identity change retained transient evidence state"
        );
        context_state.insert_session(
            "quarantine".to_owned(),
            payload_session("quarantine", vec![1], now),
        );
        changed_report.verification_state = CatalogVerificationState::Mismatch;
        context_state.reconcile(&changed_report);
        ensure!(
            context_state.sessions.is_empty(),
            "catalog quarantine retained payload session"
        );
        Ok(())
    }

    #[test]
    fn payload_report_enforces_inline_session_and_oversize_boundaries() -> Result<()> {
        let interface = ZoneEvidenceCommandInterface::new(Arc::new(NoopReader));
        let plan = payload_plan()?;

        let inline = interface.payload_report(
            &plan,
            CatalogEvidencePayload {
                opcode: 0x11,
                bytes: vec![b'a'; EVIDENCE_INLINE_MAX_BYTES],
                format: CatalogEvidencePayloadFormat::Bytes,
            },
        )?;
        ensure!(
            inline.inline_text.is_some() && inline.session_id.is_none() && !inline.inline_truncated,
            "inline boundary was not retained inline"
        );

        let session = interface.payload_report(
            &plan,
            CatalogEvidencePayload {
                opcode: 0x11,
                bytes: vec![b'b'; EVIDENCE_INLINE_MAX_BYTES + 1],
                format: CatalogEvidencePayloadFormat::Bytes,
            },
        )?;
        ensure!(
            session.inline_text.is_none()
                && session.session_id.is_some()
                && session.inline_truncated,
            "session boundary did not create a bounded payload session"
        );

        let maximum_session = interface.payload_report(
            &plan,
            CatalogEvidencePayload {
                opcode: 0x11,
                bytes: vec![b'c'; EVIDENCE_SESSION_MAX_BYTES],
                format: CatalogEvidencePayloadFormat::Bytes,
            },
        )?;
        ensure!(
            maximum_session.session_id.is_some()
                && maximum_session.warning.is_none()
                && maximum_session.inline_text.is_none(),
            "maximum session payload was rejected"
        );

        let oversized = interface.payload_report(
            &plan,
            CatalogEvidencePayload {
                opcode: 0x11,
                bytes: vec![0xff; EVIDENCE_SESSION_MAX_BYTES + 1],
                format: CatalogEvidencePayloadFormat::Bytes,
            },
        )?;
        ensure!(
            oversized.session_id.is_none()
                && oversized.inline_base64.is_none()
                && oversized.warning.as_ref().is_some_and(|warning| {
                    warning.code == ZoneEvidencePayloadWarningCode::EvidenceTooLarge
                }),
            "oversized evidence did not return metadata-only warning"
        );
        Ok(())
    }

    struct NoopReader;

    impl EvidenceBlockReader for NoopReader {
        fn block<'a>(
            &'a self,
            _source: ZoneCatalogSourceDescriptor,
            _block_id: String,
        ) -> EvidenceBlockFuture<'a> {
            Box::pin(async { Ok(None) })
        }
    }

    fn payload_plan() -> Result<EvidenceDetailPlan> {
        let row = evidence_rows(
            &evidence_catalog(),
            &identity('a'),
            ZoneEvidenceFilter::RawInscription,
            ZoneEvidenceSourceProvenance {
                kind: ZoneEvidenceSourceKind::DirectHttp,
                fingerprint: "sha256:test".to_owned(),
            },
        )?
        .pop()
        .context("raw evidence row is missing")?;
        Ok(EvidenceDetailPlan {
            source: ZoneCatalogSourceDescriptor::direct_http("https://l1.example")?,
            source_revision: 4,
            network_scope: scope('1'),
            catalog_revision: 9,
            channel_id: identity('a'),
            row,
        })
    }

    fn payload_session(
        evidence_id: &str,
        bytes: Vec<u8>,
        last_access_millis: u64,
    ) -> PayloadSession {
        PayloadSession {
            source_revision: 4,
            network_scope: scope('1'),
            channel_id: identity('a'),
            evidence_id: evidence_id.to_owned(),
            encoding: ZoneEvidencePayloadEncoding::Utf8,
            bytes,
            last_access_millis,
        }
    }

    fn evidence_catalog() -> CatalogSnapshot {
        let channel_id = identity('a');
        let segment = CoverageSegment {
            segment_id: "segment-main".to_owned(),
            floor: CatalogBlockCheckpoint {
                slot: 0,
                block_id: identity('0'),
                parent_id: identity('f'),
            },
            frontier: CatalogBlockReference {
                slot: 12,
                block_id: identity('c'),
            },
            reaches_target_lib: true,
        };
        CatalogSnapshot {
            metadata: CatalogMetadata {
                catalog_file_id: "catalog_test".to_owned(),
                network_scope: scope('1'),
                identity_aliases: Vec::new(),
                identity_assurance:
                    crate::inspection::catalog::CatalogIdentityAssurance::SourceAttested,
                identity_transition: None,
                catalog_revision: 9,
                created_at_unix: 1,
                updated_at_unix: 2,
            },
            frontier: None,
            traversal: None,
            zones: vec![ZoneCatalogRecord {
                channel_id: channel_id.clone(),
                observed_label: Some("Test Zone".to_owned()),
                l1_channel: L1ChannelSummary {
                    tip_slot: Some(12),
                    tip_hash: Some(identity('c')),
                    lib_slot: Some(12),
                    balance: None,
                    key_count: Some(1),
                    withdraw_threshold: Some("1".to_owned()),
                    operation_count: 3,
                    finality_state: L1FinalityState::Final,
                },
                sequencer_committee: None,
                classification: ZoneClassificationCounters {
                    channel_operations: 2,
                    recognized_l2_blocks: 0,
                    raw_inscriptions: 1,
                    conflicting_evidence: false,
                },
                first_seen_slot: 10,
                last_seen_slot: 12,
                latest_evidence_id: "evidence-raw".to_owned(),
                evidence_count: 3,
                snapshot_provenance: crate::inspection::catalog::CatalogSnapshotProvenance {
                    origin: CatalogSnapshotOrigin::ReplayDerived,
                    coverage_segment_id: "segment-main".to_owned(),
                    observed_slot: 12,
                    source_revision: 4,
                },
                updated_at_unix: 2,
            }],
            evidence: vec![
                reference(
                    "evidence-config",
                    &channel_id,
                    10,
                    0,
                    ZoneEvidenceKind::ChannelConfiguration,
                    CatalogEvidenceUse::PointSnapshot,
                ),
                reference(
                    "evidence-operation",
                    &channel_id,
                    11,
                    2,
                    ZoneEvidenceKind::ChannelOperation,
                    CatalogEvidenceUse::ReplayContribution,
                ),
                reference(
                    "evidence-raw",
                    &channel_id,
                    12,
                    0,
                    ZoneEvidenceKind::RawInscription,
                    CatalogEvidenceUse::Presence,
                ),
            ],
            segments: vec![segment],
            gaps: Vec::new(),
        }
    }

    fn verified_report(catalog: CatalogSnapshot) -> ZoneCatalogServiceReport {
        ZoneCatalogServiceReport {
            source_revision: 4,
            source_fingerprint: Some("sha256:test".to_owned()),
            verification_state: CatalogVerificationState::Verified,
            catalog: Some(Arc::new(catalog)),
            current_error: None,
            worker_running: false,
        }
    }

    fn reference(
        evidence_id: &str,
        channel_id: &str,
        slot: u64,
        operation_index: u32,
        evidence_kind: ZoneEvidenceKind,
        evidence_use: CatalogEvidenceUse,
    ) -> ZoneEvidenceReference {
        ZoneEvidenceReference {
            evidence_id: evidence_id.to_owned(),
            channel_id: channel_id.to_owned(),
            coverage_segment_id: "segment-main".to_owned(),
            l1_slot: slot,
            block_id: identity(
                char::from_digit(u32::try_from(slot).unwrap_or(0) % 16, 16).unwrap_or('0'),
            ),
            transaction_hash: Some(identity('e')),
            operation_index,
            message_id: None,
            evidence_kind,
            evidence_use,
        }
    }

    fn scope(value: char) -> NetworkScope {
        NetworkScope::GenesisId {
            genesis_id: identity(value),
        }
    }

    fn identity(value: char) -> String {
        value.to_string().repeat(64)
    }
}
