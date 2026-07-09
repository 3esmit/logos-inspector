use super::super::SourceProbeKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HttpProbeNormalizer {
    Identity,
    StorageManifests,
    StorageSpr,
    StoragePeerId,
    StorageExists(String),
    DeliveryHealth,
    DeliveryInfo,
    DeliveryVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpJsonProbeStep {
    pub(crate) key: SourceProbeKey,
    pub(crate) label: &'static str,
    pub(crate) path: String,
    pub(crate) normalizer: HttpProbeNormalizer,
}

impl HttpJsonProbeStep {
    fn new(
        key: SourceProbeKey,
        label: &'static str,
        path: impl Into<String>,
        normalizer: HttpProbeNormalizer,
    ) -> Self {
        Self {
            key,
            label,
            path: path.into(),
            normalizer,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleProbeStep<'a> {
    pub(crate) method: &'static str,
    pub(crate) args: Vec<&'a str>,
    pub(crate) key: Option<SourceProbeKey>,
}

impl<'a> ModuleProbeStep<'a> {
    pub(crate) fn keyed(method: &'static str, key: SourceProbeKey) -> Self {
        Self {
            method,
            args: Vec::new(),
            key: Some(key),
        }
    }

    pub(crate) fn keyed_with_args(
        method: &'static str,
        args: Vec<&'a str>,
        key: SourceProbeKey,
    ) -> Self {
        Self {
            method,
            args,
            key: Some(key),
        }
    }

    pub(crate) fn unkeyed(method: &'static str, args: Vec<&'a str>) -> Self {
        Self {
            method,
            args,
            key: None,
        }
    }
}

pub(crate) fn storage_rest_probe_plan(
    cid: Option<&str>,
    privileged_debug_enabled: bool,
) -> Vec<HttpJsonProbeStep> {
    let mut steps = vec![
        HttpJsonProbeStep::new(
            SourceProbeKey::StorageSpace,
            "storage_rest.space",
            "/space",
            HttpProbeNormalizer::Identity,
        ),
        HttpJsonProbeStep::new(
            SourceProbeKey::StorageSpr,
            "storage_rest.spr",
            "/spr",
            HttpProbeNormalizer::StorageSpr,
        ),
        HttpJsonProbeStep::new(
            SourceProbeKey::StoragePeerId,
            "storage_rest.peerId",
            "/peerid",
            HttpProbeNormalizer::StoragePeerId,
        ),
        HttpJsonProbeStep::new(
            SourceProbeKey::StorageManifests,
            "storage_rest.manifests",
            "/data",
            HttpProbeNormalizer::StorageManifests,
        ),
    ];
    if privileged_debug_enabled {
        steps.push(HttpJsonProbeStep::new(
            SourceProbeKey::StorageDebug,
            "storage_rest.debug",
            "/debug/info",
            HttpProbeNormalizer::Identity,
        ));
    }
    if let Some(cid) = cid.map(str::trim).filter(|cid| !cid.is_empty()) {
        steps.push(HttpJsonProbeStep::new(
            SourceProbeKey::StorageExists,
            "storage_rest.exists",
            format!("/data/{cid}/exists"),
            HttpProbeNormalizer::StorageExists(cid.to_owned()),
        ));
    }
    steps
}

pub(crate) fn delivery_rest_probe_plan() -> Vec<HttpJsonProbeStep> {
    vec![
        HttpJsonProbeStep::new(
            SourceProbeKey::DeliveryHealth,
            "delivery_rest.health",
            "/health",
            HttpProbeNormalizer::DeliveryHealth,
        ),
        HttpJsonProbeStep::new(
            SourceProbeKey::DeliveryInfo,
            "delivery_rest.info",
            "/info",
            HttpProbeNormalizer::DeliveryInfo,
        ),
        HttpJsonProbeStep::new(
            SourceProbeKey::DeliveryVersion,
            "delivery_rest.version",
            "/version",
            HttpProbeNormalizer::DeliveryVersion,
        ),
    ]
}

pub(crate) fn delivery_network_monitor_probe_plan() -> Vec<HttpJsonProbeStep> {
    vec![
        HttpJsonProbeStep::new(
            SourceProbeKey::DeliveryAllPeersInfo,
            "delivery_network_monitor.allPeersInfo",
            "/allpeersinfo",
            HttpProbeNormalizer::Identity,
        ),
        HttpJsonProbeStep::new(
            SourceProbeKey::DeliveryContentTopics,
            "delivery_network_monitor.contentTopics",
            "/contenttopics",
            HttpProbeNormalizer::Identity,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_rest_probe_plan_includes_optional_debug_and_cid_steps() {
        let steps = storage_rest_probe_plan(Some("cid-1"), true);
        let keys = steps.iter().map(|step| step.key).collect::<Vec<_>>();

        assert!(keys.contains(&SourceProbeKey::StorageDebug));
        assert!(keys.contains(&SourceProbeKey::StorageExists));
        assert!(steps.iter().any(|step| step.path == "/data/cid-1/exists"));
    }

    #[test]
    fn delivery_rest_probe_plan_declares_base_health_info_version_steps() {
        let steps = delivery_rest_probe_plan();
        let keys = steps.iter().map(|step| step.key).collect::<Vec<_>>();

        assert_eq!(steps.len(), 3);
        assert!(keys.contains(&SourceProbeKey::DeliveryHealth));
        assert!(keys.contains(&SourceProbeKey::DeliveryInfo));
        assert!(keys.contains(&SourceProbeKey::DeliveryVersion));
    }

    #[test]
    fn delivery_network_monitor_probe_plan_declares_monitor_endpoints() {
        let steps = delivery_network_monitor_probe_plan();

        assert!(steps.iter().any(|step| step.path == "/allpeersinfo"));
        assert!(steps.iter().any(|step| step.path == "/contenttopics"));
    }
}
