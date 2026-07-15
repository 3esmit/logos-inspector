use std::{
    borrow::Cow,
    collections::{BTreeMap, VecDeque},
    sync::Mutex,
};

use anyhow::{Context as _, Result, anyhow};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use url::Url;

use crate::{
    source_routing::{
        SourceFamily, messaging_layer, network_adapter_policy_for_connector, source_mode_policy,
        storage_layer,
    },
    support::args::Args,
};

use super::{
    CapabilityBuildMode, CapabilityRegistryReport, CapabilityRuntimeInputs,
    capability_registry_report_with_inputs,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourceScope {
    L1,
    Storage,
    Delivery,
}

impl SourceScope {
    const fn family(self) -> SourceFamily {
        match self {
            Self::L1 => SourceFamily::Core,
            Self::Storage => SourceFamily::Storage,
            Self::Delivery => SourceFamily::Delivery,
        }
    }

    const fn key(self) -> &'static str {
        match self {
            Self::L1 => "l1",
            Self::Storage => "storage",
            Self::Delivery => "delivery",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ModuleScope {
    Blockchain,
    Storage,
    Delivery,
}

impl ModuleScope {
    const fn key(self) -> &'static str {
        match self {
            Self::Blockchain => "blockchain",
            Self::Storage => "storage",
            Self::Delivery => "delivery",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LastKnownScope {
    Wallet,
    LocalNodes,
}

impl LastKnownScope {
    const fn key(self) -> &'static str {
        match self {
            Self::Wallet => "wallet",
            Self::LocalNodes => "local_nodes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ObservationTarget {
    L1Connectivity,
    Source(SourceScope),
    Module(ModuleScope),
    LastKnown(LastKnownScope),
}

impl ObservationTarget {
    const fn requires_configuration(self) -> bool {
        matches!(self, Self::L1Connectivity | Self::Source(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ConfigurationIdentity([u8; 32]);

impl ConfigurationIdentity {
    fn new(
        scope: SourceScope,
        connector: &str,
        endpoint: &str,
        supplemental_endpoint: &str,
        generation: Option<u64>,
    ) -> Self {
        let mut hasher = Sha256::new();
        let endpoint = canonical_endpoint(endpoint);
        let supplemental_endpoint = canonical_endpoint(supplemental_endpoint);
        for part in [
            "capability-evidence-v1",
            scope.key(),
            connector.trim(),
            endpoint.as_ref(),
            supplemental_endpoint.as_ref(),
        ] {
            hasher.update(part.len().to_be_bytes());
            hasher.update(part.as_bytes());
        }
        hasher.update([u8::from(generation.is_some())]);
        if let Some(generation) = generation {
            hasher.update(generation.to_be_bytes());
        }
        Self(hasher.finalize().into())
    }
}

fn canonical_endpoint(endpoint: &str) -> Cow<'_, str> {
    let endpoint = endpoint.trim();
    let Ok(mut parsed) = Url::parse(endpoint) else {
        return Cow::Borrowed(endpoint);
    };
    parsed.set_fragment(None);
    Cow::Owned(parsed.to_string())
}

#[derive(Debug, Clone, Default)]
pub(super) struct CapabilityEvidenceSnapshot {
    source_reports: BTreeMap<SourceScope, Value>,
    module_reports: BTreeMap<ModuleScope, Value>,
    last_known: BTreeMap<LastKnownScope, String>,
}

impl CapabilityEvidenceSnapshot {
    pub(super) fn source_report(&self, scope: &str) -> Option<&Value> {
        let scope = match scope {
            "l1" => SourceScope::L1,
            "storage" => SourceScope::Storage,
            "delivery" => SourceScope::Delivery,
            _ => return None,
        };
        self.source_reports.get(&scope)
    }

    pub(super) fn diagnostics_report(&self) -> Option<Value> {
        if self.source_reports.is_empty()
            && self.module_reports.is_empty()
            && self.last_known.is_empty()
        {
            return None;
        }
        let module_reports = self
            .module_reports
            .iter()
            .map(|(scope, report)| (scope.key().to_owned(), report.clone()))
            .collect::<serde_json::Map<_, _>>();
        let source_reports = self
            .source_reports
            .iter()
            .map(|(scope, report)| (scope.key().to_owned(), report.clone()))
            .collect::<serde_json::Map<_, _>>();
        let last_known = self
            .last_known
            .iter()
            .map(|(scope, detail)| (scope.key().to_owned(), Value::String(detail.clone())))
            .collect::<serde_json::Map<_, _>>();
        Some(json!({
            "module_reports": module_reports,
            "source_reports": source_reports,
            "last_known": last_known,
        }))
    }

    #[cfg(test)]
    pub(super) fn from_legacy_test_value(value: Option<&Value>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };
        let mut evidence = Self::default();
        if let Some(reports) = value.get("source_reports").and_then(Value::as_object) {
            for (key, report) in reports {
                let scope = match key.as_str() {
                    "l1" | "blockchain" | "node" => Some(SourceScope::L1),
                    "storage" | "storage_source" => Some(SourceScope::Storage),
                    "delivery" | "delivery_source" | "messaging" | "messaging_source" => {
                        Some(SourceScope::Delivery)
                    }
                    _ => None,
                };
                if let Some(scope) = scope {
                    evidence.source_reports.insert(scope, report.clone());
                }
            }
        }
        if let Some(report) = value.get("diagnostics_reports").and_then(Value::as_object) {
            evidence.import_test_diagnostics(report);
        }
        evidence
    }

    #[cfg(test)]
    fn import_test_diagnostics(&mut self, report: &serde_json::Map<String, Value>) {
        if let Some(reports) = report.get("module_reports").and_then(Value::as_object) {
            for (key, value) in reports {
                let scope = match key.as_str() {
                    "blockchain" | "l1" => Some(ModuleScope::Blockchain),
                    "storage" => Some(ModuleScope::Storage),
                    "delivery" | "messaging" => Some(ModuleScope::Delivery),
                    _ => None,
                };
                if let Some(scope) = scope {
                    self.module_reports.insert(scope, value.clone());
                }
            }
        }
        if let Some(reports) = report.get("source_reports").and_then(Value::as_object) {
            for (key, value) in reports {
                let scope = match key.as_str() {
                    "l1" | "blockchain" | "node" => Some(SourceScope::L1),
                    "storage" | "storage_source" => Some(SourceScope::Storage),
                    "delivery" | "delivery_source" | "messaging" | "messaging_source" => {
                        Some(SourceScope::Delivery)
                    }
                    _ => None,
                };
                if let Some(scope) = scope {
                    self.source_reports.insert(scope, value.clone());
                }
            }
        }
        if let Some(last_known) = report.get("last_known").and_then(Value::as_object) {
            for (key, value) in last_known {
                let scope = match key.as_str() {
                    "wallet" => Some(LastKnownScope::Wallet),
                    "local_nodes" | "localNodes" => Some(LastKnownScope::LocalNodes),
                    _ => None,
                };
                if let (Some(scope), Some(detail)) = (scope, value.as_str())
                    && !detail.trim().is_empty()
                {
                    self.last_known.insert(scope, detail.trim().to_owned());
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObservationSequence(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ObservationKey {
    target: ObservationTarget,
    configuration: Option<ConfigurationIdentity>,
}

#[derive(Debug)]
pub(crate) struct CapabilityObservation {
    key: ObservationKey,
    sequence: ObservationSequence,
}

#[derive(Debug)]
struct RuntimeCapabilityObservation {
    observation: CapabilityObservation,
    configuration_generation: Option<u64>,
}

const MAX_SOURCE_CONFIGURATIONS: usize = 8;
const MAX_ACTIVE_OBSERVATIONS: usize = 64;

#[derive(Debug)]
struct ConfiguredSourceReport {
    configuration: ConfigurationIdentity,
    value: Value,
}

#[derive(Debug, Default)]
struct ConfiguredSourceReports {
    entries: VecDeque<ConfiguredSourceReport>,
}

impl ConfiguredSourceReports {
    fn insert(
        &mut self,
        configuration: ConfigurationIdentity,
        value: Value,
        protected: Option<ConfigurationIdentity>,
    ) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.configuration == configuration)
        {
            self.entries.remove(index);
        }
        self.entries.push_back(ConfiguredSourceReport {
            configuration,
            value,
        });
        while self.entries.len() > MAX_SOURCE_CONFIGURATIONS {
            let removable = self
                .entries
                .iter()
                .position(|entry| Some(entry.configuration) != protected);
            let Some(index) = removable else {
                break;
            };
            self.entries.remove(index);
        }
    }

    fn get(&self, configuration: ConfigurationIdentity) -> Option<&Value> {
        self.entries
            .iter()
            .find(|entry| entry.configuration == configuration)
            .map(|entry| &entry.value)
    }
}

#[derive(Debug, Default)]
struct CapabilityRegistryState {
    evidence: CapabilityEvidenceSnapshot,
    configured_sources: BTreeMap<SourceScope, ConfiguredSourceReports>,
    selected_configurations: BTreeMap<SourceScope, ConfigurationIdentity>,
    latest: BTreeMap<ObservationKey, ObservationSequence>,
    runtime_observations: BTreeMap<String, RuntimeCapabilityObservation>,
    next_sequence: u64,
}

impl CapabilityRegistryState {
    fn begin(
        &mut self,
        target: ObservationTarget,
        configuration: Option<ConfigurationIdentity>,
    ) -> Result<CapabilityObservation> {
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .context("capability observation sequence is exhausted")?;
        let sequence = ObservationSequence(self.next_sequence);
        let key = ObservationKey {
            target,
            configuration,
        };
        if !self.latest.contains_key(&key) && self.latest.len() >= MAX_ACTIVE_OBSERVATIONS {
            let oldest = self
                .latest
                .iter()
                .min_by_key(|(_, sequence)| sequence.0)
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                self.latest.remove(&oldest);
            }
        }
        self.latest.insert(key, sequence);
        Ok(CapabilityObservation { key, sequence })
    }

    fn accept(&mut self, observation: &CapabilityObservation) -> bool {
        if self.latest.get(&observation.key) != Some(&observation.sequence) {
            return false;
        }
        self.latest.remove(&observation.key);
        true
    }

    fn abandon(&mut self, observation: &CapabilityObservation) {
        if self.latest.get(&observation.key) == Some(&observation.sequence) {
            self.latest.remove(&observation.key);
        }
    }

    fn track_runtime_operation(
        &mut self,
        operation_id: String,
        lease: RuntimeCapabilityObservation,
    ) {
        if self.latest.get(&lease.observation.key) != Some(&lease.observation.sequence) {
            return;
        }
        if let Some(replaced) = self.runtime_observations.insert(operation_id, lease) {
            self.abandon(&replaced.observation);
        }
        while self.runtime_observations.len() > MAX_ACTIVE_OBSERVATIONS {
            let oldest = self
                .runtime_observations
                .iter()
                .min_by_key(|(_, lease)| lease.observation.sequence.0)
                .map(|(operation_id, _)| operation_id.clone());
            let Some(oldest) = oldest else {
                break;
            };
            if let Some(removed) = self.runtime_observations.remove(&oldest) {
                self.abandon(&removed.observation);
            }
        }
    }

    fn complete_success(&mut self, observation: CapabilityObservation, value: &Value) {
        if !self.accept(&observation) {
            return;
        }
        match observation.key.target {
            ObservationTarget::L1Connectivity => {
                self.insert_source(
                    SourceScope::L1,
                    observation.key.configuration,
                    json!({
                        "health": {
                            "ready": true,
                            "reachable": true,
                            "status": "ready",
                            "detail": "L1 provider call succeeded",
                            "summary": "L1 provider call succeeded",
                        },
                        "probe_facts": [{
                            "key": "blockchain.connection",
                            "ok": true,
                            "value": value,
                            "error": "",
                        }],
                    }),
                );
            }
            ObservationTarget::Source(scope) => {
                self.insert_source(scope, observation.key.configuration, value.clone());
            }
            ObservationTarget::Module(scope) => {
                self.evidence.module_reports.insert(scope, value.clone());
            }
            ObservationTarget::LastKnown(scope) => {
                self.evidence.last_known.remove(&scope);
            }
        }
    }

    fn complete_failure(&mut self, observation: CapabilityObservation, detail: &str) {
        if !self.accept(&observation) {
            return;
        }
        match observation.key.target {
            ObservationTarget::L1Connectivity => self.insert_source(
                SourceScope::L1,
                observation.key.configuration,
                failed_source_report("blockchain.connection", detail),
            ),
            ObservationTarget::Source(scope) => self.insert_source(
                scope,
                observation.key.configuration,
                failed_source_report(scope.key(), detail),
            ),
            ObservationTarget::Module(scope) => {
                self.evidence.module_reports.insert(
                    scope,
                    json!({
                        "module_info": { "ok": false, "error": detail },
                        "health": {
                            "ready": false,
                            "reachable": false,
                            "status": "unavailable",
                            "detail": detail,
                        },
                    }),
                );
            }
            ObservationTarget::LastKnown(scope) => {
                self.evidence.last_known.insert(scope, detail.to_owned());
            }
        }
    }

    fn insert_source(
        &mut self,
        scope: SourceScope,
        configuration: Option<ConfigurationIdentity>,
        value: Value,
    ) {
        let Some(configuration) = configuration else {
            return;
        };
        self.configured_sources.entry(scope).or_default().insert(
            configuration,
            value,
            self.selected_configurations.get(&scope).copied(),
        );
    }

    fn evidence_for_runtime(
        &mut self,
        build_mode: CapabilityBuildMode,
        inputs: &CapabilityRuntimeInputs,
    ) -> CapabilityEvidenceSnapshot {
        let mut evidence = self.evidence.clone();
        evidence.source_reports.clear();
        for scope in [SourceScope::L1, SourceScope::Storage, SourceScope::Delivery] {
            let Some(configuration) = runtime_configuration_identity(scope, build_mode, inputs)
            else {
                self.selected_configurations.remove(&scope);
                continue;
            };
            self.selected_configurations.insert(scope, configuration);
            if let Some(report) = self
                .configured_sources
                .get(&scope)
                .and_then(|reports| reports.get(configuration))
            {
                evidence.source_reports.insert(scope, report.clone());
            }
        }
        evidence
    }
}

#[derive(Debug, Default)]
pub(crate) struct CapabilityRegistry {
    state: Mutex<CapabilityRegistryState>,
}

impl CapabilityRegistry {
    pub(crate) fn begin_observation(
        &self,
        method: &str,
        args: &Value,
    ) -> Result<Option<CapabilityObservation>> {
        self.begin_observation_with_generation(method, args, None)
    }

    pub(crate) fn begin_runtime_observation(
        &self,
        method: &str,
        args: &Value,
        configuration_generation: Option<u64>,
    ) -> Result<Option<CapabilityObservation>> {
        self.begin_observation_with_generation(method, args, configuration_generation)
    }

    fn begin_observation_with_generation(
        &self,
        method: &str,
        args: &Value,
        configuration_generation: Option<u64>,
    ) -> Result<Option<CapabilityObservation>> {
        let Some(target) = observation_target(method) else {
            return Ok(None);
        };
        let configuration = request_configuration_identity(target, args, configuration_generation);
        if target.requires_configuration() && configuration.is_none() {
            return Ok(None);
        }
        self.state
            .lock()
            .map_err(|_| anyhow!("capability evidence lock is poisoned"))?
            .begin(target, configuration)
            .map(Some)
    }

    pub(crate) fn abandon_observation(
        &self,
        observation: Option<CapabilityObservation>,
    ) -> Result<()> {
        let Some(observation) = observation else {
            return Ok(());
        };
        self.state
            .lock()
            .map_err(|_| anyhow!("capability evidence lock is poisoned"))?
            .abandon(&observation);
        Ok(())
    }

    pub(crate) fn track_runtime_operation(
        &self,
        observation: Option<CapabilityObservation>,
        configuration_generation: Option<u64>,
        operation: &Value,
    ) -> Result<()> {
        let Some(observation) = observation else {
            return Ok(());
        };
        let Some(operation_id) = runtime_operation_string(operation, "operationId") else {
            self.abandon_observation(Some(observation))?;
            return Err(anyhow!(
                "runtime operation admission is missing operationId"
            ));
        };
        if !runtime_operation_matches(operation, operation_id, configuration_generation) {
            self.abandon_observation(Some(observation))?;
            return Err(anyhow!(
                "runtime operation admission does not match capability observation"
            ));
        }
        self.state
            .lock()
            .map_err(|_| anyhow!("capability evidence lock is poisoned"))?
            .track_runtime_operation(
                operation_id.to_owned(),
                RuntimeCapabilityObservation {
                    observation,
                    configuration_generation,
                },
            );
        Ok(())
    }

    pub(crate) fn complete_runtime_operation(&self, operation: &Value) -> Result<()> {
        let Some(operation_id) = runtime_operation_string(operation, "operationId") else {
            return Ok(());
        };
        let Some(status) = runtime_operation_string(operation, "status") else {
            return Ok(());
        };
        if matches!(status, "running" | "awaiting_external" | "canceling") {
            return Ok(());
        }
        if !matches!(
            status,
            "completed" | "dispatched" | "failed" | "canceled" | "timed_out"
        ) {
            return Ok(());
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("capability evidence lock is poisoned"))?;
        let Some(lease) = state.runtime_observations.get(operation_id) else {
            return Ok(());
        };
        if !runtime_operation_matches(operation, operation_id, lease.configuration_generation) {
            return Ok(());
        }
        let Some(lease) = state.runtime_observations.remove(operation_id) else {
            return Ok(());
        };
        if status == "completed"
            && let Some(result) = operation.get("result").filter(|value| value.is_object())
        {
            state.complete_success(lease.observation, result);
            return Ok(());
        }
        state.complete_failure(
            lease.observation,
            &runtime_operation_failure_detail(operation, status),
        );
        Ok(())
    }

    pub(crate) fn complete_success(
        &self,
        observation: Option<CapabilityObservation>,
        value: &Value,
    ) -> Result<()> {
        let Some(observation) = observation else {
            return Ok(());
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("capability evidence lock is poisoned"))?;
        state.complete_success(observation, value);
        Ok(())
    }

    pub(crate) fn complete_failure(
        &self,
        observation: Option<CapabilityObservation>,
        detail: &str,
    ) -> Result<()> {
        let Some(observation) = observation else {
            return Ok(());
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("capability evidence lock is poisoned"))?;
        state.complete_failure(observation, detail);
        Ok(())
    }

    pub(crate) fn report(
        &self,
        build_mode: CapabilityBuildMode,
        value: Option<&Value>,
    ) -> Result<CapabilityRegistryReport> {
        let inputs = CapabilityRuntimeInputs::from_value(value);
        let evidence = self
            .state
            .lock()
            .map_err(|_| anyhow!("capability evidence lock is poisoned"))?
            .evidence_for_runtime(build_mode, &inputs);
        let inputs = inputs.with_evidence(evidence);
        Ok(capability_registry_report_with_inputs(build_mode, &inputs))
    }
}

fn runtime_operation_matches(
    operation: &Value,
    operation_id: &str,
    configuration_generation: Option<u64>,
) -> bool {
    runtime_operation_string(operation, "operationId") == Some(operation_id)
        && runtime_operation_string(operation, "domain") == Some("blockchain")
        && runtime_operation_string(operation, "method") == Some("blockchainNode")
        && runtime_operation_configuration_generation(operation) == configuration_generation
}

fn runtime_operation_string<'a>(operation: &'a Value, key: &str) -> Option<&'a str> {
    operation
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn runtime_operation_configuration_generation(operation: &Value) -> Option<u64> {
    operation
        .get("context")
        .and_then(|context| context.get("configurationGeneration"))
        .and_then(Value::as_u64)
}

fn runtime_operation_failure_detail(operation: &Value, status: &str) -> String {
    ["error", "terminalReason"]
        .iter()
        .find_map(|key| runtime_operation_string(operation, key))
        .map_or_else(
            || format!("blockchain operation ended with status `{status}`"),
            str::to_owned,
        )
}

fn request_configuration_identity(
    target: ObservationTarget,
    value: &Value,
    configuration_generation: Option<u64>,
) -> Option<ConfigurationIdentity> {
    let args = Args::new(value.clone()).ok()?;
    let generation = configuration_generation.or_else(|| request_configuration_generation(&args));
    match target {
        ObservationTarget::L1Connectivity => {
            let source = args.source_endpoint(0, "node endpoint").ok()?;
            Some(source_configuration_identity(
                SourceScope::L1,
                source.mode.as_str(),
                Some(source.endpoint),
                None,
                generation,
            ))
        }
        ObservationTarget::Source(SourceScope::Storage) => {
            let inputs = storage_layer::report_inputs(&args).ok()?;
            Some(source_configuration_identity(
                SourceScope::Storage,
                &inputs.source_mode,
                inputs.rest_endpoint.as_deref(),
                inputs.metrics_endpoint.as_deref(),
                generation,
            ))
        }
        ObservationTarget::Source(SourceScope::Delivery) => {
            let inputs = messaging_layer::report_inputs(&args).ok()?;
            Some(source_configuration_identity(
                SourceScope::Delivery,
                &inputs.source_mode,
                inputs.rest_endpoint.as_deref(),
                inputs.metrics_endpoint.as_deref(),
                generation,
            ))
        }
        ObservationTarget::Source(SourceScope::L1)
        | ObservationTarget::Module(_)
        | ObservationTarget::LastKnown(_) => None,
    }
}

fn request_configuration_generation(args: &Args) -> Option<u64> {
    args.iter().find_map(|value| {
        configuration_generation_value(value.get("configuration_generation").or_else(|| {
            value
                .get("observation_context")
                .and_then(|context| context.get("configuration_generation"))
        }))
    })
}

fn configuration_generation_value(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::Number(value)) => value.as_u64(),
        Some(Value::String(value)) => value.trim().parse().ok(),
        _ => None,
    }
}

fn source_configuration_identity(
    scope: SourceScope,
    mode: &str,
    rest_or_rpc_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    generation: Option<u64>,
) -> ConfigurationIdentity {
    let adapter = &source_mode_policy(scope.family(), mode).adapter;
    let endpoint = match adapter.target {
        "rpc_endpoint" | "rest_endpoint" => rest_or_rpc_endpoint.unwrap_or_default(),
        "metrics_endpoint" => metrics_endpoint.unwrap_or_default(),
        _ => "",
    };
    let supplemental_endpoint = if adapter.target == "rest_endpoint" {
        metrics_endpoint.unwrap_or_default()
    } else {
        ""
    };
    ConfigurationIdentity::new(
        scope,
        adapter.connector_id,
        endpoint,
        supplemental_endpoint,
        generation,
    )
}

fn runtime_configuration_identity(
    scope: SourceScope,
    build_mode: CapabilityBuildMode,
    inputs: &CapabilityRuntimeInputs,
) -> Option<ConfigurationIdentity> {
    let connector = inputs.connector_for(build_mode, scope.key());
    let adapter = network_adapter_policy_for_connector(&connector.id)?;
    let endpoint = match adapter.target {
        "rpc_endpoint" | "rest_endpoint" | "metrics_endpoint" => {
            inputs.endpoint_for(scope.key(), &connector)
        }
        _ => String::new(),
    };
    let supplemental_endpoint = if adapter.target == "rest_endpoint" {
        inputs.metrics_endpoint_for(scope.key())
    } else {
        ""
    };
    Some(ConfigurationIdentity::new(
        scope,
        &connector.id,
        &endpoint,
        supplemental_endpoint,
        inputs.configuration_generation_for(scope.key()),
    ))
}

fn failed_source_report(key: &str, detail: &str) -> Value {
    json!({
        "health": {
            "ready": false,
            "reachable": false,
            "status": "unavailable",
            "detail": detail,
            "summary": detail,
        },
        "probe_facts": [{ "key": key, "ok": false, "error": detail }],
    })
}

fn observation_target(method: &str) -> Option<ObservationTarget> {
    match method {
        "blockchainNode" => Some(ObservationTarget::L1Connectivity),
        "storageSourceReport" => Some(ObservationTarget::Source(SourceScope::Storage)),
        "deliverySourceReport" => Some(ObservationTarget::Source(SourceScope::Delivery)),
        "blockchainModuleReport" => Some(ObservationTarget::Module(ModuleScope::Blockchain)),
        "storageReport" => Some(ObservationTarget::Module(ModuleScope::Storage)),
        "deliveryReport" => Some(ObservationTarget::Module(ModuleScope::Delivery)),
        "localNodesStatus" | "localNodesAction" => {
            Some(ObservationTarget::LastKnown(LastKnownScope::LocalNodes))
        }
        "localWalletProfileStatus"
        | "localWalletAccounts"
        | "localWalletCreateAccount"
        | "localWalletSendTransaction"
        | "localWalletCommand"
        | "localWalletSyncPrivate"
        | "localWalletDeployProgram"
        | "localWalletInstructionSubmit" => {
            Some(ObservationTarget::LastKnown(LastKnownScope::Wallet))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result, bail};

    use super::*;

    fn complete_success(
        registry: &CapabilityRegistry,
        method: &str,
        args: &Value,
        value: &Value,
    ) -> Result<()> {
        let observation = registry.begin_observation(method, args)?;
        registry.complete_success(observation, value)
    }

    fn storage_source_args(endpoint: &str, metrics_endpoint: Option<&str>) -> Value {
        let mut inputs = serde_json::Map::new();
        inputs.insert("rest_endpoint".to_owned(), json!(endpoint));
        if let Some(metrics_endpoint) = metrics_endpoint {
            inputs.insert("metrics_endpoint".to_owned(), json!(metrics_endpoint));
        }
        json!([{
            "source_mode": "rest",
            "inputs": inputs,
        }])
    }

    fn delivery_source_args(endpoint: &str) -> Value {
        json!([{
            "source_mode": "rest",
            "inputs": { "rest_endpoint": endpoint },
        }])
    }

    fn with_configuration_generation(mut args: Value, generation: u64) -> Result<Value> {
        let initialization = args
            .as_array_mut()
            .and_then(|values| values.first_mut())
            .and_then(Value::as_object_mut)
            .context("missing source initialization")?;
        initialization.insert("configuration_generation".to_owned(), json!(generation));
        Ok(args)
    }

    fn ready_source_report() -> Value {
        json!({
            "health": { "ready": true, "reachable": true, "status": "ready" },
            "probe_facts": [{ "key": "provider.ready", "ok": true }],
        })
    }

    fn runtime_blockchain_operation(
        operation_id: &str,
        status: &str,
        generation: u64,
        result: Value,
        error: &str,
    ) -> Value {
        json!({
            "operationId": operation_id,
            "domain": "blockchain",
            "method": "blockchainNode",
            "status": status,
            "context": { "configurationGeneration": generation },
            "result": result,
            "error": error,
        })
    }

    fn track_runtime_blockchain_observation(
        registry: &CapabilityRegistry,
        operation_id: &str,
        endpoint: &str,
        generation: u64,
    ) -> Result<()> {
        let observation = registry.begin_runtime_observation(
            "blockchainNode",
            &json!(["rpc", endpoint]),
            Some(generation),
        )?;
        registry.track_runtime_operation(
            observation,
            Some(generation),
            &runtime_blockchain_operation(operation_id, "running", generation, Value::Null, ""),
        )
    }

    fn l1_report(
        registry: &CapabilityRegistry,
        endpoint: &str,
        generation: u64,
    ) -> Result<CapabilityRegistryReport> {
        registry.report(
            CapabilityBuildMode::Standalone,
            Some(&json!({
                "node_url": endpoint,
                "configuration_generations": { "l1": generation },
                "network_connector_config": {
                    "scopes": { "l1": { "connector_id": "direct_l1_rpc" } }
                }
            })),
        )
    }

    fn capability<'a>(
        report: &'a CapabilityRegistryReport,
        key: &str,
    ) -> Result<&'a super::super::CapabilityReport> {
        report
            .capabilities
            .iter()
            .find(|capability| capability.key == key)
            .with_context(|| format!("missing {key} capability"))
    }

    #[test]
    fn registry_observes_provider_results_without_qml_reports() -> Result<()> {
        let registry = CapabilityRegistry::default();
        complete_success(
            &registry,
            "blockchainNode",
            &json!(["rpc", "http://127.0.0.1:8545"]),
            &json!({ "slot": 9 }),
        )?;
        complete_success(
            &registry,
            "storageSourceReport",
            &storage_source_args("http://127.0.0.1:8081", None),
            &ready_source_report(),
        )?;

        let report = serde_json::to_value(registry.report(
            CapabilityBuildMode::Standalone,
            Some(&json!({
                "node_url": "http://127.0.0.1:8545",
                "storage_rest_url": "http://127.0.0.1:8081",
                "storage_mutating_diagnostics_enabled": true,
                "network_connector_config": {
                    "scopes": {
                        "l1": { "connector_id": "direct_l1_rpc" },
                        "storage": { "connector_id": "direct_storage_rest" }
                    }
                }
            })),
        )?)?;
        let capabilities = report
            .get("capabilities")
            .and_then(Value::as_array)
            .context("missing capabilities")?;
        for key in ["l1", "storage"] {
            let capability = capabilities
                .iter()
                .find(|entry| entry.get("key").and_then(Value::as_str) == Some(key))
                .with_context(|| format!("missing {key} capability"))?;
            if capability.get("status").and_then(Value::as_str) != Some("available") {
                bail!("{key} evidence was not applied: {capability}");
            }
        }
        Ok(())
    }

    #[test]
    fn registry_records_typed_failure_evidence() -> Result<()> {
        let registry = CapabilityRegistry::default();
        let observation = registry
            .begin_observation("blockchainNode", &json!(["rpc", "http://127.0.0.1:8545"]))?;
        registry.complete_failure(observation, "connection refused")?;
        let report = registry.report(
            CapabilityBuildMode::Standalone,
            Some(&json!({
                "node_url": "http://127.0.0.1:8545",
                "network_connector_config": {
                    "scopes": { "l1": { "connector_id": "direct_l1_rpc" } }
                }
            })),
        )?;
        let l1 = capability(&report, "l1")?;
        if l1.status != "unavailable"
            || !l1
                .compact_errors
                .iter()
                .any(|error| error.contains("connection refused"))
        {
            bail!("failure evidence was not projected: {l1:?}");
        }
        Ok(())
    }

    #[test]
    fn runtime_terminal_result_commits_l1_generation_zero_only_after_acceptance() -> Result<()> {
        let registry = CapabilityRegistry::default();
        let endpoint = "http://127.0.0.1:8545";
        track_runtime_blockchain_observation(&registry, "l1-zero", endpoint, 0)?;

        if capability(&l1_report(&registry, endpoint, 0)?, "l1")?.status != "loading" {
            bail!("runtime admission committed L1 evidence before terminal result");
        }
        registry.complete_runtime_operation(&runtime_blockchain_operation(
            "l1-zero",
            "completed",
            1,
            json!({ "node": "foreign-generation" }),
            "",
        ))?;
        if capability(&l1_report(&registry, endpoint, 0)?, "l1")?.status != "loading" {
            bail!("foreign runtime generation committed L1 evidence");
        }

        registry.complete_runtime_operation(&runtime_blockchain_operation(
            "l1-zero",
            "completed",
            0,
            json!({ "node": true }),
            "",
        ))?;
        if capability(&l1_report(&registry, endpoint, 0)?, "l1")?.status != "available" {
            bail!("accepted generation-zero terminal result was not projected");
        }
        Ok(())
    }

    #[test]
    fn late_a_completion_cannot_replace_reselected_a_generation() -> Result<()> {
        let registry = CapabilityRegistry::default();
        let endpoint_a = "http://l1-a";
        let endpoint_b = "http://l1-b";
        track_runtime_blockchain_observation(&registry, "a-old", endpoint_a, 0)?;
        track_runtime_blockchain_observation(&registry, "b-current", endpoint_b, 1)?;
        if capability(&l1_report(&registry, endpoint_b, 1)?, "l1")?.status != "loading" {
            bail!("B unexpectedly reused A evidence");
        }
        track_runtime_blockchain_observation(&registry, "a-new", endpoint_a, 2)?;
        if capability(&l1_report(&registry, endpoint_a, 2)?, "l1")?.status != "loading" {
            bail!("reselected A unexpectedly reused its prior generation");
        }

        registry.complete_runtime_operation(&runtime_blockchain_operation(
            "a-new",
            "completed",
            2,
            json!({ "node": "new-a" }),
            "",
        ))?;
        registry.complete_runtime_operation(&runtime_blockchain_operation(
            "a-old",
            "failed",
            0,
            Value::Null,
            "late old A failure",
        ))?;

        let current = capability(&l1_report(&registry, endpoint_a, 2)?, "l1")?.clone();
        if current.status != "available"
            || current
                .compact_errors
                .iter()
                .any(|error| error.contains("late old A failure"))
        {
            bail!("late old A completion replaced reselected A evidence: {current:?}");
        }
        Ok(())
    }

    #[test]
    fn registry_hides_evidence_from_a_different_source_configuration() -> Result<()> {
        let registry = CapabilityRegistry::default();
        complete_success(
            &registry,
            "storageSourceReport",
            &storage_source_args("http://old-storage", None),
            &json!({
                "health": { "ready": false, "status": "unavailable" },
                "probe_facts": [{ "key": "storage.old", "ok": false, "error": "old source failed" }],
            }),
        )?;

        let report = registry.report(
            CapabilityBuildMode::Standalone,
            Some(&json!({
                "storage_rest_url": "http://new-storage",
                "storage_mutating_diagnostics_enabled": true,
                "network_connector_config": {
                    "scopes": { "storage": { "connector_id": "direct_storage_rest" } }
                }
            })),
        )?;
        let storage = capability(&report, "storage")?;
        let diagnostics = capability(&report, "diagnostics")?;

        if storage.status != "loading"
            || storage
                .compact_errors
                .iter()
                .any(|error| error.contains("old source failed"))
            || diagnostics
                .compact_errors
                .iter()
                .any(|error| error.contains("old source failed"))
        {
            bail!("old source evidence leaked into current configuration: {storage:?}");
        }
        Ok(())
    }

    #[test]
    fn newer_observation_rejects_late_completion_for_same_scope() -> Result<()> {
        let registry = CapabilityRegistry::default();
        let args = storage_source_args("http://storage", None);
        let older = registry.begin_observation("storageSourceReport", &args)?;
        let newer = registry.begin_observation("storageSourceReport", &args)?;
        registry.complete_success(newer, &ready_source_report())?;
        registry.complete_failure(older, "late old failure")?;

        let report = registry.report(
            CapabilityBuildMode::Standalone,
            Some(&json!({
                "storage_rest_url": "http://storage",
                "storage_mutating_diagnostics_enabled": true,
                "network_connector_config": {
                    "scopes": { "storage": { "connector_id": "direct_storage_rest" } }
                }
            })),
        )?;
        let storage = capability(&report, "storage")?;

        if storage.status != "available"
            || storage
                .compact_errors
                .iter()
                .any(|error| error.contains("late old failure"))
        {
            bail!("late completion replaced newer evidence: {storage:?}");
        }
        Ok(())
    }

    #[test]
    fn stale_configuration_completion_preserves_selected_configuration_evidence() -> Result<()> {
        let registry = CapabilityRegistry::default();
        complete_success(
            &registry,
            "storageSourceReport",
            &storage_source_args("http://current-storage", None),
            &ready_source_report(),
        )?;
        let current_inputs = json!({
            "storage_rest_url": "http://current-storage",
            "storage_mutating_diagnostics_enabled": true,
            "network_connector_config": {
                "scopes": { "storage": { "connector_id": "direct_storage_rest" } }
            }
        });
        let current = registry.report(CapabilityBuildMode::Standalone, Some(&current_inputs))?;
        if capability(&current, "storage")?.status != "available" {
            bail!("current source evidence was not selected");
        }

        let stale = registry.begin_observation(
            "storageSourceReport",
            &storage_source_args("http://stale-storage", None),
        )?;
        registry.complete_failure(stale, "stale source failed")?;
        let current = registry.report(CapabilityBuildMode::Standalone, Some(&current_inputs))?;
        let storage = capability(&current, "storage")?;

        if storage.status != "available"
            || storage
                .compact_errors
                .iter()
                .any(|error| error.contains("stale source failed"))
        {
            bail!("stale source completion replaced selected evidence: {storage:?}");
        }
        Ok(())
    }

    #[test]
    fn prior_generation_cannot_surface_after_same_configuration_is_reselected() -> Result<()> {
        let registry = CapabilityRegistry::default();
        let generation_one =
            with_configuration_generation(storage_source_args("http://storage", None), 1)?;
        complete_success(
            &registry,
            "storageSourceReport",
            &generation_one,
            &ready_source_report(),
        )?;

        let report_for_generation = |generation| {
            registry.report(
                CapabilityBuildMode::Standalone,
                Some(&json!({
                    "storage_rest_url": "http://storage",
                    "storage_mutating_diagnostics_enabled": true,
                    "configuration_generations": { "storage": generation },
                    "network_connector_config": {
                        "scopes": { "storage": { "connector_id": "direct_storage_rest" } }
                    }
                })),
            )
        };

        if capability(&report_for_generation(1)?, "storage")?.status != "available" {
            bail!("matching generation did not expose evidence");
        }
        for generation in [2, 3] {
            let report = report_for_generation(generation)?;
            let storage = capability(&report, "storage")?;
            if storage.status != "loading" {
                bail!("generation {generation} reused stale evidence: {storage:?}");
            }
        }
        Ok(())
    }

    #[test]
    fn observations_are_ordered_independently_per_source_scope() -> Result<()> {
        let registry = CapabilityRegistry::default();
        let storage = registry.begin_observation(
            "storageSourceReport",
            &storage_source_args("http://storage", None),
        )?;
        let delivery = registry.begin_observation(
            "deliverySourceReport",
            &delivery_source_args("http://delivery"),
        )?;
        registry.complete_success(storage, &ready_source_report())?;
        registry.complete_success(delivery, &ready_source_report())?;

        let report = registry.report(
            CapabilityBuildMode::Standalone,
            Some(&json!({
                "storage_rest_url": "http://storage",
                "messaging_rest_url": "http://delivery",
                "storage_mutating_diagnostics_enabled": true,
                "messaging_mutating_diagnostics_enabled": true,
                "network_connector_config": {
                    "scopes": {
                        "storage": { "connector_id": "direct_storage_rest" },
                        "delivery": { "connector_id": "direct_delivery_rest" }
                    }
                }
            })),
        )?;

        for (key, expected_status) in [("storage", "available"), ("delivery", "degraded")] {
            let entry = capability(&report, key)?;
            if entry.status != expected_status {
                bail!("{key} observation was invalidated by another scope: {entry:?}");
            }
        }
        Ok(())
    }

    #[test]
    fn supplemental_metrics_endpoint_participates_in_source_identity() -> Result<()> {
        let registry = CapabilityRegistry::default();
        complete_success(
            &registry,
            "storageSourceReport",
            &storage_source_args("http://storage", Some("http://old-metrics")),
            &json!({
                "health": { "ready": false, "status": "unavailable" },
                "probe_facts": [{ "key": "storage.metrics", "ok": false, "error": "old metrics failed" }],
            }),
        )?;

        let report = registry.report(
            CapabilityBuildMode::Standalone,
            Some(&json!({
                "storage_rest_url": "http://storage",
                "storage_metrics_url": "http://new-metrics",
                "storage_mutating_diagnostics_enabled": true,
                "network_connector_config": {
                    "scopes": { "storage": { "connector_id": "direct_storage_rest" } }
                }
            })),
        )?;
        let storage = capability(&report, "storage")?;

        if storage.status != "loading"
            || storage
                .compact_errors
                .iter()
                .any(|error| error.contains("old metrics failed"))
        {
            bail!("old supplemental endpoint evidence leaked: {storage:?}");
        }
        Ok(())
    }

    #[test]
    fn configured_source_history_is_bounded_without_evicting_selected_identity() {
        let protected = ConfigurationIdentity::new(
            SourceScope::Storage,
            "direct_storage_rest",
            "http://selected",
            "",
            None,
        );
        let mut reports = ConfiguredSourceReports::default();
        reports.insert(protected, json!({ "selected": true }), Some(protected));

        for index in 0..(MAX_SOURCE_CONFIGURATIONS + 4) {
            let endpoint = format!("http://stale-{index}");
            let configuration = ConfigurationIdentity::new(
                SourceScope::Storage,
                "direct_storage_rest",
                &endpoint,
                "",
                None,
            );
            reports.insert(configuration, json!({ "index": index }), Some(protected));
        }

        assert_eq!(reports.entries.len(), MAX_SOURCE_CONFIGURATIONS);
        assert!(reports.get(protected).is_some());
    }

    #[test]
    fn active_observation_leases_are_bounded() -> Result<()> {
        let mut state = CapabilityRegistryState::default();
        for generation in 0..(MAX_ACTIVE_OBSERVATIONS as u64 + 4) {
            let configuration = ConfigurationIdentity::new(
                SourceScope::Storage,
                "direct_storage_rest",
                "http://storage",
                "",
                Some(generation),
            );
            state.begin(
                ObservationTarget::Source(SourceScope::Storage),
                Some(configuration),
            )?;
        }

        assert_eq!(state.latest.len(), MAX_ACTIVE_OBSERVATIONS);
        Ok(())
    }

    #[test]
    fn runtime_operation_correlations_are_bounded() -> Result<()> {
        let mut state = CapabilityRegistryState::default();
        for generation in 0..(MAX_ACTIVE_OBSERVATIONS as u64 + 4) {
            let configuration = ConfigurationIdentity::new(
                SourceScope::L1,
                "direct_l1_rpc",
                "http://l1",
                "",
                Some(generation),
            );
            let observation =
                state.begin(ObservationTarget::L1Connectivity, Some(configuration))?;
            state.track_runtime_operation(
                format!("l1-{generation}"),
                RuntimeCapabilityObservation {
                    observation,
                    configuration_generation: Some(generation),
                },
            );
        }

        assert_eq!(state.runtime_observations.len(), MAX_ACTIVE_OBSERVATIONS);
        assert_eq!(state.latest.len(), MAX_ACTIVE_OBSERVATIONS);
        Ok(())
    }

    #[test]
    fn configuration_identity_canonicalizes_equivalent_url_spellings() {
        let first = ConfigurationIdentity::new(
            SourceScope::Storage,
            "direct_storage_rest",
            "HTTP://EXAMPLE.COM:80",
            "",
            Some(1),
        );
        let second = ConfigurationIdentity::new(
            SourceScope::Storage,
            "direct_storage_rest",
            "http://example.com/",
            "",
            Some(1),
        );

        assert_eq!(first, second);
    }
}
