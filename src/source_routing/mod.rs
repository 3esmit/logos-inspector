mod adapter;
pub mod channel_sources;
mod core;
mod delivery;
mod network_profiles;
mod policy;
mod selection;
mod shared;
mod storage;

pub use adapter::{
    AdapterConnectionType, AdapterInputPolicy, SourceAdapterPolicy, SourceModePolicy,
};
pub(crate) use adapter::{ManagedModuleCallSpec, ManagedNodeAction};
pub(crate) use channel_sources::layer as execution_zone_layer;
#[cfg(test)]
pub(crate) use core::adapters::BLOCKCHAIN_MODULE;
pub(crate) use core::layer as bedrock_layer;
pub(crate) use delivery::delivery_module_probe_plan;
pub use delivery::delivery_source_report;
pub(crate) use delivery::layer as messaging_layer;
pub use network_profiles::{
    CUSTOM_NETWORK_PROFILE, DEFAULT_NETWORK_PROFILE, NetworkEndpoints, NetworkProfile,
    infer_network_profile, network_profiles, resolve_network_endpoints,
};
pub use policy::{
    CoreEndpointMode, CoreSourceMode, DEFAULT_DELIVERY_METRICS_ENDPOINT,
    DEFAULT_DELIVERY_REST_ENDPOINT, DEFAULT_NODE_ENDPOINT, DEFAULT_STORAGE_METRICS_ENDPOINT,
    DEFAULT_STORAGE_REST_ENDPOINT, DeliverySourceMode, DeliverySourceReportKind,
    SourceCapabilityKey, SourceFamily, SourceModeFamilies, SourcePolicyDefaults,
    SourcePolicyReport, SourceProbeKey, StorageSourceMode, StorageSourceReportKind,
    default_endpoint_for_domain, default_source_mode_for_domain, effective_source_mode,
    normalized_core_source_mode, normalized_source_mode, source_mode_is_token, source_mode_policy,
    source_policy_report,
};
pub(crate) use policy::{capability_provider_mode_policies, network_adapter_policy_for_connector};
pub(crate) use selection::{
    DeliveryStoreQuery, SourceArgsNormalization, SourceEndpoint, delivery_rest_source,
    normalized_source_args, require_mutating_diagnostics, storage_rest_source,
};
pub use shared::{
    SourceCapabilityFact, SourceFacts, SourceHealthFacts, SourceHealthStatus, SourceProbeFact,
    SourceReport,
};
pub(crate) use storage::layer as storage_layer;
pub(crate) use storage::storage_module_probe_plan;
pub use storage::storage_source_report;
