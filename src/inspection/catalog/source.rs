use std::{fmt, future::Future, num::NonZeroUsize, pin::Pin, time::Duration};

use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::Value;

use super::model::{CatalogBlockCheckpoint, CatalogBlockReference};
use crate::{
    blockchain::bedrock::{
        blockchain_block_bounded, blockchain_cryptarchia_info_bounded,
        blockchain_finalized_blocks_response, blockchain_time_info_bounded,
    },
    support::http_response::ensure_response_content_length,
};

pub const DEFAULT_CATALOG_L1_RANGE_BLOCKS: usize = 16;
pub const MAX_CATALOG_L1_RANGE_BLOCKS: usize = 100;

const CATALOG_L1_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CATALOG_METADATA_RESPONSE_MAX_BYTES: usize = 64 * 1024;
const CATALOG_BLOCK_RESPONSE_MAX_BYTES: usize = 8 * 1024 * 1024;
const CATALOG_RANGE_RESPONSE_MAX_BYTES: usize = 64 * 1024 * 1024;
// Mirrors the upstream processed-block NDJSON codec bound.
const CATALOG_NDJSON_LINE_MAX_BYTES: usize = 3 * 1024 * 1024;
const CATALOG_ERROR_RESPONSE_MAX_BYTES: usize = 16 * 1024;

pub type CatalogL1SourceResult<T> = Result<T, CatalogL1SourceError>;
pub type CatalogL1SourceFuture<'a, T> =
    Pin<Box<dyn Future<Output = CatalogL1SourceResult<T>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogL1SourceError {
    InvalidRequest(String),
    Unavailable(String),
    InvalidResponse(String),
}

impl fmt::Display for CatalogL1SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(detail) => {
                write!(formatter, "invalid catalog L1 request: {detail}")
            }
            Self::Unavailable(detail) => {
                write!(formatter, "catalog L1 source unavailable: {detail}")
            }
            Self::InvalidResponse(detail) => {
                write!(formatter, "invalid catalog L1 response: {detail}")
            }
        }
    }
}

impl std::error::Error for CatalogL1SourceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogL1ChainSnapshot {
    pub tip: CatalogBlockReference,
    pub lib: CatalogBlockReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogL1ChainStatus {
    pub snapshot: CatalogL1ChainSnapshot,
    pub genesis_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogL1TimeStatus {
    pub genesis_time_unix_ms: i64,
    pub slot_duration_ms: u64,
    pub current_slot: u64,
    pub current_epoch: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogL1Block {
    pub checkpoint: CatalogBlockCheckpoint,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogL1BlockEvent {
    pub block: CatalogL1Block,
    pub snapshot: CatalogL1ChainSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogL1RangeRequest {
    slot_from: u64,
    target_lib: CatalogBlockReference,
    blocks_limit: NonZeroUsize,
}

impl CatalogL1RangeRequest {
    pub fn new(
        slot_from: u64,
        target_lib: CatalogBlockReference,
        blocks_limit: usize,
    ) -> CatalogL1SourceResult<Self> {
        let target_lib = canonical_block_reference(target_lib, "target LIB")?;
        if slot_from > target_lib.slot {
            return Err(CatalogL1SourceError::InvalidRequest(
                "range start is beyond target LIB slot".to_owned(),
            ));
        }
        let blocks_limit = NonZeroUsize::new(blocks_limit).ok_or_else(|| {
            CatalogL1SourceError::InvalidRequest(
                "range block limit must be greater than zero".to_owned(),
            )
        })?;
        if blocks_limit.get() > MAX_CATALOG_L1_RANGE_BLOCKS {
            return Err(CatalogL1SourceError::InvalidRequest(format!(
                "range block limit exceeds {MAX_CATALOG_L1_RANGE_BLOCKS}"
            )));
        }
        Ok(Self {
            slot_from,
            target_lib,
            blocks_limit,
        })
    }

    #[must_use]
    pub const fn slot_from(&self) -> u64 {
        self.slot_from
    }

    #[must_use]
    pub const fn slot_to(&self) -> u64 {
        self.target_lib.slot
    }

    #[must_use]
    pub const fn target_lib(&self) -> &CatalogBlockReference {
        &self.target_lib
    }

    #[must_use]
    pub const fn blocks_limit(&self) -> NonZeroUsize {
        self.blocks_limit
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogL1RangePage {
    pub events: Vec<CatalogL1BlockEvent>,
}

impl CatalogL1RangePage {
    #[must_use]
    pub fn source_snapshot(&self) -> Option<&CatalogL1ChainSnapshot> {
        self.events.first().map(|event| &event.snapshot)
    }
}

pub trait CatalogL1Source: Send + Sync {
    fn chain_status(&self) -> CatalogL1SourceFuture<'_, CatalogL1ChainStatus>;

    fn time_status(&self) -> CatalogL1SourceFuture<'_, CatalogL1TimeStatus>;

    fn finalized_range(
        &self,
        request: CatalogL1RangeRequest,
    ) -> CatalogL1SourceFuture<'_, CatalogL1RangePage>;

    fn block(&self, block_id: String) -> CatalogL1SourceFuture<'_, Option<CatalogL1Block>>;
}

#[derive(Clone)]
pub struct DirectCatalogL1Source {
    endpoint: String,
    client: Client,
    limits: CatalogL1SourceLimits,
}

impl DirectCatalogL1Source {
    pub fn new(endpoint: impl AsRef<str>) -> CatalogL1SourceResult<Self> {
        let client = Client::builder()
            .timeout(CATALOG_L1_REQUEST_TIMEOUT)
            .build()
            .map_err(|error| CatalogL1SourceError::Unavailable(error.to_string()))?;
        Self::with_client(endpoint, client)
    }

    pub fn with_client(endpoint: impl AsRef<str>, client: Client) -> CatalogL1SourceResult<Self> {
        Ok(Self {
            endpoint: canonical_endpoint(endpoint.as_ref())?,
            client,
            limits: CatalogL1SourceLimits::default(),
        })
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    async fn fetch_chain_status(&self) -> CatalogL1SourceResult<CatalogL1ChainStatus> {
        let value = blockchain_cryptarchia_info_bounded(
            &self.client,
            &self.endpoint,
            self.limits.metadata_response_bytes,
        )
        .await
        .map_err(|error| source_unavailable("Cryptarchia info request failed", error))?;
        parse_chain_status(&value)
    }

    async fn fetch_time_status(&self) -> CatalogL1SourceResult<CatalogL1TimeStatus> {
        let value = blockchain_time_info_bounded(
            &self.client,
            &self.endpoint,
            self.limits.metadata_response_bytes,
        )
        .await
        .map_err(|error| source_unavailable("time info request failed", error))?;
        parse_time_status(&value)
    }

    async fn fetch_finalized_range(
        &self,
        request: CatalogL1RangeRequest,
    ) -> CatalogL1SourceResult<CatalogL1RangePage> {
        let mut response = blockchain_finalized_blocks_response(
            &self.client,
            &self.endpoint,
            request.slot_from(),
            request.slot_to(),
            request.blocks_limit(),
            self.limits.error_response_bytes,
        )
        .await
        .map_err(|error| source_unavailable("finalized range request failed", error))?;
        ensure_response_content_length(&response, self.limits.range_response_bytes)
            .map_err(|error| invalid_response(error.to_string()))?;

        let mut parser = CatalogL1NdjsonParser::new(request, self.limits);
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| source_unavailable("finalized range body read failed", error))?
        {
            parser.push_chunk(&chunk)?;
        }
        parser.finish()
    }

    async fn fetch_block(&self, block_id: String) -> CatalogL1SourceResult<Option<CatalogL1Block>> {
        let requested_id = canonical_hex_id(&block_id, "requested block id")?;
        let value = blockchain_block_bounded(
            &self.client,
            &self.endpoint,
            &requested_id,
            self.limits.block_response_bytes,
        )
        .await
        .map_err(|error| source_unavailable("block detail request failed", error))?;
        value
            .map(|value| {
                let block = parse_block(value, "block detail")?;
                if block.checkpoint.block_id != requested_id {
                    return Err(invalid_response(format!(
                        "block detail returned id {}, requested {requested_id}",
                        block.checkpoint.block_id
                    )));
                }
                Ok(block)
            })
            .transpose()
    }

    #[cfg(test)]
    fn with_limits(mut self, limits: CatalogL1SourceLimits) -> Self {
        self.limits = limits;
        self
    }
}

impl CatalogL1Source for DirectCatalogL1Source {
    fn chain_status(&self) -> CatalogL1SourceFuture<'_, CatalogL1ChainStatus> {
        Box::pin(self.fetch_chain_status())
    }

    fn time_status(&self) -> CatalogL1SourceFuture<'_, CatalogL1TimeStatus> {
        Box::pin(self.fetch_time_status())
    }

    fn finalized_range(
        &self,
        request: CatalogL1RangeRequest,
    ) -> CatalogL1SourceFuture<'_, CatalogL1RangePage> {
        Box::pin(self.fetch_finalized_range(request))
    }

    fn block(&self, block_id: String) -> CatalogL1SourceFuture<'_, Option<CatalogL1Block>> {
        Box::pin(self.fetch_block(block_id))
    }
}

#[derive(Debug, Clone, Copy)]
struct CatalogL1SourceLimits {
    metadata_response_bytes: usize,
    block_response_bytes: usize,
    range_response_bytes: usize,
    ndjson_line_bytes: usize,
    error_response_bytes: usize,
}

impl Default for CatalogL1SourceLimits {
    fn default() -> Self {
        Self {
            metadata_response_bytes: CATALOG_METADATA_RESPONSE_MAX_BYTES,
            block_response_bytes: CATALOG_BLOCK_RESPONSE_MAX_BYTES,
            range_response_bytes: CATALOG_RANGE_RESPONSE_MAX_BYTES,
            ndjson_line_bytes: CATALOG_NDJSON_LINE_MAX_BYTES,
            error_response_bytes: CATALOG_ERROR_RESPONSE_MAX_BYTES,
        }
    }
}

pub fn parse_catalog_l1_range_ndjson(
    body: &[u8],
    request: CatalogL1RangeRequest,
) -> CatalogL1SourceResult<CatalogL1RangePage> {
    let mut parser = CatalogL1NdjsonParser::new(request, CatalogL1SourceLimits::default());
    parser.push_chunk(body)?;
    parser.finish()
}

struct CatalogL1NdjsonParser {
    request: CatalogL1RangeRequest,
    limits: CatalogL1SourceLimits,
    pending: Vec<u8>,
    events: Vec<CatalogL1BlockEvent>,
    line_number: usize,
    received_bytes: usize,
    snapshot: Option<CatalogL1ChainSnapshot>,
    previous_slot: Option<u64>,
}

impl CatalogL1NdjsonParser {
    fn new(request: CatalogL1RangeRequest, limits: CatalogL1SourceLimits) -> Self {
        Self {
            request,
            limits,
            pending: Vec::new(),
            events: Vec::new(),
            line_number: 0,
            received_bytes: 0,
            snapshot: None,
            previous_slot: None,
        }
    }

    fn push_chunk(&mut self, chunk: &[u8]) -> CatalogL1SourceResult<()> {
        self.received_bytes = self
            .received_bytes
            .checked_add(chunk.len())
            .ok_or_else(|| invalid_response("finalized range body length overflow"))?;
        if self.received_bytes > self.limits.range_response_bytes {
            return Err(invalid_response(format!(
                "finalized range body exceeded {} byte limit",
                self.limits.range_response_bytes
            )));
        }
        self.pending.extend_from_slice(chunk);

        while let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
            let trailing = self.pending.split_off(newline.saturating_add(1));
            let mut line = std::mem::replace(&mut self.pending, trailing);
            line.truncate(newline);
            self.line_number = self.line_number.saturating_add(1);
            self.parse_line(&line)?;
        }
        if self.pending.len() > self.limits.ndjson_line_bytes {
            return Err(invalid_response(format!(
                "finalized range event on line {} exceeded {} byte limit",
                self.line_number.saturating_add(1),
                self.limits.ndjson_line_bytes
            )));
        }
        Ok(())
    }

    fn finish(mut self) -> CatalogL1SourceResult<CatalogL1RangePage> {
        if !self.pending.is_empty() {
            let line = std::mem::take(&mut self.pending);
            self.line_number = self.line_number.saturating_add(1);
            self.parse_line(&line)?;
        }
        Ok(CatalogL1RangePage {
            events: self.events,
        })
    }

    fn parse_line(&mut self, line: &[u8]) -> CatalogL1SourceResult<()> {
        if line.iter().all(u8::is_ascii_whitespace) {
            return Ok(());
        }
        if line.len() > self.limits.ndjson_line_bytes {
            return Err(invalid_response(format!(
                "finalized range event on line {} exceeded {} byte limit",
                self.line_number, self.limits.ndjson_line_bytes
            )));
        }
        if self.events.len() >= self.request.blocks_limit().get() {
            return Err(invalid_response(format!(
                "finalized range returned more than {} requested blocks",
                self.request.blocks_limit()
            )));
        }

        let wire: WireProcessedBlockEvent = serde_json::from_slice(line).map_err(|error| {
            invalid_response(format!(
                "invalid finalized range event on line {}: {error}; line={}",
                self.line_number,
                response_preview(line)
            ))
        })?;
        let event = parse_wire_event(wire, &self.request)?;
        if self
            .previous_slot
            .is_some_and(|previous| event.block.checkpoint.slot <= previous)
        {
            return Err(invalid_response(format!(
                "finalized range block slots are not strictly ascending at line {}",
                self.line_number
            )));
        }
        if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot != &event.snapshot)
        {
            return Err(invalid_response(format!(
                "finalized range chain snapshot changed at line {}",
                self.line_number
            )));
        }

        self.previous_slot = Some(event.block.checkpoint.slot);
        self.snapshot.get_or_insert_with(|| event.snapshot.clone());
        self.events.push(event);
        Ok(())
    }
}

#[derive(Deserialize)]
struct WireProcessedBlockEvent {
    block: Value,
    tip: Value,
    tip_slot: Value,
    lib: Value,
    lib_slot: Value,
}

fn parse_wire_event(
    wire: WireProcessedBlockEvent,
    request: &CatalogL1RangeRequest,
) -> CatalogL1SourceResult<CatalogL1BlockEvent> {
    let block = parse_block(wire.block, "finalized range event")?;
    let snapshot = CatalogL1ChainSnapshot {
        tip: CatalogBlockReference {
            slot: required_u64(&wire.tip_slot, "event tip_slot")?,
            block_id: required_hex_id(&wire.tip, "event tip")?,
        },
        lib: CatalogBlockReference {
            slot: required_u64(&wire.lib_slot, "event lib_slot")?,
            block_id: required_hex_id(&wire.lib, "event lib")?,
        },
    };
    validate_snapshot(&snapshot)?;

    if block.checkpoint.slot < request.slot_from() || block.checkpoint.slot > request.slot_to() {
        return Err(invalid_response(format!(
            "finalized block slot {} is outside requested range {}..={}",
            block.checkpoint.slot,
            request.slot_from(),
            request.slot_to()
        )));
    }
    if block.checkpoint.slot > snapshot.lib.slot {
        return Err(invalid_response(format!(
            "finalized block slot {} is beyond source LIB slot {}",
            block.checkpoint.slot, snapshot.lib.slot
        )));
    }
    if snapshot.lib.slot < request.target_lib().slot {
        return Err(invalid_response(format!(
            "source LIB slot {} is behind target LIB slot {}",
            snapshot.lib.slot,
            request.target_lib().slot
        )));
    }
    if snapshot.lib.slot == request.target_lib().slot
        && snapshot.lib.block_id != request.target_lib().block_id
    {
        return Err(invalid_response(
            "source LIB id conflicts with fixed target LIB".to_owned(),
        ));
    }
    if block.checkpoint.slot == request.target_lib().slot
        && block.checkpoint.block_id != request.target_lib().block_id
    {
        return Err(invalid_response(
            "block at target LIB slot has conflicting id".to_owned(),
        ));
    }

    Ok(CatalogL1BlockEvent { block, snapshot })
}

fn parse_block(value: Value, label: &str) -> CatalogL1SourceResult<CatalogL1Block> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_response(format!("{label} block is not an object")))?;
    let header = object
        .get("header")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_response(format!("{label} block header is missing")))?;
    if object
        .get("transactions")
        .and_then(Value::as_array)
        .is_none()
    {
        return Err(invalid_response(format!(
            "{label} block transactions body is missing"
        )));
    }

    let checkpoint = CatalogBlockCheckpoint {
        slot: header
            .get("slot")
            .map(|field| required_u64(field, "block header slot"))
            .transpose()?
            .ok_or_else(|| invalid_response("block header slot is missing"))?,
        block_id: header
            .get("id")
            .map(|field| required_hex_id(field, "block header id"))
            .transpose()?
            .ok_or_else(|| invalid_response("block header id is missing"))?,
        parent_id: header
            .get("parent_block")
            .map(|field| required_hex_id(field, "block header parent_block"))
            .transpose()?
            .ok_or_else(|| invalid_response("block header parent_block is missing"))?,
    };
    Ok(CatalogL1Block {
        checkpoint,
        payload: value,
    })
}

fn parse_chain_status(value: &Value) -> CatalogL1SourceResult<CatalogL1ChainStatus> {
    let source = value
        .get("cryptarchia_info")
        .filter(|nested| nested.is_object())
        .unwrap_or(value);
    let snapshot = CatalogL1ChainSnapshot {
        tip: CatalogBlockReference {
            slot: required_object_u64(source, &["slot", "tip_slot"], "tip slot")?,
            block_id: required_object_hex_id(source, &["tip", "tip_hash"], "tip id")?,
        },
        lib: CatalogBlockReference {
            slot: required_object_u64(source, &["lib_slot"], "LIB slot")?,
            block_id: required_object_hex_id(source, &["lib", "lib_hash"], "LIB id")?,
        },
    };
    validate_snapshot(&snapshot)?;
    let genesis_id = source
        .get("genesis_id")
        .or_else(|| value.get("genesis_id"))
        .map(|field| required_hex_id(field, "genesis id"))
        .transpose()?;
    Ok(CatalogL1ChainStatus {
        snapshot,
        genesis_id,
    })
}

fn parse_time_status(value: &Value) -> CatalogL1SourceResult<CatalogL1TimeStatus> {
    let current_epoch = required_object_u64(value, &["current_epoch"], "current epoch")?;
    Ok(CatalogL1TimeStatus {
        genesis_time_unix_ms: required_object_i64(
            value,
            &["genesis_time_unix_ms"],
            "genesis time",
        )?,
        slot_duration_ms: required_object_u64(value, &["slot_duration_ms"], "slot duration")?,
        current_slot: required_object_u64(value, &["current_slot"], "current slot")?,
        current_epoch: u32::try_from(current_epoch)
            .map_err(|_| invalid_response("current epoch exceeds u32 range"))?,
    })
}

fn validate_snapshot(snapshot: &CatalogL1ChainSnapshot) -> CatalogL1SourceResult<()> {
    if snapshot.lib.slot > snapshot.tip.slot {
        return Err(invalid_response(format!(
            "source LIB slot {} is beyond tip slot {}",
            snapshot.lib.slot, snapshot.tip.slot
        )));
    }
    Ok(())
}

fn required_object_u64(value: &Value, fields: &[&str], label: &str) -> CatalogL1SourceResult<u64> {
    let field = fields
        .iter()
        .find_map(|field| value.get(*field))
        .ok_or_else(|| invalid_response(format!("{label} is missing")))?;
    required_u64(field, label)
}

fn required_object_i64(value: &Value, fields: &[&str], label: &str) -> CatalogL1SourceResult<i64> {
    let field = fields
        .iter()
        .find_map(|field| value.get(*field))
        .ok_or_else(|| invalid_response(format!("{label} is missing")))?;
    field
        .as_i64()
        .or_else(|| field.as_str().and_then(|text| text.trim().parse().ok()))
        .ok_or_else(|| invalid_response(format!("{label} is not an integer")))
}

fn required_object_hex_id(
    value: &Value,
    fields: &[&str],
    label: &str,
) -> CatalogL1SourceResult<String> {
    let field = fields
        .iter()
        .find_map(|field| value.get(*field))
        .ok_or_else(|| invalid_response(format!("{label} is missing")))?;
    required_hex_id(field, label)
}

fn required_u64(value: &Value, label: &str) -> CatalogL1SourceResult<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
        .ok_or_else(|| invalid_response(format!("{label} is not an unsigned integer")))
}

fn required_hex_id(value: &Value, label: &str) -> CatalogL1SourceResult<String> {
    let text = value
        .as_str()
        .ok_or_else(|| invalid_response(format!("{label} is not text")))?;
    canonical_hex_id(text, label).map_err(|error| match error {
        CatalogL1SourceError::InvalidRequest(detail) => invalid_response(detail),
        other => other,
    })
}

fn canonical_block_reference(
    reference: CatalogBlockReference,
    label: &str,
) -> CatalogL1SourceResult<CatalogBlockReference> {
    Ok(CatalogBlockReference {
        slot: reference.slot,
        block_id: canonical_hex_id(&reference.block_id, &format!("{label} id"))?,
    })
}

fn canonical_hex_id(value: &str, label: &str) -> CatalogL1SourceResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CatalogL1SourceError::InvalidRequest(format!(
            "{label} must be 32-byte hexadecimal text"
        )));
    }
    Ok(value)
}

fn canonical_endpoint(endpoint: &str) -> CatalogL1SourceResult<String> {
    let endpoint = endpoint.trim();
    let url = Url::parse(endpoint).map_err(|error| {
        CatalogL1SourceError::InvalidRequest(format!("invalid endpoint URL: {error}"))
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(CatalogL1SourceError::InvalidRequest(
            "endpoint must be an HTTP URL with a host".to_owned(),
        ));
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(CatalogL1SourceError::InvalidRequest(
            "endpoint cannot contain credentials, query, or fragment".to_owned(),
        ));
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

fn source_unavailable(context: &str, error: impl fmt::Display) -> CatalogL1SourceError {
    CatalogL1SourceError::Unavailable(format!("{context}: {error}"))
}

fn invalid_response(detail: impl Into<String>) -> CatalogL1SourceError {
    CatalogL1SourceError::InvalidResponse(detail.into())
}

fn response_preview(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::*;

    #[test]
    fn range_request_canonicalizes_target_and_rejects_invalid_limits() -> Result<()> {
        let target = CatalogBlockReference {
            slot: 20,
            block_id: "A".repeat(64),
        };

        let request = CatalogL1RangeRequest::new(10, target.clone(), 4)?;

        if request.target_lib().block_id != "a".repeat(64) {
            bail!("target LIB id was not canonicalized");
        }
        if CatalogL1RangeRequest::new(21, target.clone(), 4).is_ok() {
            bail!("range beyond target should fail");
        }
        if CatalogL1RangeRequest::new(10, target.clone(), 0).is_ok() {
            bail!("zero block limit should fail");
        }
        if CatalogL1RangeRequest::new(10, target, MAX_CATALOG_L1_RANGE_BLOCKS + 1).is_ok() {
            bail!("oversized block limit should fail");
        }
        Ok(())
    }

    #[test]
    fn ndjson_parser_preserves_chunked_events_and_fixed_snapshot() -> Result<()> {
        let target = block_reference(20, 'd');
        let request = CatalogL1RangeRequest::new(10, target, 3)?;
        let mut body = event_line(10, 'a', '0', 30, 'f', 20, 'd')?;
        body.extend(event_line(14, 'b', 'a', 30, 'f', 20, 'd')?);
        let (first, second) = body.split_at(17);
        let mut parser = CatalogL1NdjsonParser::new(request, CatalogL1SourceLimits::default());

        parser.push_chunk(first)?;
        parser.push_chunk(second)?;
        let page = parser.finish()?;

        if page.events.len() != 2 {
            bail!("expected two events, got {}", page.events.len());
        }
        let first = page.events.first().context("first event should exist")?;
        let second = page.events.get(1).context("second event should exist")?;
        if first.block.checkpoint.slot != 10
            || second.block.checkpoint.slot != 14
            || first.snapshot != second.snapshot
            || first.snapshot.lib != block_reference(20, 'd')
        {
            bail!("parsed events did not preserve block and snapshot fields");
        }
        Ok(())
    }

    #[test]
    fn ndjson_parser_preserves_parent_break_for_ingestion_engine() -> Result<()> {
        let request = CatalogL1RangeRequest::new(10, block_reference(20, 'd'), 3)?;
        let mut body = event_line(10, 'a', '0', 30, 'f', 20, 'd')?;
        body.extend(event_line(14, 'b', '9', 30, 'f', 20, 'd')?);

        let page = parse_catalog_l1_range_ndjson(&body, request)?;

        let second = page.events.get(1).context("second event should exist")?;
        if second.block.checkpoint.parent_id != id('9') {
            bail!("parser did not preserve parent break for engine validation");
        }
        Ok(())
    }

    #[test]
    fn ndjson_parser_accepts_empty_body_without_claiming_snapshot() -> Result<()> {
        let request = CatalogL1RangeRequest::new(1, block_reference(20, 'd'), 3)?;

        let page = parse_catalog_l1_range_ndjson(b"\n \r\n", request)?;

        if !page.events.is_empty() || page.source_snapshot().is_some() {
            bail!("empty range should have no events or source snapshot");
        }
        Ok(())
    }

    #[test]
    fn ndjson_parser_reports_malformed_line() -> Result<()> {
        let request = CatalogL1RangeRequest::new(1, block_reference(20, 'd'), 3)?;
        let mut body = event_line(10, 'a', '0', 30, 'f', 20, 'd')?;
        body.extend_from_slice(b"{not-json}\n");

        let error = parse_catalog_l1_range_ndjson(&body, request.clone())
            .err()
            .context("malformed line should fail")?;

        if !error.to_string().contains("line 2") {
            bail!("malformed line error lacked line number: {error}");
        }
        Ok(())
    }

    #[test]
    fn ndjson_parser_rejects_missing_block_or_transactions_body() -> Result<()> {
        let request = CatalogL1RangeRequest::new(1, block_reference(20, 'd'), 3)?;
        let body = serde_json::to_vec(&json!({
            "tip": id('f'),
            "tip_slot": 30,
            "lib": id('d'),
            "lib_slot": 20
        }))?;

        let error = parse_catalog_l1_range_ndjson(&body, request.clone())
            .err()
            .context("missing block should fail")?;

        if !error.to_string().contains("missing field `block`") {
            bail!("unexpected missing block error: {error}");
        }

        let body = serde_json::to_vec(&json!({
            "block": {
                "header": {
                    "id": id('a'),
                    "parent_block": id('0'),
                    "slot": 10
                }
            },
            "tip": id('f'),
            "tip_slot": 30,
            "lib": id('d'),
            "lib_slot": 20
        }))?;
        let error = parse_catalog_l1_range_ndjson(&body, request)
            .err()
            .context("missing transactions body should fail")?;
        if !error.to_string().contains("transactions body is missing") {
            bail!("unexpected missing transactions error: {error}");
        }
        Ok(())
    }

    #[test]
    fn ndjson_parser_rejects_json_array_envelope() -> Result<()> {
        let request = CatalogL1RangeRequest::new(1, block_reference(20, 'd'), 3)?;

        let error = parse_catalog_l1_range_ndjson(b"[]", request)
            .err()
            .context("JSON array should not satisfy NDJSON event contract")?;

        if !error.to_string().contains("line 1") {
            bail!("unexpected array-envelope error: {error}");
        }
        Ok(())
    }

    #[test]
    fn ndjson_parser_rejects_out_of_range_and_nonascending_blocks() -> Result<()> {
        let request = CatalogL1RangeRequest::new(10, block_reference(20, 'd'), 3)?;
        let outside = event_line(9, 'a', '0', 30, 'f', 20, 'd')?;
        let outside_error = parse_catalog_l1_range_ndjson(&outside, request.clone())
            .err()
            .context("out-of-range block should fail")?;
        if !outside_error
            .to_string()
            .contains("outside requested range")
        {
            bail!("unexpected range error: {outside_error}");
        }

        let mut descending = event_line(14, 'b', 'a', 30, 'f', 20, 'd')?;
        descending.extend(event_line(12, 'c', 'b', 30, 'f', 20, 'd')?);
        let descending_error = parse_catalog_l1_range_ndjson(&descending, request)
            .err()
            .context("descending blocks should fail")?;
        if !descending_error.to_string().contains("strictly ascending") {
            bail!("unexpected ordering error: {descending_error}");
        }
        Ok(())
    }

    #[test]
    fn ndjson_parser_rejects_changed_or_behind_snapshot() -> Result<()> {
        let request = CatalogL1RangeRequest::new(10, block_reference(20, 'd'), 3)?;
        let mut changed = event_line(10, 'a', '0', 30, 'f', 21, 'e')?;
        changed.extend(event_line(14, 'b', 'a', 31, '1', 22, '2')?);
        let changed_error = parse_catalog_l1_range_ndjson(&changed, request.clone())
            .err()
            .context("changed snapshot should fail")?;
        if !changed_error.to_string().contains("snapshot changed") {
            bail!("unexpected snapshot error: {changed_error}");
        }

        let behind = event_line(10, 'a', '0', 19, 'f', 19, 'c')?;
        let behind_error = parse_catalog_l1_range_ndjson(&behind, request)
            .err()
            .context("behind snapshot should fail")?;
        if !behind_error.to_string().contains("behind target LIB") {
            bail!("unexpected behind-target error: {behind_error}");
        }
        Ok(())
    }

    #[test]
    fn ndjson_parser_rejects_target_identity_mismatch() -> Result<()> {
        let request = CatalogL1RangeRequest::new(10, block_reference(20, 'd'), 3)?;
        let body = event_line(20, 'e', 'a', 20, 'e', 20, 'e')?;

        let error = parse_catalog_l1_range_ndjson(&body, request)
            .err()
            .context("target mismatch should fail")?;

        if !error.to_string().contains("fixed target LIB") {
            bail!("unexpected target mismatch error: {error}");
        }
        Ok(())
    }

    #[test]
    fn ndjson_parser_enforces_body_line_and_event_limits() -> Result<()> {
        let request = CatalogL1RangeRequest::new(1, block_reference(20, 'd'), 1)?;
        let limits = CatalogL1SourceLimits {
            range_response_bytes: 10,
            ndjson_line_bytes: 5,
            ..CatalogL1SourceLimits::default()
        };
        let mut parser = CatalogL1NdjsonParser::new(request.clone(), limits);
        let body_error = parser
            .push_chunk(b"01234567890")
            .err()
            .context("oversized body should fail")?;
        if !body_error.to_string().contains("body exceeded") {
            bail!("unexpected body limit error: {body_error}");
        }

        let limits = CatalogL1SourceLimits {
            range_response_bytes: 100,
            ndjson_line_bytes: 5,
            ..CatalogL1SourceLimits::default()
        };
        let mut parser = CatalogL1NdjsonParser::new(request, limits);
        let line_error = parser
            .push_chunk(b"123456")
            .err()
            .context("oversized line should fail")?;
        if !line_error.to_string().contains("event on line 1") {
            bail!("unexpected line limit error: {line_error}");
        }

        let mut events = event_line(10, 'a', '0', 30, 'f', 20, 'd')?;
        events.extend(event_line(14, 'b', 'a', 30, 'f', 20, 'd')?);
        let request = CatalogL1RangeRequest::new(1, block_reference(20, 'd'), 1)?;
        let event_error = parse_catalog_l1_range_ndjson(&events, request)
            .err()
            .context("too many events should fail")?;
        if !event_error
            .to_string()
            .contains("more than 1 requested blocks")
        {
            bail!("unexpected event limit error: {event_error}");
        }
        Ok(())
    }

    #[test]
    fn block_parser_requires_full_body_and_canonicalizes_identity() -> Result<()> {
        let block = parse_block(
            json!({
                "header": {
                    "id": "A".repeat(64),
                    "parent_block": "B".repeat(64),
                    "slot": 10
                },
                "transactions": []
            }),
            "test",
        )?;

        if block.checkpoint.block_id != id('a') || block.checkpoint.parent_id != id('b') {
            bail!("block identity was not canonicalized: {block:?}");
        }
        Ok(())
    }

    #[test]
    fn chain_and_time_status_parse_current_wire_shapes() -> Result<()> {
        let chain = parse_chain_status(&json!({
            "cryptarchia_info": {
                "lib": id('d'),
                "lib_slot": "20",
                "tip": id('f'),
                "slot": 30,
                "genesis_id": id('0')
            },
            "mode": { "Started": "Online" }
        }))?;
        if chain.snapshot.lib != block_reference(20, 'd')
            || chain.genesis_id.as_deref() != Some(id('0').as_str())
        {
            bail!("unexpected parsed chain status: {chain:?}");
        }

        let time = parse_time_status(&json!({
            "genesis_time_unix_ms": "1000",
            "slot_duration_ms": 500,
            "current_slot": 30,
            "current_epoch": 2
        }))?;
        if time.genesis_time_unix_ms != 1000
            || time.slot_duration_ms != 500
            || time.current_slot != 30
            || time.current_epoch != 2
        {
            bail!("unexpected parsed time status: {time:?}");
        }
        Ok(())
    }

    #[test]
    fn direct_source_rejects_endpoint_credentials_and_normalizes_slash() -> Result<()> {
        let source = DirectCatalogL1Source::new("http://localhost:8080/")?.with_limits(
            CatalogL1SourceLimits {
                range_response_bytes: 1024,
                ..CatalogL1SourceLimits::default()
            },
        );
        if source.endpoint() != "http://localhost:8080" {
            bail!("unexpected normalized endpoint: {}", source.endpoint());
        }
        if DirectCatalogL1Source::new("http://user:secret@localhost:8080").is_ok() {
            bail!("credential-bearing endpoint should fail");
        }
        Ok(())
    }

    fn event_line(
        slot: u64,
        block: char,
        parent: char,
        tip_slot: u64,
        tip: char,
        lib_slot: u64,
        lib: char,
    ) -> Result<Vec<u8>> {
        let mut line = serde_json::to_vec(&json!({
            "block": {
                "header": {
                    "id": id(block),
                    "parent_block": id(parent),
                    "slot": slot
                },
                "transactions": []
            },
            "tip": id(tip),
            "tip_slot": tip_slot,
            "lib": id(lib),
            "lib_slot": lib_slot
        }))?;
        line.push(b'\n');
        Ok(line)
    }

    fn block_reference(slot: u64, value: char) -> CatalogBlockReference {
        CatalogBlockReference {
            slot,
            block_id: id(value),
        }
    }

    fn id(value: char) -> String {
        value.to_string().repeat(64)
    }
}
