use std::{
    collections::VecDeque,
    env,
    io::{Read as _, Write as _},
    net::{TcpStream, ToSocketAddrs as _},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};

use crate::support::command_runner::CommandControl;

const RPC_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const RPC_PROBE_TIMEOUT: Duration = Duration::from_millis(250);
const RPC_PROBE_INTERVAL: Duration = Duration::from_millis(50);
const STARTUP_LOG_TAIL_BYTES: u64 = 4 * 1024;
const RPC_RESPONSE_LIMIT: u64 = 64 * 1024;

pub(super) fn find_command(command: &str) -> Option<String> {
    if command.contains(std::path::MAIN_SEPARATOR) {
        let path = Path::new(command);
        return path.is_file().then(|| path.display().to_string());
    }
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .map(|path| path.join(command))
        .find(|path| path.is_file())
        .map(|path| path.display().to_string())
}

pub(super) fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .is_ok_and(|status| status.success())
}

pub(super) fn stop_process(pid: u32) -> Result<()> {
    #[cfg(unix)]
    let target = format!("-{pid}");
    #[cfg(not(unix))]
    let target = pid.to_string();
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(target)
        .status()
        .with_context(|| format!("failed to stop process {pid}"))?;
    if !status.success() {
        bail!("process {pid} stop exited with {status}");
    }
    Ok(())
}

pub(super) fn spawn_detached(mut command: Command, label: &str) -> Result<u32> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let child = command
        .spawn()
        .with_context(|| format!("failed to start {label}"))?;
    Ok(child.id())
}

pub(super) fn spawn_rpc_ready(
    mut command: Command,
    label: &str,
    endpoint: &str,
    health_method: &str,
    control: Option<&CommandControl>,
) -> Result<u32> {
    if rpc_endpoint_ready(endpoint, health_method)? {
        bail!("{label} RPC endpoint is already serving another process");
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start {label}"))?;
    let output = BoundedChildOutput::start(&mut child, label)?;
    let deadline = Instant::now() + RPC_STARTUP_TIMEOUT;
    loop {
        if let Some(control) = control
            && let Err(error) = control.check_active()
        {
            terminate_child(&mut child);
            let _detail = output.finish();
            return Err(error.into());
        }
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("failed to inspect {label} startup"))?
        {
            let detail = output.finish();
            bail!("{label} exited before RPC readiness with {status}{detail}");
        }
        if rpc_endpoint_ready(endpoint, health_method)? {
            return Ok(child.id());
        }
        if Instant::now() >= deadline {
            terminate_child(&mut child);
            let detail = output.finish();
            bail!("{label} did not reach RPC readiness within 10 seconds{detail}");
        }
        thread::sleep(RPC_PROBE_INTERVAL);
    }
}

fn rpc_endpoint_ready(endpoint: &str, health_method: &str) -> Result<bool> {
    let url =
        reqwest::Url::parse(endpoint).context("registered process RPC endpoint is invalid")?;
    if url.scheme() != "http" {
        bail!("registered process RPC readiness requires an http endpoint");
    }
    let host = url
        .host_str()
        .context("registered process RPC endpoint has no host")?;
    let port = url
        .port_or_known_default()
        .context("registered process RPC endpoint has no port")?;
    let mut addresses = (host, port)
        .to_socket_addrs()
        .context("registered process RPC endpoint could not be resolved")?;
    let Some(address) = addresses.next() else {
        return Ok(false);
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&address, RPC_PROBE_TIMEOUT) else {
        return Ok(false);
    };
    stream.set_read_timeout(Some(RPC_PROBE_TIMEOUT))?;
    stream.set_write_timeout(Some(RPC_PROBE_TIMEOUT))?;
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": health_method,
        "params": [],
    })
    .to_string();
    let path = if url.path().is_empty() {
        "/"
    } else {
        url.path()
    };
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return Ok(false);
    }
    let mut response = Vec::new();
    if stream
        .take(RPC_RESPONSE_LIMIT)
        .read_to_end(&mut response)
        .is_err()
    {
        return Ok(false);
    }
    rpc_response_is_ready(&response)
}

fn rpc_response_is_ready(response: &[u8]) -> Result<bool> {
    let text = String::from_utf8_lossy(response);
    if !text.starts_with("HTTP/1.1 200") && !text.starts_with("HTTP/1.0 200") {
        return Ok(false);
    }
    let Some(start) = text.find('{') else {
        return Ok(false);
    };
    let Some(end) = text.rfind('}') else {
        return Ok(false);
    };
    let value: serde_json::Value = match serde_json::from_str(&text[start..=end]) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    if value.get("error").is_some_and(|error| !error.is_null()) {
        return Ok(false);
    }
    Ok(!matches!(
        value.get("result"),
        None | Some(serde_json::Value::Bool(false))
    ))
}

fn terminate_child(child: &mut Child) {
    let _ignored = stop_process(child.id());
    let _ignored = child.wait();
}

struct BoundedChildOutput {
    bytes: Arc<Mutex<VecDeque<u8>>>,
    drains: Vec<thread::JoinHandle<()>>,
}

impl BoundedChildOutput {
    fn start(child: &mut Child, label: &str) -> Result<Self> {
        let stdout = child.stdout.take().context("child stdout was not piped")?;
        let stderr = child.stderr.take().context("child stderr was not piped")?;
        let bytes = Arc::new(Mutex::new(VecDeque::new()));
        let stdout_drain = spawn_bounded_drain(stdout, Arc::clone(&bytes), label, "stdout")?;
        let stderr_drain = match spawn_bounded_drain(stderr, Arc::clone(&bytes), label, "stderr") {
            Ok(drain) => drain,
            Err(error) => {
                terminate_child(child);
                let _ignored = stdout_drain.join();
                return Err(error);
            }
        };
        let drains = vec![stdout_drain, stderr_drain];
        Ok(Self { bytes, drains })
    }

    fn finish(self) -> String {
        for drain in self.drains {
            let _ignored = drain.join();
        }
        let Ok(bytes) = self.bytes.lock() else {
            return String::new();
        };
        let bytes = bytes.iter().copied().collect::<Vec<_>>();
        let text = String::from_utf8_lossy(&bytes);
        let text = text.trim();
        if text.is_empty() {
            String::new()
        } else {
            format!(": {text}")
        }
    }
}

fn spawn_bounded_drain(
    mut reader: impl std::io::Read + Send + 'static,
    bytes: Arc<Mutex<VecDeque<u8>>>,
    label: &str,
    stream: &str,
) -> Result<thread::JoinHandle<()>> {
    thread::Builder::new()
        .name(format!("local-node-{stream}"))
        .spawn(move || {
            let mut chunk = [0_u8; 1024];
            loop {
                let count = match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                let Ok(mut output) = bytes.lock() else {
                    break;
                };
                output.extend(chunk.iter().take(count).copied());
                while output.len() > STARTUP_LOG_TAIL_BYTES as usize {
                    output.pop_front();
                }
            }
        })
        .with_context(|| format!("failed to capture {label} {stream}"))
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn rpc_readiness_accepts_successful_unit_result() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let server = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut request = [0_u8; 2048];
            let _read = stream.read(&mut request)?;
            let body = r#"{"jsonrpc":"2.0","id":1,"result":null}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )?;
            Ok(())
        });

        if !rpc_endpoint_ready(&format!("http://{address}/"), "checkHealth")? {
            bail!("valid JSON-RPC health response was not ready");
        }
        server
            .join()
            .map_err(|_| anyhow::anyhow!("RPC readiness server panicked"))??;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rpc_ready_spawn_reports_bounded_early_exit_diagnostics() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        drop(listener);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "echo indexer-startup-failed >&2; exit 7"]);

        let error = match spawn_rpc_ready(
            command,
            "test indexer",
            &format!("http://{address}/"),
            "checkHealth",
            None,
        ) {
            Ok(pid) => bail!("early exit unexpectedly reached readiness with process {pid}"),
            Err(error) => error,
        };
        let detail = error.to_string();
        if !detail.contains("exited before RPC readiness")
            || !detail.contains("indexer-startup-failed")
        {
            bail!("early exit diagnostics were lost: {detail}");
        }
        Ok(())
    }
}
