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

use super::adapters::RpcStartupReadiness;

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
    let exists = Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !exists {
        return false;
    }
    #[cfg(target_os = "linux")]
    let terminated = linux_process_state(pid).is_some_and(|state| matches!(state, 'Z' | 'X'));
    #[cfg(not(target_os = "linux"))]
    let terminated = false;
    !terminated
}

pub(super) fn process_group_is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    let target = format!("-{pid}");
    #[cfg(not(unix))]
    let target = pid.to_string();
    Command::new("kill")
        .arg("-0")
        .arg("--")
        .arg(target)
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub(super) fn process_group_has_live_members(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return process_group_is_alive(pid);
        };
        entries.filter_map(|entry| entry.ok()).any(|entry| {
            let Ok(process_id) = entry.file_name().to_string_lossy().parse::<u32>() else {
                return false;
            };
            linux_process_group_state(process_id).is_some_and(|(process_group, state)| {
                process_group == pid && !matches!(state, 'Z' | 'X')
            })
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        process_group_is_alive(pid)
    }
}

#[cfg(target_os = "linux")]
fn linux_process_state(pid: u32) -> Option<char> {
    linux_process_group_state(pid).map(|(_, state)| state)
}

#[cfg(target_os = "linux")]
fn linux_process_group_state(pid: u32) -> Option<(u32, char)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, suffix) = stat.rsplit_once(") ")?;
    let mut fields = suffix.split_whitespace();
    let state = fields.next()?.chars().next()?;
    let _parent_process_id = fields.next()?;
    let process_group = fields.next()?.parse::<u32>().ok()?;
    Some((process_group, state))
}

pub(super) fn stop_process(pid: u32) -> Result<()> {
    #[cfg(unix)]
    let target = format!("-{pid}");
    #[cfg(not(unix))]
    let target = pid.to_string();
    let status = Command::new("kill")
        .arg("-TERM")
        .arg("--")
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
    readiness: RpcStartupReadiness,
    control: Option<&CommandControl>,
) -> Result<u32> {
    validate_rpc_readiness(readiness)?;
    let started_at = Instant::now();
    let configured_deadline = started_at
        .checked_add(readiness.startup_timeout)
        .context("registered process RPC startup timeout overflowed")?;
    let deadline = control
        .map(CommandControl::deadline)
        .map_or(configured_deadline, |control_deadline| {
            configured_deadline.min(control_deadline)
        });
    if let Some(control) = control {
        control.check_active()?;
    }
    let preflight_timeout = remaining_probe_timeout(deadline, readiness.probe_timeout)
        .context("registered process RPC readiness deadline expired before startup")?;
    if rpc_endpoint_ready(endpoint, readiness.method, preflight_timeout)? {
        bail!("{label} RPC endpoint is already serving another process");
    }
    if let Some(control) = control {
        control.check_active()?;
    }
    if Instant::now() >= deadline {
        bail!(
            "{label} RPC readiness deadline expired before startup after {}",
            display_duration(readiness.startup_timeout)
        );
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
        if Instant::now() >= deadline {
            terminate_child(&mut child);
            let detail = output.finish();
            bail!(
                "{label} did not reach RPC readiness within {}{detail}",
                display_duration(readiness.startup_timeout)
            );
        }
        let Some(probe_timeout) = remaining_probe_timeout(deadline, readiness.probe_timeout) else {
            continue;
        };
        let ready = match rpc_endpoint_ready(endpoint, readiness.method, probe_timeout) {
            Ok(ready) => ready,
            Err(error) => {
                terminate_child(&mut child);
                let detail = output.finish();
                return Err(error.context(format!("{label} RPC readiness probe failed{detail}")));
            }
        };
        if ready && Instant::now() < deadline {
            return Ok(child.id());
        }
        if ready {
            continue;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(readiness.retry_interval.min(remaining));
    }
}

fn validate_rpc_readiness(readiness: RpcStartupReadiness) -> Result<()> {
    if readiness.method.trim().is_empty() {
        bail!("registered process RPC readiness method is empty");
    }
    if readiness.startup_timeout.is_zero()
        || readiness.probe_timeout.is_zero()
        || readiness.retry_interval.is_zero()
    {
        bail!("registered process RPC readiness durations must be positive");
    }
    Ok(())
}

fn remaining_probe_timeout(deadline: Instant, configured: Duration) -> Option<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    (!remaining.is_zero()).then(|| configured.min(remaining))
}

fn display_duration(duration: Duration) -> String {
    let milliseconds = duration.as_millis();
    if milliseconds.is_multiple_of(1_000) {
        format!("{} seconds", milliseconds / 1_000)
    } else {
        format!("{milliseconds} milliseconds")
    }
}

fn rpc_endpoint_ready(
    endpoint: &str,
    health_method: &str,
    probe_timeout: Duration,
) -> Result<bool> {
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
    let Ok(mut stream) = TcpStream::connect_timeout(&address, probe_timeout) else {
        return Ok(false);
    };
    stream.set_read_timeout(Some(probe_timeout))?;
    stream.set_write_timeout(Some(probe_timeout))?;
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
    #[cfg(target_os = "linux")]
    use std::fs;
    use std::{net::TcpListener, time::Duration};

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

        if !rpc_endpoint_ready(
            &format!("http://{address}/"),
            "checkHealth",
            Duration::from_secs(1),
        )? {
            bail!("valid JSON-RPC health response was not ready");
        }
        server
            .join()
            .map_err(|_| anyhow::anyhow!("RPC readiness server panicked"))??;
        Ok(())
    }

    #[test]
    fn rpc_readiness_allows_a_slow_health_response_within_policy() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let server = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut request = [0_u8; 2048];
            let _read = stream.read(&mut request)?;
            thread::sleep(Duration::from_millis(500));
            let body = r#"{"jsonrpc":"2.0","id":1,"result":null}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )?;
            Ok(())
        });

        if !rpc_endpoint_ready(
            &format!("http://{address}/"),
            "checkHealth",
            Duration::from_secs(1),
        )? {
            bail!("slow JSON-RPC health response was not ready within policy");
        }
        server
            .join()
            .map_err(|_| anyhow::anyhow!("slow RPC readiness server panicked"))??;
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
            RpcStartupReadiness::new(
                "checkHealth",
                Duration::from_secs(1),
                Duration::from_millis(100),
                Duration::from_millis(10),
            ),
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

    #[cfg(target_os = "linux")]
    #[test]
    fn rpc_ready_spawn_reports_configured_timeout_and_reaps_child() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        drop(listener);
        let directory = tempfile::tempdir()?;
        let pid_path = directory.path().join("startup.pid");
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "printf '%s' \"$$\" > \"$1\"; sleep 30",
            "sh",
            pid_path
                .to_str()
                .context("test PID path is not valid UTF-8")?,
        ]);

        let error = match spawn_rpc_ready(
            command,
            "test indexer",
            &format!("http://{address}/"),
            RpcStartupReadiness::new(
                "checkHealth",
                Duration::from_millis(150),
                Duration::from_millis(25),
                Duration::from_millis(10),
            ),
            None,
        ) {
            Ok(pid) => bail!("unready process unexpectedly reached readiness with process {pid}"),
            Err(error) => error,
        };
        let detail = error.to_string();
        if !detail.contains("within 150 milliseconds") {
            bail!("configured readiness timeout was not reported: {detail}");
        }
        let pid = fs::read_to_string(&pid_path)?
            .trim()
            .parse::<u32>()
            .context("test startup PID is invalid")?;
        if process_is_alive(pid) {
            bail!("timed-out readiness process {pid} remained alive");
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rpc_ready_spawn_honors_shorter_command_deadline_and_reaps_child() -> Result<()> {
        use crate::support::command_runner::{CommandStopReason, CommandTerminated};
        use tokio_util::sync::CancellationToken;

        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        drop(listener);
        let directory = tempfile::tempdir()?;
        let pid_path = directory.path().join("controlled-startup.pid");
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "printf '%s' \"$$\" > \"$1\"; sleep 30",
            "sh",
            pid_path
                .to_str()
                .context("test PID path is not valid UTF-8")?,
        ]);
        let control = CommandControl::new(
            CancellationToken::new(),
            Instant::now() + Duration::from_millis(150),
        );

        let error = match spawn_rpc_ready(
            command,
            "controlled indexer",
            &format!("http://{address}/"),
            RpcStartupReadiness::new(
                "checkHealth",
                Duration::from_secs(5),
                Duration::from_millis(25),
                Duration::from_millis(10),
            ),
            Some(&control),
        ) {
            Ok(pid) => {
                bail!("controlled process unexpectedly reached readiness with process {pid}")
            }
            Err(error) => error,
        };
        let termination = error
            .downcast_ref::<CommandTerminated>()
            .context("controlled readiness did not retain typed termination")?;
        if termination.reason() != CommandStopReason::DeadlineExceeded {
            bail!("controlled readiness stopped for the wrong reason");
        }
        let pid = fs::read_to_string(&pid_path)?
            .trim()
            .parse::<u32>()
            .context("controlled startup PID is invalid")?;
        if process_is_alive(pid) {
            bail!("deadline-stopped readiness process {pid} remained alive");
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn process_is_alive_treats_zombies_as_stopped() -> Result<()> {
        let mut child = Command::new("/bin/sh").args(["-c", "exit 0"]).spawn()?;
        let pid = child.id();
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut zombie = false;
        while Instant::now() < deadline {
            let state = fs::read_to_string(format!("/proc/{pid}/stat"))
                .ok()
                .and_then(|stat| {
                    stat.rsplit_once(") ")
                        .and_then(|(_, stat)| stat.chars().next())
                });
            if state == Some('Z') {
                zombie = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let alive = process_is_alive(pid);
        let _status = child.wait()?;

        if !zombie {
            bail!("test child did not enter the zombie state");
        }
        if alive {
            bail!("zombie process {pid} was reported as alive");
        }
        Ok(())
    }

    #[cfg(all(unix, target_os = "linux"))]
    #[test]
    fn process_group_live_members_ignore_a_zombie_leader() -> Result<()> {
        use anyhow::Context as _;
        use std::os::unix::process::CommandExt as _;

        let directory = tempfile::tempdir()?;
        let child_path = directory.path().join("child.pid");
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "sleep 30 & printf '%s' \"$!\" > \"$1\"; exit 0",
                "sh",
                child_path
                    .to_str()
                    .context("test child path is not valid UTF-8")?,
            ])
            .process_group(0);
        let mut leader = command.spawn()?;
        let leader_process_id = leader.id();

        let result = (|| -> Result<()> {
            let deadline = Instant::now() + Duration::from_secs(1);
            while !child_path.is_file() || process_is_alive(leader_process_id) {
                if Instant::now() >= deadline {
                    bail!("test process group leader did not exit after starting its child");
                }
                thread::sleep(Duration::from_millis(10));
            }
            anyhow::ensure!(
                process_group_has_live_members(leader_process_id),
                "live child was not found in the zombie leader process group"
            );

            stop_process(leader_process_id)?;
            let deadline = Instant::now() + Duration::from_secs(1);
            while process_group_has_live_members(leader_process_id) {
                if Instant::now() >= deadline {
                    bail!("live child remained after process group termination");
                }
                thread::sleep(Duration::from_millis(10));
            }
            Ok(())
        })();
        if process_group_is_alive(leader_process_id) {
            let _ignored = stop_process(leader_process_id);
        }
        let _status = leader.wait()?;
        result
    }
}
