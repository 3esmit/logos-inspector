use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use crate::support::args::Args;

use crate::modules::logos_core::LogoscoreCliRuntime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterConnectionType {
    Module,
    Rpc,
    Rest,
    Metrics,
    NetworkMonitor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AdapterInputPolicy {
    pub key: &'static str,
    pub label: &'static str,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SourceAdapterPolicy {
    pub connector_id: &'static str,
    pub connection_type: AdapterConnectionType,
    pub target: &'static str,
    pub module_id: Option<&'static str>,
    pub inputs: &'static [AdapterInputPolicy],
    pub capabilities: &'static [&'static str],
    pub supports_cid_probe: bool,
    pub supports_mutating_diagnostics: bool,
    #[serde(skip)]
    pub capability_scopes: &'static [&'static str],
    #[serde(skip)]
    pub endpoint_role: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceModePolicy {
    pub key: &'static str,
    pub aliases: &'static [&'static str],
    pub effective: &'static str,
    pub label_key: &'static str,
    pub label: &'static str,
    pub source_label: &'static str,
    pub summary: &'static str,
    pub implemented: bool,
    pub adapter: SourceAdapterPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct AdapterInitialization {
    #[serde(default)]
    source_mode: String,
    #[serde(default)]
    inputs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct NodeOperationRequest {
    adapter: Value,
    #[serde(default)]
    payload: Value,
    #[serde(default)]
    mutating_enabled: bool,
}

impl NodeOperationRequest {
    pub(crate) fn from_bridge_args(args: &Args) -> Result<Self> {
        let value = args
            .value(0)
            .context("node operation request is required")?;
        if args.iter().count() != 1 {
            bail!("node operation accepts one structured request")
        }
        Self::from_value(value)
    }

    pub(crate) fn from_value(value: &Value) -> Result<Self> {
        serde_json::from_value(value.clone()).context("node operation request must be an object")
    }

    #[must_use]
    pub(crate) fn adapter(&self) -> &Value {
        &self.adapter
    }

    pub(crate) fn payload<T>(&self, label: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        serde_json::from_value(self.payload.clone())
            .with_context(|| format!("{label} payload is invalid"))
    }

    #[must_use]
    pub(crate) const fn mutating_enabled(&self) -> bool {
        self.mutating_enabled
    }

    pub(crate) fn require_mutating(&self, label: &str) -> Result<()> {
        if self.mutating_enabled {
            return Ok(());
        }
        bail!("{label} requires mutating diagnostics to be enabled")
    }

    #[must_use]
    pub(crate) fn source_mode(&self) -> &str {
        self.adapter
            .get("source_mode")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("direct")
    }

    #[must_use]
    pub(crate) fn input(&self, key: &str) -> Option<&str> {
        self.adapter
            .get("inputs")
            .and_then(|inputs| inputs.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagedNodeAction {
    Initialize,
    Start,
    Stop,
    Destroy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedModuleCallSpec {
    pub(crate) method: &'static str,
    pub(crate) signature: &'static str,
    pub(crate) args: Vec<String>,
}

impl ManagedModuleCallSpec {
    #[must_use]
    pub(crate) fn new(method: &'static str, signature: &'static str, args: Vec<String>) -> Self {
        Self {
            method,
            signature,
            args,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedLifecycleOutcome {
    pub(crate) success: bool,
    pub(crate) detail: String,
}

type EnsureManagedModule = fn(&LogoscoreCliRuntime) -> Result<()>;
type CallManagedModule = fn(&LogoscoreCliRuntime, &str, &str, &[String]) -> Result<Value>;
type ManagedCallSpecBuilder = fn(ManagedNodeAction, &str) -> Option<ManagedModuleCallSpec>;
type ManagedLifecycleEvent = fn(ManagedNodeAction) -> Option<&'static str>;
type ManagedLifecycleDecoder = fn(&Map<String, Value>) -> Result<ManagedLifecycleOutcome>;

#[derive(Debug)]
pub(crate) struct ManagedNodeContract {
    module_id: &'static str,
    ensure_module: EnsureManagedModule,
    call_module: CallManagedModule,
    call_spec: ManagedCallSpecBuilder,
    lifecycle_event: Option<ManagedLifecycleEvent>,
    lifecycle_decoder: Option<ManagedLifecycleDecoder>,
}

impl ManagedNodeContract {
    #[must_use]
    pub(crate) const fn new(
        module_id: &'static str,
        ensure_module: EnsureManagedModule,
        call_module: CallManagedModule,
        call_spec: ManagedCallSpecBuilder,
        lifecycle_event: Option<ManagedLifecycleEvent>,
        lifecycle_decoder: Option<ManagedLifecycleDecoder>,
    ) -> Self {
        Self {
            module_id,
            ensure_module,
            call_module,
            call_spec,
            lifecycle_event,
            lifecycle_decoder,
        }
    }

    #[must_use]
    pub(crate) const fn module_id(&self) -> &'static str {
        self.module_id
    }

    pub(crate) fn ensure_loaded(&self, runtime: &LogoscoreCliRuntime) -> Result<()> {
        (self.ensure_module)(runtime)
    }

    pub(crate) fn call(
        &self,
        runtime: &LogoscoreCliRuntime,
        spec: &ManagedModuleCallSpec,
    ) -> Result<Value> {
        (self.call_module)(runtime, spec.method, spec.signature, &spec.args)
    }

    #[must_use]
    pub(crate) fn call_spec(
        &self,
        action: ManagedNodeAction,
        config_path: &str,
    ) -> Option<ManagedModuleCallSpec> {
        (self.call_spec)(action, config_path)
    }

    #[must_use]
    pub(crate) fn lifecycle_event(&self, action: ManagedNodeAction) -> Option<&'static str> {
        self.lifecycle_event.and_then(|event| event(action))
    }

    pub(crate) fn decode_lifecycle_event(
        &self,
        data: &Map<String, Value>,
    ) -> Result<ManagedLifecycleOutcome> {
        self.lifecycle_decoder
            .context("managed module has no lifecycle event decoder")?(data)
    }
}

impl AdapterInitialization {
    pub(crate) fn parse(
        value: &Value,
        modes: &'static [SourceModePolicy],
        default_mode: &str,
    ) -> Result<Self> {
        let mut initialization: Self = serde_json::from_value(value.clone())
            .context("adapter initialization must be an object")?;
        let requested_mode = if initialization.source_mode.trim().is_empty() {
            default_mode
        } else {
            initialization.source_mode.trim()
        };
        let mode = mode_for_token(modes, requested_mode)
            .with_context(|| format!("unsupported source mode `{requested_mode}`"))?;
        initialization.source_mode = mode.key.to_owned();
        initialization.normalize_inputs(mode)?;
        Ok(initialization)
    }

    #[must_use]
    pub(crate) fn source_mode(&self) -> &str {
        &self.source_mode
    }

    #[must_use]
    pub(crate) fn input(&self, key: &str) -> Option<&str> {
        self.inputs
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    fn normalize_inputs(&mut self, mode: &SourceModePolicy) -> Result<()> {
        for key in self.inputs.keys() {
            if !mode.adapter.inputs.iter().any(|input| input.key == key) {
                bail!(
                    "adapter `{}` does not accept input `{key}`",
                    mode.adapter.connector_id
                );
            }
        }
        for input in mode.adapter.inputs {
            if input.required && self.input(input.key).is_none() {
                bail!("{} is required", input.label);
            }
        }
        self.inputs.retain(|_, value| !value.trim().is_empty());
        Ok(())
    }
}

#[must_use]
pub(crate) fn adapter_for_connector(
    mode_families: &[&'static [SourceModePolicy]],
    connector_id: &str,
) -> Option<&'static SourceAdapterPolicy> {
    mode_families
        .iter()
        .flat_map(|modes| modes.iter())
        .find(|mode| mode.adapter.connector_id == connector_id)
        .map(|mode| &mode.adapter)
}

fn mode_for_token(
    modes: &'static [SourceModePolicy],
    value: &str,
) -> Option<&'static SourceModePolicy> {
    let value = value.trim().to_ascii_lowercase();
    modes
        .iter()
        .find(|mode| mode.key == value || mode.aliases.contains(&value.as_str()))
}

#[cfg(test)]
pub(crate) mod contract_tests {
    use std::collections::BTreeSet;

    use anyhow::Context as _;
    use serde_json::{Value, json};

    use super::{
        AdapterConnectionType, AdapterInitialization, ManagedModuleCallSpec, ManagedNodeAction,
        ManagedNodeContract, SourceAdapterPolicy, SourceModePolicy,
    };

    const MANAGED_ACTIONS: &[ManagedNodeAction] = &[
        ManagedNodeAction::Initialize,
        ManagedNodeAction::Start,
        ManagedNodeAction::Stop,
        ManagedNodeAction::Destroy,
    ];

    pub(crate) fn assert_layer_contract(key: &str, modes: &'static [SourceModePolicy]) {
        assert!(modes.len() >= 2, "{} must expose a real adapter seam", key);

        let mut mode_keys = BTreeSet::new();
        let mut connector_ids = BTreeSet::new();
        for mode in modes {
            assert!(mode_keys.insert(mode.key), "duplicate mode `{}`", mode.key);
            assert!(
                connector_ids.insert(mode.adapter.connector_id),
                "duplicate connector `{}`",
                mode.adapter.connector_id
            );
            assert_adapter_contract(&mode.adapter);
            if mode.implemented {
                assert!(
                    !mode.adapter.capabilities.is_empty(),
                    "implemented adapter `{}` must declare capabilities",
                    mode.adapter.connector_id
                );
            }
            assert_initialization_contract(mode, modes);
        }
    }

    pub(crate) fn assert_managed_module_contract(
        key: &str,
        contract: &'static ManagedNodeContract,
        supported_actions: &[ManagedNodeAction],
    ) {
        assert!(
            !contract.module_id().trim().is_empty(),
            "{key} module id is empty"
        );
        for action in MANAGED_ACTIONS {
            let spec = contract.call_spec(*action, "/tmp/node.json");
            assert_eq!(
                spec.is_some(),
                supported_actions.contains(action),
                "{key} action support mismatch for {action:?}"
            );
            if let Some(spec) = spec {
                assert_call_signature(key, &spec);
            }
        }
    }

    pub(crate) fn assert_managed_lifecycle_behavior(
        key: &str,
        contract: &'static ManagedNodeContract,
        action: ManagedNodeAction,
        expected_event: &str,
        data: Value,
        expected_success: bool,
        expected_detail: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            contract.lifecycle_event(action) == Some(expected_event),
            "{key} lifecycle event drift for {action:?}"
        );
        let data = data
            .as_object()
            .context("managed lifecycle fixture must be an object")?;
        let outcome = contract.decode_lifecycle_event(data)?;
        anyhow::ensure!(
            outcome.success == expected_success && outcome.detail == expected_detail,
            "{key} lifecycle payload decoding drift: {outcome:?}"
        );
        Ok(())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum EndpointAdapterBehavior {
        Module {
            module_id: &'static str,
        },
        Endpoint {
            connection_type: AdapterConnectionType,
            endpoint: String,
            metrics_endpoint: Option<String>,
        },
        Unsupported,
    }

    pub(crate) fn assert_endpoint_adapter_contract(
        key: &str,
        modes: &'static [SourceModePolicy],
        select: impl Fn(&str, Option<&str>, Option<&str>) -> EndpointAdapterBehavior,
    ) {
        const REST_ENDPOINT: &str = "http://rest-adapter";
        const METRICS_ENDPOINT: &str = "http://metrics-adapter";

        for mode in modes {
            let actual = select(mode.key, Some(REST_ENDPOINT), Some(METRICS_ENDPOINT));
            let expected = match mode.adapter.connection_type {
                AdapterConnectionType::Module => EndpointAdapterBehavior::Module {
                    module_id: mode.adapter.module_id.unwrap_or_default(),
                },
                AdapterConnectionType::Rest | AdapterConnectionType::NetworkMonitor => {
                    EndpointAdapterBehavior::Endpoint {
                        connection_type: mode.adapter.connection_type,
                        endpoint: REST_ENDPOINT.to_owned(),
                        metrics_endpoint: mode
                            .adapter
                            .inputs
                            .iter()
                            .any(|input| input.key == "metrics_endpoint")
                            .then(|| METRICS_ENDPOINT.to_owned()),
                    }
                }
                AdapterConnectionType::Metrics => EndpointAdapterBehavior::Endpoint {
                    connection_type: AdapterConnectionType::Metrics,
                    endpoint: METRICS_ENDPOINT.to_owned(),
                    metrics_endpoint: None,
                },
                AdapterConnectionType::Rpc => EndpointAdapterBehavior::Endpoint {
                    connection_type: AdapterConnectionType::Rpc,
                    endpoint: REST_ENDPOINT.to_owned(),
                    metrics_endpoint: None,
                },
            };
            assert_eq!(actual, expected, "{key} `{}` selection drift", mode.key);
        }

        assert_eq!(
            select(
                "unsupported-adapter",
                Some(REST_ENDPOINT),
                Some(METRICS_ENDPOINT)
            ),
            EndpointAdapterBehavior::Unsupported,
            "{key} unsupported adapter must remain explicit"
        );
    }

    fn assert_call_signature(key: &str, spec: &ManagedModuleCallSpec) {
        assert!(!spec.method.is_empty(), "{key} method is empty");
        assert!(
            spec.signature.contains('('),
            "{key} signature has no opening parenthesis"
        );
        let Some((signature_method, parameters)) = spec.signature.split_once('(') else {
            return;
        };
        assert_eq!(
            signature_method, spec.method,
            "{key} method/signature drift"
        );
        assert!(
            parameters.ends_with(')'),
            "{key} signature has no closing parenthesis"
        );
        let Some(parameters) = parameters.strip_suffix(')') else {
            return;
        };
        let parameter_count = if parameters.is_empty() {
            0
        } else {
            parameters.split(',').count()
        };
        assert_eq!(
            parameter_count,
            spec.args.len(),
            "{key} call argument/signature arity drift"
        );
    }

    fn assert_initialization_contract(mode: &SourceModePolicy, modes: &'static [SourceModePolicy]) {
        let inputs = mode
            .adapter
            .inputs
            .iter()
            .map(|input| {
                (
                    input.key.to_owned(),
                    Value::String(format!("{}-value", input.key)),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let value = json!({
            "source_mode": mode.key,
            "inputs": inputs,
        });
        let result = AdapterInitialization::parse(&value, modes, mode.key);
        assert!(
            result.is_ok(),
            "adapter `{}` rejected valid inputs: {result:?}",
            mode.key
        );
        let Ok(initialization) = result else {
            return;
        };
        assert_eq!(initialization.source_mode(), mode.key);

        let unknown = json!({
            "source_mode": mode.key,
            "inputs": { "unknown_input": "value" },
        });
        assert!(AdapterInitialization::parse(&unknown, modes, mode.key).is_err());

        if mode.adapter.inputs.iter().any(|input| input.required) {
            let missing = json!({ "source_mode": mode.key, "inputs": {} });
            assert!(AdapterInitialization::parse(&missing, modes, mode.key).is_err());
        }
    }

    fn assert_adapter_contract(adapter: &SourceAdapterPolicy) {
        let keys: Vec<&str> = adapter.inputs.iter().map(|input| input.key).collect();
        let required: Vec<&str> = adapter
            .inputs
            .iter()
            .filter(|input| input.required)
            .map(|input| input.key)
            .collect();
        let unique: BTreeSet<&str> = keys.iter().copied().collect();
        assert_eq!(unique.len(), keys.len(), "duplicate adapter input key");

        match adapter.connection_type {
            AdapterConnectionType::Module => {
                assert!(keys.is_empty(), "module adapters take no user input");
                assert_eq!(adapter.target, "module");
                assert!(adapter.module_id.is_some(), "module id is layer-owned");
            }
            AdapterConnectionType::Rpc => {
                assert_eq!(keys, ["rpc_endpoint"]);
                assert_eq!(required, ["rpc_endpoint"]);
                assert_eq!(adapter.target, "rpc_endpoint");
                assert!(adapter.module_id.is_none());
            }
            AdapterConnectionType::Rest | AdapterConnectionType::NetworkMonitor => {
                assert_eq!(keys.first().copied(), Some("rest_endpoint"));
                assert_eq!(required, ["rest_endpoint"]);
                assert!(keys == ["rest_endpoint"] || keys == ["rest_endpoint", "metrics_endpoint"]);
                assert_eq!(adapter.target, "rest_endpoint");
                assert!(adapter.module_id.is_none());
            }
            AdapterConnectionType::Metrics => {
                assert_eq!(keys, ["metrics_endpoint"]);
                assert_eq!(required, ["metrics_endpoint"]);
                assert_eq!(adapter.target, "metrics_endpoint");
                assert!(adapter.module_id.is_none());
            }
        }
    }
}
