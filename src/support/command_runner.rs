use std::{
    io::ErrorKind,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};

pub(crate) struct CommandRunPolicy<'a> {
    pub(crate) label: &'a str,
    pub(crate) timeout: Duration,
    pub(crate) poll_interval: Duration,
    pub(crate) redactions: &'a [&'a str],
    pub(crate) output_limit: usize,
}

pub(crate) fn run_command(mut command: Command, policy: CommandRunPolicy<'_>) -> Result<Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to run {}", policy.label))?;
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .with_context(|| format!("failed to poll {}", policy.label))?
            .is_some()
        {
            break;
        }
        if started.elapsed() >= policy.timeout {
            match child.kill() {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::InvalidInput => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to kill timed-out {}", policy.label));
                }
            }
            let output = child
                .wait_with_output()
                .with_context(|| format!("failed to collect timed-out {}", policy.label))?;
            let message = process_message(&output, policy.redactions, policy.output_limit);
            bail!(
                "{} timed out after {} ms: {}",
                policy.label,
                policy.timeout.as_millis(),
                message
            );
        }
        thread::sleep(policy.poll_interval);
    }
    let output = child
        .wait_with_output()
        .with_context(|| format!("failed to collect {}", policy.label))?;
    if !output.status.success() {
        let message = process_message(&output, policy.redactions, policy.output_limit);
        bail!("{} exited with {}: {message}", policy.label, output.status);
    }
    Ok(output)
}

pub(crate) fn spawn_detached(mut command: Command, label: &str) -> Result<u32> {
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

pub(crate) fn process_message(output: &Output, redactions: &[&str], limit: usize) -> String {
    let message = if output.stderr.is_empty() {
        output_text(&output.stdout, redactions, limit)
    } else {
        output_text(&output.stderr, redactions, limit)
    };
    if message.is_empty() {
        "no output".to_owned()
    } else {
        message
    }
}

pub(crate) fn output_text(output: &[u8], redactions: &[&str], limit: usize) -> String {
    let text = String::from_utf8_lossy(output).trim().to_owned();
    let mut redacted = text;
    for value in redactions {
        let value = value.trim();
        if !value.is_empty() {
            redacted = redacted.replace(value, "...");
        }
    }
    redacted.chars().take(limit).collect()
}
