use anyhow::{Context as _, Result, bail};
use reqwest::{Response, StatusCode};
use serde_json::Value;

pub(crate) async fn read_response_text(
    request: reqwest::RequestBuilder,
    label: &str,
    body_context: &'static str,
    allow_no_content: bool,
) -> Result<String> {
    let response = request
        .send()
        .await
        .with_context(|| format!("failed to call {label}"))?;
    let (status, text) = read_response_body_text(response, body_context).await?;
    if !(status.is_success() || allow_no_content && status == StatusCode::NO_CONTENT) {
        bail!(
            "http call `{label}` failed with status {status}: {}",
            response_excerpt(&text)
        );
    }
    Ok(text)
}

pub(crate) async fn read_response_body_text(
    response: Response,
    body_context: &'static str,
) -> Result<(StatusCode, String)> {
    let status = response.status();
    let text = response.text().await.context(body_context)?;
    Ok((status, text))
}

pub(crate) fn parse_json_body(
    text: &str,
    invalid_context: &'static str,
    empty_as_null: bool,
) -> Result<Value> {
    let body = if empty_as_null { text.trim() } else { text };
    if empty_as_null && body.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(body)
        .with_context(|| format!("{invalid_context}: {}", response_excerpt(body)))
}

pub(crate) async fn read_response_bytes(
    request: reqwest::RequestBuilder,
    label: &str,
    body_context: &'static str,
) -> Result<Vec<u8>> {
    let response = request
        .send()
        .await
        .with_context(|| format!("failed to call {label}"))?;
    let status = response.status();
    let bytes = response.bytes().await.context(body_context)?;
    if !status.is_success() {
        bail!(
            "http call `{label}` failed with status {status}: {}",
            response_excerpt_bytes(&bytes)
        );
    }
    Ok(bytes.to_vec())
}

pub(crate) async fn expect_success_response(
    response: Response,
    label: &str,
    error_body_context: &'static str,
) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let bytes = response.bytes().await.context(error_body_context)?;
    bail!(
        "http call `{label}` failed with status {status}: {}",
        response_excerpt_bytes(&bytes)
    )
}

pub(crate) fn response_excerpt(text: &str) -> String {
    text.chars().take(400).collect()
}

pub(crate) fn response_excerpt_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(400).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_excerpt_truncates_at_four_hundred_chars() {
        let text = "a".repeat(450);

        assert_eq!(response_excerpt(&text).len(), 400);
    }

    #[test]
    fn response_excerpt_bytes_is_lossy_and_truncated() {
        let mut bytes = vec![0xff];
        bytes.extend(vec![b'a'; 450]);

        assert!(response_excerpt_bytes(&bytes).starts_with('\u{fffd}'));
        assert_eq!(response_excerpt_bytes(&bytes).chars().count(), 400);
    }

    #[test]
    fn parse_json_body_accepts_empty_as_null() {
        let result = parse_json_body("   ", "invalid JSON response", true);

        match result {
            Ok(value) => assert_eq!(value, Value::Null),
            Err(error) => assert!(error.to_string().is_empty(), "unexpected error: {error}"),
        }
    }

    #[test]
    fn parse_json_body_rejects_empty_when_strict() {
        let result = parse_json_body("   ", "invalid JSON response", false);

        let text = match result {
            Ok(value) => format!("expected error, got {value}"),
            Err(error) => error.to_string(),
        };
        assert!(text.contains("invalid JSON response"));
    }

    #[test]
    fn parse_json_body_reports_excerpt() {
        let result = parse_json_body("not-json", "invalid JSON response", false);

        let text = match result {
            Ok(value) => format!("expected error, got {value}"),
            Err(error) => error.to_string(),
        };
        assert!(text.contains("not-json"));
    }
}
