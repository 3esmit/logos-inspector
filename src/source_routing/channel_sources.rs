mod config;
pub(crate) mod layer;
mod monitor;
mod probe;
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
    rebind_channel_source_configs, restore_settings_state_from_backup, save_user_settings_state,
};
