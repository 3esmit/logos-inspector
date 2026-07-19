use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use reqwest::{Method, Url};
use serde_json::{Value, json};

use crate::modules::logos_core::{ModuleTransportKind, SharedModuleTransport};
use crate::source_routing::{
    ModuleDispatchIdentityRole, ModuleDispatchReceipt,
    shared::{http, module_bridge},
};
use crate::support::raw_source_transport::request_json_bounded;

use super::operations::DeliveryStoreQuery;

pub(super) const DELIVERY_PROBE_RESPONSE_LIMIT: usize = 64 * 1024;
pub(super) const DELIVERY_PEER_RESPONSE_LIMIT: usize = 4 * 1024 * 1024;
const DELIVERY_PROBE_TIMEOUT: Duration = Duration::from_secs(6);
const DELIVERY_PROBE_SOURCE_FALLBACK: &str = "configured Delivery probe endpoint";

pub(super) async fn module_call(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    method: &'static str,
    args: Vec<Value>,
) -> Result<Value> {
    module_bridge::call_value(
        transport,
        transport_kind,
        super::layer::module_id(),
        method,
        args,
    )
    .await
    .map(|reply| reply.into_value())
}

pub(super) async fn module_dispatch(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    method: &'static str,
    args: Vec<Value>,
    context: &[(&'static str, String)],
    identity_role: ModuleDispatchIdentityRole,
) -> Result<ModuleDispatchReceipt> {
    let reply = module_bridge::call_value(
        transport,
        transport_kind,
        super::layer::module_id(),
        method,
        args,
    )
    .await?;
    Ok(module_bridge::dispatch_result(
        super::layer::module_id(),
        method,
        reply,
        context,
        identity_role,
    ))
}

pub(super) async fn update_subscription(
    endpoint: &str,
    topic: &str,
    subscribe: bool,
) -> Result<Value> {
    let method = if subscribe {
        Method::POST
    } else {
        Method::DELETE
    };
    http::rest_empty_request(
        method,
        endpoint,
        "/relay/v1/auto/subscriptions",
        Some(json!([topic])),
    )
    .await?;
    Ok(json!({
        "subscribed": subscribe,
        "contentTopic": topic,
        "endpoint": endpoint,
    }))
}

pub(super) async fn send(endpoint: &str, topic: &str, payload: &str) -> Result<Value> {
    http::rest_empty_request(
        Method::POST,
        endpoint,
        "/relay/v1/auto/messages",
        Some(json!({
            "contentTopic": topic,
            "payload": BASE64_STANDARD.encode(payload.as_bytes()),
        })),
    )
    .await?;
    Ok(json!({
        "sent": true,
        "contentTopic": topic,
        "bytes": payload.len(),
        "endpoint": endpoint,
    }))
}

pub(super) async fn probe_value(endpoint: &str, path: &str) -> Result<Value> {
    let url = http::rest_url(endpoint, path);
    let text = http::raw_http_text_url(&url).await?;
    Ok(parse_probe_text(&text))
}

pub(super) async fn probe_json_value_bounded(
    endpoint: &str,
    path: &str,
    max_bytes: usize,
) -> Result<Value> {
    let url = probe_json_url(endpoint, path)?;
    request_json_bounded(
        reqwest::Client::new()
            .get(url.clone())
            .timeout(DELIVERY_PROBE_TIMEOUT),
        url.as_str(),
        "failed to read Delivery probe response",
        "invalid Delivery probe JSON",
        false,
        false,
        max_bytes,
    )
    .await
}

pub(super) fn probe_json_source(endpoint: &str, path: &str) -> String {
    probe_json_url(endpoint, path)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| DELIVERY_PROBE_SOURCE_FALLBACK.to_owned())
}

fn probe_json_url(endpoint: &str, path: &str) -> Result<Url> {
    let url =
        Url::parse(&http::rest_url(endpoint, path)).context("invalid Delivery probe endpoint")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        bail!("Delivery probe endpoint must be an HTTP URL with a host");
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("Delivery probe endpoint cannot contain credentials, query, or fragment");
    }
    Ok(url)
}

pub(super) async fn probe_metrics(endpoint: &str) -> Result<String> {
    http::raw_http_text_url(endpoint).await
}

pub(super) async fn store_query(
    endpoint: &str,
    query: DeliveryStoreQuery<'_>,
) -> Result<(String, Value)> {
    let url = store_query_url(endpoint, query)?;
    let value = http::raw_http_json_url(url.as_str()).await?;
    Ok((url.to_string(), value))
}

pub(super) fn store_query_url(endpoint: &str, store_query: DeliveryStoreQuery<'_>) -> Result<Url> {
    let mut url = Url::parse(&http::rest_url(endpoint, "/store/v3/messages"))
        .context("invalid Delivery REST endpoint")?;
    {
        let mut query = url.query_pairs_mut();
        if let Some(peer_addr) = store_query.peer_addr {
            query.append_pair("peerAddr", peer_addr);
        }
        if let Some(content_topics) = store_query.content_topics {
            query.append_pair("contentTopics", content_topics);
        }
        if let Some(pubsub_topic) = store_query.pubsub_topic {
            query.append_pair("pubsubTopic", pubsub_topic);
        }
        if let Some(cursor) = store_query.cursor {
            query.append_pair("cursor", cursor);
        }
        query.append_pair(
            "includeData",
            if store_query.include_data {
                "true"
            } else {
                "false"
            },
        );
        query.append_pair("pageSize", &store_query.page_size.to_string());
        query.append_pair(
            "ascending",
            if store_query.ascending {
                "true"
            } else {
                "false"
            },
        );
    }
    Ok(url)
}

fn parse_probe_text(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| Value::String(trimmed.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read as _, Write as _},
        net::TcpListener,
        thread,
    };

    #[test]
    fn probe_url_rejects_credential_and_query_leaks() {
        assert!(probe_json_url("http://user:secret@example.test", "/health").is_err());
        assert!(probe_json_url("http://example.test?token=secret", "/health").is_err());
        assert_eq!(
            probe_json_source("http://user:secret@example.test", "/health"),
            DELIVERY_PROBE_SOURCE_FALLBACK
        );
        assert_eq!(
            probe_json_source("http://example.test", "/health"),
            "http://example.test/health"
        );
    }

    #[tokio::test]
    async fn bounded_probe_rejects_oversized_declared_body() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request)?;
            let body = vec![b'x'; DELIVERY_PROBE_RESPONSE_LIMIT + 1];
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )?;
            if let Err(error) = stream.write_all(&body)
                && !matches!(
                    error.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                )
            {
                return Err(error.into());
            }
            Ok(())
        });

        let error =
            probe_json_value_bounded(&endpoint, "/contenttopics", DELIVERY_PROBE_RESPONSE_LIMIT)
                .await
                .err()
                .context("oversized Delivery probe body should fail")?;
        server
            .join()
            .map_err(|_| anyhow::anyhow!("Delivery probe server panicked"))??;

        anyhow::ensure!(
            error
                .to_string()
                .contains("http response body exceeded 65536 byte limit"),
            "unexpected Delivery probe limit error: {error:#}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn peer_probe_accepts_body_above_topic_limit() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request)?;
            let peer_data = "x".repeat(DELIVERY_PROBE_RESPONSE_LIMIT + 1);
            let body = serde_json::to_vec(&serde_json::json!({
                "peers": [{ "peerId": peer_data }]
            }))?;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )?;
            stream.write_all(&body)?;
            Ok(())
        });

        let value =
            probe_json_value_bounded(&endpoint, "/allpeersinfo", DELIVERY_PEER_RESPONSE_LIMIT)
                .await?;
        server
            .join()
            .map_err(|_| anyhow::anyhow!("Delivery peer probe server panicked"))??;

        let peer_id_length = value
            .get("peers")
            .and_then(Value::as_array)
            .and_then(|peers| peers.first())
            .and_then(|peer| peer.get("peerId"))
            .and_then(Value::as_str)
            .map(str::len);
        anyhow::ensure!(
            peer_id_length == Some(DELIVERY_PROBE_RESPONSE_LIMIT + 1),
            "unexpected bounded peer probe payload: {peer_id_length:?}"
        );
        Ok(())
    }
}
