use std::{
    io::{ErrorKind, Read as _, Write as _},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use serde_json::Value;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const IO_TIMEOUT: Duration = Duration::from_secs(1);
const RESPONSE_LIMIT: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MessagingHealth {
    Ready,
    Initializing,
    Synchronizing,
    NotReady,
    NotMounted,
    ShuttingDown,
    EventLoopLagging,
    Unavailable,
    /// A listener accepted the health request but did not answer before its
    /// deadline. This is distinct from an unavailable endpoint but remains
    /// inconclusive lifecycle evidence.
    Unresponsive,
    Unknown,
}

impl MessagingHealth {
    /// Maps the semantic Delivery health state to lifecycle evidence.
    ///
    /// The REST health listener exists while a context is initializing, so a
    /// successful TCP connection alone must not be considered a running node.
    pub(super) const fn liveness(self) -> Option<bool> {
        match self {
            Self::Ready | Self::EventLoopLagging => Some(true),
            Self::Initializing | Self::Unavailable => Some(false),
            Self::Synchronizing
            | Self::NotReady
            | Self::NotMounted
            | Self::ShuttingDown
            | Self::Unresponsive
            | Self::Unknown => None,
        }
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Initializing => "INITIALIZING",
            Self::Synchronizing => "SYNCHRONIZING",
            Self::NotReady => "NOT_READY",
            Self::NotMounted => "NOT_MOUNTED",
            Self::ShuttingDown => "SHUTTING_DOWN",
            Self::EventLoopLagging => "EVENT_LOOP_LAGGING",
            Self::Unavailable => "unavailable",
            Self::Unresponsive => "unresponsive",
            Self::Unknown => "unknown",
        }
    }
}

pub(super) fn probe(address: SocketAddr) -> MessagingHealth {
    let mut stream = match TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
        Ok(stream) => stream,
        Err(error) => {
            return match error.kind() {
                ErrorKind::ConnectionRefused
                | ErrorKind::ConnectionAborted
                | ErrorKind::NotConnected => MessagingHealth::Unavailable,
                _ => MessagingHealth::Unknown,
            };
        }
    };
    if stream.set_read_timeout(Some(IO_TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(IO_TIMEOUT)).is_err()
    {
        return MessagingHealth::Unknown;
    }
    let request = format!("GET /health HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return MessagingHealth::Unknown;
    }
    let mut response = Vec::new();
    if let Err(error) = stream.take(RESPONSE_LIMIT).read_to_end(&mut response) {
        return if response.is_empty()
            && matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock)
        {
            MessagingHealth::Unresponsive
        } else {
            MessagingHealth::Unknown
        };
    }
    from_http_response(&response)
}

fn from_http_response(response: &[u8]) -> MessagingHealth {
    let Ok(response) = std::str::from_utf8(response) else {
        return MessagingHealth::Unknown;
    };
    let Some((head, body)) = response.split_once("\r\n\r\n") else {
        return MessagingHealth::Unknown;
    };
    if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
        return MessagingHealth::Unknown;
    }
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return MessagingHealth::Unknown;
    };
    match value.get("nodeHealth").and_then(Value::as_str) {
        Some("READY") => MessagingHealth::Ready,
        Some("INITIALIZING") => MessagingHealth::Initializing,
        Some("SYNCHRONIZING") => MessagingHealth::Synchronizing,
        Some("NOT_READY") => MessagingHealth::NotReady,
        Some("NOT_MOUNTED") => MessagingHealth::NotMounted,
        Some("SHUTTING_DOWN") => MessagingHealth::ShuttingDown,
        Some("EVENT_LOOP_LAGGING") => MessagingHealth::EventLoopLagging,
        _ => MessagingHealth::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read as _, Write as _},
        net::{Ipv4Addr, TcpListener},
        thread,
    };

    use anyhow::{Result, bail};

    use super::*;

    #[test]
    fn parses_semantic_node_state() {
        let response = |node_health: &str| {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"nodeHealth\":\"{node_health}\"}}"
            )
        };

        assert_eq!(
            from_http_response(response("READY").as_bytes()),
            MessagingHealth::Ready
        );
        assert_eq!(
            from_http_response(response("INITIALIZING").as_bytes()),
            MessagingHealth::Initializing
        );
        assert_eq!(
            from_http_response(response("SYNCHRONIZING").as_bytes()),
            MessagingHealth::Synchronizing
        );
        assert_eq!(
            from_http_response(response("NOT_READY").as_bytes()),
            MessagingHealth::NotReady
        );
        assert_eq!(
            from_http_response(response("NOT_MOUNTED").as_bytes()),
            MessagingHealth::NotMounted
        );
        assert_eq!(
            from_http_response(response("SHUTTING_DOWN").as_bytes()),
            MessagingHealth::ShuttingDown
        );
        assert_eq!(
            from_http_response(response("EVENT_LOOP_LAGGING").as_bytes()),
            MessagingHealth::EventLoopLagging
        );
        assert_eq!(
            from_http_response(b"HTTP/1.1 503 Service Unavailable\r\n\r\n{}"),
            MessagingHealth::Unknown
        );
    }

    #[test]
    fn liveness_requires_ready_delivery_state() {
        assert_eq!(MessagingHealth::Ready.liveness(), Some(true));
        assert_eq!(MessagingHealth::EventLoopLagging.liveness(), Some(true));
        assert_eq!(MessagingHealth::Initializing.liveness(), Some(false));
        assert_eq!(MessagingHealth::Unavailable.liveness(), Some(false));
        assert_eq!(MessagingHealth::Synchronizing.liveness(), None);
        assert_eq!(MessagingHealth::NotReady.liveness(), None);
        assert_eq!(MessagingHealth::NotMounted.liveness(), None);
        assert_eq!(MessagingHealth::ShuttingDown.liveness(), None);
        assert_eq!(MessagingHealth::Unresponsive.liveness(), None);
        assert_eq!(MessagingHealth::Unknown.liveness(), None);
    }

    #[test]
    fn partial_health_response_timeout_stays_unknown() -> Result<()> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let worker = thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request)?;
            stream.write_all(b"HTTP/1.1 200 OK\r\n")?;
            thread::sleep(Duration::from_secs(2));
            Ok(())
        });

        let health = probe(address);
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("partial health listener panicked"))??;
        if health != MessagingHealth::Unknown {
            bail!("partial Delivery health response became lifecycle evidence: {health:?}");
        }
        Ok(())
    }
}
