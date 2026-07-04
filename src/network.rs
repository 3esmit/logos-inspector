use anyhow::{Context as _, Result};
use serde::Serialize;

pub const TESTNET_SEQUENCER_ENDPOINT: &str = "https://testnet.lez.logos.co/";
pub const LOCAL_SEQUENCER_ENDPOINT: &str = "http://127.0.0.1:3040/";
pub const DEFAULT_SEQUENCER_ENDPOINT: &str = TESTNET_SEQUENCER_ENDPOINT;
pub const DEFAULT_INDEXER_ENDPOINT: &str = "http://127.0.0.1:8779/";
pub const DEFAULT_NODE_ENDPOINT: &str = "http://127.0.0.1:8080/";
pub const DEFAULT_NETWORK_PROFILE: &str = "default";
pub const CUSTOM_NETWORK_PROFILE: &str = "custom";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NetworkProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub sequencer_endpoint: &'static str,
    pub indexer_endpoint: &'static str,
    pub node_endpoint: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkEndpoints {
    pub profile: String,
    pub sequencer_endpoint: String,
    pub indexer_endpoint: String,
    pub node_endpoint: String,
}

const NETWORK_PROFILES: &[NetworkProfile] = &[
    NetworkProfile {
        id: DEFAULT_NETWORK_PROFILE,
        label: "Testnet",
        sequencer_endpoint: DEFAULT_SEQUENCER_ENDPOINT,
        indexer_endpoint: DEFAULT_INDEXER_ENDPOINT,
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    },
    NetworkProfile {
        id: "local",
        label: "Local sequencer",
        sequencer_endpoint: LOCAL_SEQUENCER_ENDPOINT,
        indexer_endpoint: DEFAULT_INDEXER_ENDPOINT,
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    },
];

#[must_use]
pub const fn network_profiles() -> &'static [NetworkProfile] {
    NETWORK_PROFILES
}

pub fn resolve_network_endpoints(
    profile_id: Option<&str>,
    sequencer_url: Option<&str>,
    indexer_url: Option<&str>,
    node_url: Option<&str>,
) -> Result<NetworkEndpoints> {
    let selected_profile = profile_id
        .map(str::trim)
        .filter(|profile_id| !profile_id.is_empty())
        .unwrap_or(DEFAULT_NETWORK_PROFILE);
    let base = if selected_profile == CUSTOM_NETWORK_PROFILE {
        default_network_profile()
    } else {
        network_profile(selected_profile)?
    };

    let sequencer_endpoint = sequencer_url.unwrap_or(base.sequencer_endpoint).to_owned();
    let indexer_endpoint = indexer_url.unwrap_or(base.indexer_endpoint).to_owned();
    let node_endpoint = node_url.unwrap_or(base.node_endpoint).to_owned();
    let has_overrides = sequencer_url.is_some() || indexer_url.is_some() || node_url.is_some();
    let profile = if has_overrides {
        if selected_profile != CUSTOM_NETWORK_PROFILE
            && sequencer_endpoint == base.sequencer_endpoint
            && indexer_endpoint == base.indexer_endpoint
            && node_endpoint == base.node_endpoint
        {
            selected_profile.to_owned()
        } else {
            infer_network_profile(&sequencer_endpoint, &indexer_endpoint, &node_endpoint)
                .unwrap_or(CUSTOM_NETWORK_PROFILE)
                .to_owned()
        }
    } else if selected_profile == CUSTOM_NETWORK_PROFILE {
        DEFAULT_NETWORK_PROFILE.to_owned()
    } else {
        selected_profile.to_owned()
    };

    Ok(NetworkEndpoints {
        profile,
        sequencer_endpoint,
        indexer_endpoint,
        node_endpoint,
    })
}

#[must_use]
pub fn infer_network_profile(
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
    node_endpoint: &str,
) -> Option<&'static str> {
    NETWORK_PROFILES
        .iter()
        .find(|profile| {
            profile.sequencer_endpoint == sequencer_endpoint
                && profile.indexer_endpoint == indexer_endpoint
                && profile.node_endpoint == node_endpoint
        })
        .map(|profile| profile.id)
}

fn network_profile(profile_id: &str) -> Result<NetworkProfile> {
    NETWORK_PROFILES
        .iter()
        .copied()
        .find(|profile| profile.id == profile_id)
        .with_context(|| {
            let mut available = NETWORK_PROFILES
                .iter()
                .map(|profile| profile.id)
                .collect::<Vec<_>>();
            available.push(CUSTOM_NETWORK_PROFILE);
            let available = available.join(", ");
            format!("unknown network profile `{profile_id}`; available profiles: {available}")
        })
}

fn default_network_profile() -> NetworkProfile {
    NetworkProfile {
        id: DEFAULT_NETWORK_PROFILE,
        label: "Testnet",
        sequencer_endpoint: DEFAULT_SEQUENCER_ENDPOINT,
        indexer_endpoint: DEFAULT_INDEXER_ENDPOINT,
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_network_endpoints_uses_default_profile_without_overrides() {
        let endpoints = resolve_network_endpoints(None, None, None, None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, DEFAULT_NETWORK_PROFILE);
        assert_eq!(endpoints.sequencer_endpoint, DEFAULT_SEQUENCER_ENDPOINT);
        assert_eq!(endpoints.indexer_endpoint, DEFAULT_INDEXER_ENDPOINT);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_uses_local_profile() {
        let endpoints = resolve_network_endpoints(Some("local"), None, None, None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, "local");
        assert_eq!(endpoints.sequencer_endpoint, LOCAL_SEQUENCER_ENDPOINT);
        assert_eq!(endpoints.indexer_endpoint, DEFAULT_INDEXER_ENDPOINT);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_preserves_custom_urls() {
        let sequencer = "https://sequencer.example.invalid/";
        let indexer = "http://127.0.0.1:9999/";
        let node = "http://127.0.0.1:9090/";
        let endpoints = resolve_network_endpoints(
            Some(CUSTOM_NETWORK_PROFILE),
            Some(sequencer),
            Some(indexer),
            Some(node),
        );

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, CUSTOM_NETWORK_PROFILE);
        assert_eq!(endpoints.sequencer_endpoint, sequencer);
        assert_eq!(endpoints.indexer_endpoint, indexer);
        assert_eq!(endpoints.node_endpoint, node);
    }

    #[test]
    fn resolve_network_endpoints_custom_without_overrides_uses_default_profile() {
        let endpoints = resolve_network_endpoints(Some(CUSTOM_NETWORK_PROFILE), None, None, None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, DEFAULT_NETWORK_PROFILE);
        assert_eq!(endpoints.sequencer_endpoint, DEFAULT_SEQUENCER_ENDPOINT);
        assert_eq!(endpoints.indexer_endpoint, DEFAULT_INDEXER_ENDPOINT);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_explicit_urls_override_profile() {
        let sequencer = "https://override.example.invalid/";
        let endpoints =
            resolve_network_endpoints(Some(DEFAULT_NETWORK_PROFILE), Some(sequencer), None, None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, CUSTOM_NETWORK_PROFILE);
        assert_eq!(endpoints.sequencer_endpoint, sequencer);
        assert_eq!(endpoints.indexer_endpoint, DEFAULT_INDEXER_ENDPOINT);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_rejects_unknown_profile() {
        let endpoints = resolve_network_endpoints(Some("missing"), None, None, None);

        assert!(endpoints.is_err(), "{endpoints:?}");
        let Err(err) = endpoints else {
            return;
        };
        assert!(err.to_string().contains("unknown network profile"));
    }
}
