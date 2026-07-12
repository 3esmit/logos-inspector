use anyhow::{Context as _, Result};
use serde::Serialize;

use super::policy::DEFAULT_NODE_ENDPOINT;

pub const DEFAULT_NETWORK_PROFILE: &str = "default";
pub const CUSTOM_NETWORK_PROFILE: &str = "custom";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NetworkProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub node_endpoint: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkEndpoints {
    pub profile: String,
    pub node_endpoint: String,
}

const NETWORK_PROFILES: &[NetworkProfile] = &[
    NetworkProfile {
        id: DEFAULT_NETWORK_PROFILE,
        label: "Testnet",
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    },
    NetworkProfile {
        id: "local",
        label: "Local node",
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    },
];

#[must_use]
pub const fn network_profiles() -> &'static [NetworkProfile] {
    NETWORK_PROFILES
}

pub fn resolve_network_endpoints(
    profile_id: Option<&str>,
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

    let node_endpoint = node_url.unwrap_or(base.node_endpoint).to_owned();
    let profile = if node_url.is_some() {
        let inferred = infer_network_profile(&node_endpoint);
        if selected_profile != CUSTOM_NETWORK_PROFILE && inferred == Some(selected_profile) {
            selected_profile.to_owned()
        } else {
            inferred.unwrap_or(CUSTOM_NETWORK_PROFILE).to_owned()
        }
    } else if selected_profile == CUSTOM_NETWORK_PROFILE {
        DEFAULT_NETWORK_PROFILE.to_owned()
    } else {
        selected_profile.to_owned()
    };

    Ok(NetworkEndpoints {
        profile,
        node_endpoint,
    })
}

#[must_use]
pub fn infer_network_profile(node_endpoint: &str) -> Option<&'static str> {
    NETWORK_PROFILES
        .iter()
        .find(|profile| profile_endpoint_matches(profile.node_endpoint, node_endpoint))
        .map(|profile| profile.id)
}

fn profile_endpoint_matches(profile_endpoint: &str, endpoint: &str) -> bool {
    comparable_endpoint(profile_endpoint) == comparable_endpoint(endpoint)
}

fn comparable_endpoint(endpoint: &str) -> &str {
    endpoint.trim().trim_end_matches('/')
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
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_network_endpoints_uses_default_profile_without_override() {
        let endpoints = resolve_network_endpoints(None, None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, DEFAULT_NETWORK_PROFILE);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_uses_local_profile() {
        let endpoints = resolve_network_endpoints(Some("local"), None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, "local");
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_preserves_custom_url() {
        let node = "http://127.0.0.1:9090/";
        let endpoints = resolve_network_endpoints(Some(CUSTOM_NETWORK_PROFILE), Some(node));

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, CUSTOM_NETWORK_PROFILE);
        assert_eq!(endpoints.node_endpoint, node);
    }

    #[test]
    fn resolve_network_endpoints_custom_without_override_uses_default_profile() {
        let endpoints = resolve_network_endpoints(Some(CUSTOM_NETWORK_PROFILE), None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, DEFAULT_NETWORK_PROFILE);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn infer_network_profile_ignores_trailing_slashes() {
        assert_eq!(
            infer_network_profile("http://127.0.0.1:8080"),
            Some(DEFAULT_NETWORK_PROFILE)
        );
    }

    #[test]
    fn resolve_network_endpoints_rejects_unknown_profile() {
        let endpoints = resolve_network_endpoints(Some("missing"), None);

        let error = match endpoints {
            Ok(value) => format!("expected error, got {value:?}"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("unknown network profile `missing`"));
        assert!(error.contains(DEFAULT_NETWORK_PROFILE));
        assert!(error.contains(CUSTOM_NETWORK_PROFILE));
    }
}
