mod config;
pub(crate) mod indexer;
pub(crate) mod layer;
mod monitor;
mod probe;
pub(crate) mod sequencer;
mod store;

pub use config::{
    ChannelSourceConfig, ChannelSourceConfigApplyRequest, ChannelSourceConfigMutation,
    ChannelSourceRole, ChannelSourceTarget, ConfiguredIndexerSource, ConfiguredSequencerSource,
    PersistedSequencerAttestation, SequencerAttestationReceipt,
};

pub use monitor::{
    ChannelSourceBindingState, ChannelSourceBlockObservation, ChannelSourceCurrentFailure,
    ChannelSourceHealthState, ChannelSourceLastGood, ChannelSourceMonitor,
    ChannelSourceMonitorError, ChannelSourceMonitorSnapshot, ChannelSourceObservation,
    ChannelSourceObservationSet, ChannelSourceProbeStage,
};
pub use probe::ChannelSourceFailureKind;
pub use store::{
    apply_channel_source_config, load_channel_source_configs, record_sequencer_attestation,
};

pub(crate) use config::normalize_channel_id;
pub(crate) use probe::attest_sequencer_target;
pub(crate) use store::{
    apply_channel_source_config_with_attestation, load_settings_state,
    normalized_settings_state_from_backup, rebind_channel_source_configs, save_user_settings_state,
    settings_state_from_stored,
};
