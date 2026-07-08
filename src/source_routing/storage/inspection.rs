use crate::source_routing::SourceReport;

pub async fn storage_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    cid: Option<&str>,
    privileged_debug_enabled: bool,
) -> SourceReport {
    crate::source_routing::shared::inspection::storage_source_report(
        source_mode,
        rest_endpoint,
        metrics_endpoint,
        cid,
        privileged_debug_enabled,
    )
    .await
}
