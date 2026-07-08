use crate::modules::ModuleReport;

pub async fn delivery_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
) -> ModuleReport {
    crate::source_routing::shared::inspection::delivery_source_report(
        source_mode,
        rest_endpoint,
        metrics_endpoint,
    )
    .await
}
