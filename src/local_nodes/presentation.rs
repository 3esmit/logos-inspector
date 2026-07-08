use super::model::{LocalNodeProblemCode, LocalNodeStatus, LocalNodeTools};

pub(super) fn mode_for_profile(profile: &str) -> &'static str {
    if profile == "local" {
        "localnet"
    } else {
        "public_testnet"
    }
}

pub(super) fn primary_problem(
    profile: &str,
    tools: &LocalNodeTools,
    nodes: &[LocalNodeStatus],
) -> Option<LocalNodeProblemCode> {
    if !tools.logoscore.available {
        return Some(LocalNodeProblemCode::MissingLogoscore);
    }
    if profile == "local"
        && nodes
            .iter()
            .any(|node| node.key == "sequencer" && node.install_state == "needs_configuration")
    {
        return Some(LocalNodeProblemCode::MissingSequencerBinary);
    }
    None
}
