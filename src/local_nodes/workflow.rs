use anyhow::{Result, bail};

use super::adapters::{adapter_for, adapters_for_profile};
use super::model::{LocalNodeActionRequest, LocalNodesState, NodeAction, NodeKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LocalNodeWorkflow {
    profile: &'static str,
    has_active_devnet: bool,
}

impl LocalNodeWorkflow {
    #[must_use]
    pub(super) fn new(profile: &str, has_active_devnet: bool) -> Self {
        Self {
            profile: normalized_profile(profile),
            has_active_devnet,
        }
    }

    #[must_use]
    pub(super) fn for_state(profile: &str, state: &LocalNodesState) -> Self {
        Self::new(profile, state.active_devnet.is_some())
    }

    #[must_use]
    pub(super) fn profile(self) -> &'static str {
        self.profile
    }

    #[must_use]
    pub(super) fn node_set(self) -> Vec<NodeKind> {
        node_set_for_profile(self.profile)
    }

    #[must_use]
    pub(super) fn network_actions(self) -> Vec<NodeAction> {
        available_actions_for(self.profile, None, self.has_active_devnet)
    }

    #[must_use]
    pub(super) fn node_actions(self, kind: NodeKind) -> Vec<NodeAction> {
        available_actions_for(self.profile, Some(kind), self.has_active_devnet)
    }

    pub(super) fn validate_request(self, request: &LocalNodeActionRequest) -> Result<()> {
        if !self.action_allowed(request.action, request.node) {
            bail!(
                "{} is not available for profile `{}`",
                request.action.label(),
                self.profile
            );
        }

        if request.action.is_network_action() && self.profile != "local" {
            bail!("local devnet actions require local operations mode");
        }

        Ok(())
    }

    fn action_allowed(self, action: NodeAction, node: Option<NodeKind>) -> bool {
        if action.is_runtime_action() {
            return self.profile == "local" && node.is_none();
        }
        available_actions_for(self.profile, node, self.has_active_devnet).contains(&action)
    }
}

#[must_use]
pub(super) fn node_set_for_profile(profile: &str) -> Vec<NodeKind> {
    let profile = normalized_profile(profile);
    adapters_for_profile(profile)
        .into_iter()
        .map(|adapter| adapter.kind())
        .collect()
}

#[must_use]
pub(super) fn available_actions_for(
    profile: &str,
    node: Option<NodeKind>,
    has_active_devnet: bool,
) -> Vec<NodeAction> {
    let local_mode = normalized_profile(profile) == "local";
    if node.is_none() {
        if local_mode {
            let mut actions = vec![NodeAction::NewNetwork, NodeAction::LoadNetwork];
            if has_active_devnet {
                actions.extend([NodeAction::ResetNetwork, NodeAction::DeleteNetwork]);
            }
            return actions;
        }
        return Vec::new();
    }

    let Some(kind) = node else {
        return Vec::new();
    };
    if !node_set_for_profile(profile).contains(&kind) {
        return Vec::new();
    }

    if !local_mode || !has_active_devnet {
        return Vec::new();
    }

    adapter_for(kind).workflow_actions().to_vec()
}

#[must_use]
pub(super) fn normalized_profile(profile: &str) -> &'static str {
    match profile.trim().to_ascii_lowercase().as_str() {
        "local" | "localnet" | "devnet" => "local",
        _ => "default",
    }
}
