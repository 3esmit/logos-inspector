use anyhow::{Result, bail};

use super::model::{
    LocalDevnetListReport, LocalNodeActionRequest, LocalNodeOperationReport, LocalNodeReport,
    LocalNodesState, NodeAction,
};
use super::{
    CONFIRMATION_TOKEN, LocalNodeStore, action_allowed, delete_network, load_network, new_network,
    node_install, node_purge, node_start, node_stop, node_uninstall, normalized_profile,
    report_for_state, reset_network,
};

#[derive(Debug, Clone)]
pub(super) struct LocalNodeActionEngine {
    store: LocalNodeStore,
}

impl LocalNodeActionEngine {
    pub(super) fn system() -> Result<Self> {
        Ok(Self {
            store: LocalNodeStore::system()?,
        })
    }

    pub(super) fn status(&self, profile: &str) -> Result<LocalNodeReport> {
        let state = self.store.load()?;
        Ok(report_for_state(profile, &state))
    }

    pub(super) fn devnets(&self, profile: &str) -> Result<LocalDevnetListReport> {
        let state = self.store.load()?;
        Ok(LocalDevnetListReport {
            profile: normalized_profile(profile).to_owned(),
            active_devnet: state.active_devnet.clone(),
            workspace_root: state.managed_workspace_root.clone(),
            devnets: state.devnets.clone(),
        })
    }

    pub(super) fn apply(
        &self,
        profile: &str,
        request: LocalNodeActionRequest,
        confirmation: Option<&str>,
    ) -> Result<LocalNodeReport> {
        if confirmation != Some(CONFIRMATION_TOKEN) {
            bail!("local node action requires explicit confirmation");
        }

        let mut state = self.store.load()?;
        let normalized_profile = normalized_profile(profile);
        self.validate_request(normalized_profile, &request, &state)?;

        let operation = dispatch_action(&mut state, normalized_profile, &request);
        state.push_operation(operation);
        self.store.save(&state)?;
        Ok(report_for_state(profile, &state))
    }

    fn validate_request(
        &self,
        normalized_profile: &str,
        request: &LocalNodeActionRequest,
        state: &LocalNodesState,
    ) -> Result<()> {
        if !action_allowed(
            normalized_profile,
            request.action,
            request.node,
            state.active_devnet.is_some(),
        ) {
            bail!(
                "{} is not available for profile `{normalized_profile}`",
                request.action.label()
            );
        }

        if request.action.is_network_action() && normalized_profile != "local" {
            bail!("local network actions require the local network profile");
        }

        Ok(())
    }
}

fn dispatch_action(
    state: &mut LocalNodesState,
    normalized_profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    match request.action {
        NodeAction::NewNetwork => new_network(state, request),
        NodeAction::LoadNetwork => load_network(state, request),
        NodeAction::DeleteNetwork => delete_network(state, request),
        NodeAction::ResetNetwork => reset_network(state, request),
        NodeAction::Install => node_install(state, normalized_profile, request),
        NodeAction::Uninstall => node_uninstall(state, normalized_profile, request),
        NodeAction::Start => node_start(state, normalized_profile, request),
        NodeAction::Stop => node_stop(state, normalized_profile, request),
        NodeAction::Purge => node_purge(state, normalized_profile, request),
    }
}
