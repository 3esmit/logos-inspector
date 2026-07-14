#[cfg(target_os = "linux")]
use std::process::{Child, Stdio};
use std::{
    fs,
    io::{Read as _, Write as _},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead as _, KeyInit as _, Payload},
};
use hkdf::Hkdf;
use serde_json::{Value, json};
use sha2::Sha256;

const OLD_SETTINGS: &[u8] =
    br#"{"version":2,"theme":"old","channel_source_configs":[],"favorites":[{"value":"old-favorite"}]}"#;
const OLD_IDLS: &[u8] = br#"{"version":1,"idls":[],"account_idl_selections":{}}"#;
const OLD_WALLET: &[u8] = br#"{"profile":{"label":"Old wallet"}}"#;

#[test]
fn cli_backup_download_preview_and_apply_are_distinct_catalog_actions() -> Result<()> {
    let directory = tempfile::tempdir()?;
    seed_original_state(directory.path())?;
    let payload = json!({
        "kind": "logos-inspector-settings-backup",
        "version": 1,
        "encrypted": false,
        "state": {
            "settings": {
                "version": 2,
                "theme": "new",
                "channel_source_configs": [],
                "favorites": [{ "value": "new-favorite" }]
            },
            "idls": {
                "version": 1,
                "idls": [{ "key": "idl-new", "json": "{}" }],
                "account_idl_selections": {}
            },
            "wallet": { "profile": { "label": "New wallet" } }
        }
    });
    let (endpoint, server) = one_response_server(serde_json::to_vec(&payload)?)?;

    let download = run_cli(
        directory.path(),
        &[
            "backup".to_owned(),
            "download".to_owned(),
            "cid-cli-vertical".to_owned(),
            "--source-mode".to_owned(),
            "rest".to_owned(),
            "--rest-url".to_owned(),
            endpoint,
        ],
    )?;
    let request = server
        .join()
        .map_err(|_| anyhow::anyhow!("backup HTTP fixture panicked"))??;
    if !request.starts_with("GET /data/cid-cli-vertical/network/stream HTTP/1.1\r\n") {
        bail!("CLI backup used unexpected Storage request: {request}");
    }
    let catalog_id = download
        .get("backup_catalog_id")
        .and_then(Value::as_str)
        .context("CLI download omitted backup catalog ID")?
        .to_owned();
    if download.get("downloaded").and_then(Value::as_bool) != Some(true)
        || download.get("restored").and_then(Value::as_bool) != Some(false)
        || download
            .pointer("/catalog_entry/remote/cid")
            .and_then(Value::as_str)
            != Some("cid-cli-vertical")
    {
        bail!("CLI download did not return a verified catalog-only result: {download}");
    }
    assert_original_state(directory.path())?;

    let catalog = run_cli(directory.path(), &["backup".to_owned(), "list".to_owned()])?;
    let entries = catalog
        .get("entries")
        .and_then(Value::as_array)
        .context("CLI backup list omitted catalog entries")?;
    if entries.len() != 1
        || entries
            .first()
            .and_then(|entry| entry.get("backup_catalog_id"))
            .and_then(Value::as_str)
            != Some(catalog_id.as_str())
    {
        bail!("CLI backup list did not select the downloaded entry: {catalog}");
    }

    let options =
        r#"{"app_settings":"replace","favorites":"replace","idl":"replace","wallet":"replace"}"#;
    let preview = run_cli(
        directory.path(),
        &[
            "backup".to_owned(),
            "preview".to_owned(),
            catalog_id.clone(),
            "--options".to_owned(),
            options.to_owned(),
        ],
    )?;
    if preview.get("outcome").and_then(Value::as_str) != Some("preview")
        || preview.get("terminal").and_then(Value::as_bool) != Some(false)
        || preview.get("backupCatalogId").and_then(Value::as_str) != Some(catalog_id.as_str())
    {
        bail!("CLI backup preview returned an invalid plan: {preview}");
    }
    assert_original_state(directory.path())?;

    let applied = run_cli(
        directory.path(),
        &[
            "backup".to_owned(),
            "apply".to_owned(),
            catalog_id.clone(),
            "--options".to_owned(),
            options.to_owned(),
        ],
    )?;
    if applied.get("outcome").and_then(Value::as_str) != Some("applied")
        || applied.get("terminal").and_then(Value::as_bool) != Some(true)
        || applied.get("applied").and_then(Value::as_bool) != Some(true)
        || applied.get("backupCatalogId").and_then(Value::as_str) != Some(catalog_id.as_str())
    {
        bail!("CLI backup apply returned a non-applied terminal: {applied}");
    }
    assert_imported_state(directory.path())
}

#[test]
fn cli_encrypted_backup_requires_wallet_before_preview_or_apply() -> Result<()> {
    let directory = tempfile::tempdir()?;
    seed_original_state(directory.path())?;
    let wallet_home = directory.path().join("wallet-home");
    fs::create_dir_all(&wallet_home)?;
    let wallet_config = br#"{"wallet":"encrypted-test"}"#;
    fs::write(wallet_home.join("wallet_config.json"), wallet_config)?;
    let plain = json!({
        "kind": "logos-inspector-settings-backup",
        "version": 1,
        "created_at": "1",
        "encrypted": false,
        "state": {
            "settings": {
                "version": 2,
                "theme": "new",
                "channel_source_configs": [],
                "favorites": [{ "value": "new-favorite" }]
            },
            "idls": {
                "version": 1,
                "idls": [{ "key": "idl-new", "json": "{}" }],
                "account_idl_selections": {}
            },
            "wallet": { "profile": { "label": "New wallet" } }
        }
    });
    let payload = encrypted_backup_payload(&plain, wallet_config)?;
    let (endpoint, server) = one_response_server(serde_json::to_vec(&payload)?)?;
    let download = run_cli(
        directory.path(),
        &[
            "backup".to_owned(),
            "download".to_owned(),
            "cid-encrypted".to_owned(),
            "--source-mode".to_owned(),
            "rest".to_owned(),
            "--rest-url".to_owned(),
            endpoint,
        ],
    )?;
    server
        .join()
        .map_err(|_| anyhow::anyhow!("encrypted backup HTTP fixture panicked"))??;
    let catalog_id = download
        .get("backup_catalog_id")
        .and_then(Value::as_str)
        .context("encrypted download omitted catalog ID")?
        .to_owned();
    anyhow::ensure!(
        download
            .pointer("/catalog_entry/encrypted")
            .and_then(Value::as_bool)
            == Some(true)
            && download.get("restored").and_then(Value::as_bool) == Some(false),
        "encrypted remote download bypassed catalog-only contract: {download}"
    );
    assert_original_state(directory.path())?;

    let options =
        r#"{"app_settings":"replace","favorites":"replace","idl":"replace","wallet":"replace"}"#;
    let missing_wallet = run_cli_output_with_env(
        directory.path(),
        &[
            "backup".to_owned(),
            "preview".to_owned(),
            catalog_id.clone(),
            "--options".to_owned(),
            options.to_owned(),
        ],
        &[],
    )?;
    anyhow::ensure!(
        !missing_wallet.status.success(),
        "encrypted preview succeeded without wallet material"
    );
    assert_original_state(directory.path())?;

    let wallet_profile = json!({ "wallet_home": wallet_home }).to_string();
    let preview = run_cli(
        directory.path(),
        &[
            "backup".to_owned(),
            "preview".to_owned(),
            catalog_id.clone(),
            "--options".to_owned(),
            options.to_owned(),
            "--wallet-profile".to_owned(),
            wallet_profile.clone(),
        ],
    )?;
    anyhow::ensure!(
        preview.get("outcome").and_then(Value::as_str) == Some("preview"),
        "encrypted preview did not produce a plan: {preview}"
    );
    assert_original_state(directory.path())?;

    let applied = run_cli(
        directory.path(),
        &[
            "backup".to_owned(),
            "apply".to_owned(),
            catalog_id,
            "--options".to_owned(),
            options.to_owned(),
            "--wallet-profile".to_owned(),
            wallet_profile,
        ],
    )?;
    anyhow::ensure!(
        applied.get("outcome").and_then(Value::as_str) == Some("applied"),
        "encrypted apply did not reach applied terminal: {applied}"
    );
    assert_imported_state(directory.path())
}

#[cfg(unix)]
#[test]
fn cli_rejects_unsafe_backup_cids_before_transport_or_catalog_mutation() -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempfile::tempdir()?;
    seed_original_state(directory.path())?;
    let fixture = directory.path().join("unsafe-cid-fixture");
    fs::create_dir_all(&fixture)?;
    let invoked = fixture.join("logoscore-invoked");
    let program = fixture.join("logoscore-test");
    fs::write(
        &program,
        format!("#!/bin/sh\ntouch {}\nexit 99\n", shell_path(&invoked)),
    )?;
    let mut permissions = fs::metadata(&program)?.permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&program, permissions)?;

    let instance_id = format!("unsafe-cid-test-{}", std::process::id());
    let logoscore_config = fixture.join("logoscore-config");
    fs::create_dir_all(logoscore_config.join("client"))?;
    fs::write(
        logoscore_config.join("client/config.json"),
        serde_json::to_vec(&json!({
            "instance_id": instance_id,
            "daemon": { "core_service": { "transport": "local" } }
        }))?,
    )?;
    let socket = std::env::temp_dir().join(format!("logos_core_service_{instance_id}"));
    fs::write(&socket, b"test socket identity")?;
    let _socket_cleanup = RemoveFileOnDrop(socket);
    let program_text = program.display().to_string();
    let config_text = logoscore_config.display().to_string();
    let envs = [
        ("LOGOSCORE_BIN", program_text.as_str()),
        ("LOGOSCORE_CONFIG_DIR", config_text.as_str()),
    ];
    let invalid_cids = [
        "../escape".to_owned(),
        "cid%2fnetwork".to_owned(),
        "cid?query".to_owned(),
        "a".repeat(257),
    ];

    for cid in &invalid_cids {
        let output = run_cli_output_with_env(
            directory.path(),
            &["backup".to_owned(), "download".to_owned(), cid.clone()],
            &envs,
        )?;
        anyhow::ensure!(
            !output.status.success(),
            "unsafe module backup CID `{cid}` was accepted"
        );
        anyhow::ensure!(
            !invoked.exists(),
            "unsafe module backup CID `{cid}` reached LogosCore dispatch"
        );
    }

    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    for cid in &invalid_cids {
        let output = run_cli_output_with_env(
            directory.path(),
            &[
                "backup".to_owned(),
                "download".to_owned(),
                cid.clone(),
                "--source-mode".to_owned(),
                "rest".to_owned(),
                "--rest-url".to_owned(),
                endpoint.clone(),
            ],
            &[],
        )?;
        anyhow::ensure!(
            !output.status.success(),
            "unsafe REST backup CID `{cid}` was accepted"
        );
        match listener.accept() {
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Ok(_) => bail!("unsafe REST backup CID `{cid}` reached network transport"),
            Err(error) => return Err(error.into()),
        }
    }

    anyhow::ensure!(
        !directory.path().join("backup_catalog.json").exists()
            && !directory.path().join("backup-payloads").exists(),
        "unsafe backup CID mutated local catalog or payload staging"
    );
    assert_original_state(directory.path())
}

#[cfg(unix)]
#[test]
fn cli_logoscore_event_download_flows_through_catalog_before_apply() -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempfile::tempdir()?;
    seed_original_state(directory.path())?;
    let payload = json!({
        "kind": "logos-inspector-settings-backup",
        "version": 1,
        "encrypted": false,
        "state": {
            "settings": {
                "version": 2,
                "theme": "new",
                "channel_source_configs": [],
                "favorites": [{ "value": "new-favorite" }]
            },
            "idls": {
                "version": 1,
                "idls": [{ "key": "idl-new", "json": "{}" }],
                "account_idl_selections": {}
            },
            "wallet": { "profile": { "label": "New wallet" } }
        }
    });
    let fixture = directory.path().join("logoscore-fixture");
    fs::create_dir_all(&fixture)?;
    let payload_path = fixture.join("payload.json");
    let trigger_path = fixture.join("download-started");
    let staging_path = fixture.join("staging-path");
    let operation_id_path = fixture.join("operation-id");
    let cid_path = fixture.join("cid");
    let program = fixture.join("logoscore-test");
    fs::write(&payload_path, serde_json::to_vec(&payload)?)?;
    let script = format!(
        "#!/bin/sh\n\
         if [ \"$1\" = \"--config-dir\" ]; then shift 2; fi\n\
         case \"$1\" in\n\
           list-modules) printf '%s\\n' '[{{\"name\":\"storage_module\",\"status\":\"loaded\"}}]' ;;\n\
           module-info) printf '%s\\n' '{{\"name\":\"storage_module\",\"methods\":[{{\"isInvokable\":true,\"name\":\"downloadProtocol\",\"signature\":\"downloadProtocol()\"}},{{\"isInvokable\":true,\"name\":\"downloadToUrlV2\",\"signature\":\"downloadToUrlV2(QString,QString,bool,int,QString,int)\"}},{{\"isInvokable\":true,\"name\":\"downloadCancelV2\",\"signature\":\"downloadCancelV2(QString)\"}}],\"events\":[{{\"name\":\"storageDownloadDoneV2\",\"signature\":\"storageDownloadDoneV2(QString)\"}}]}}' ;;\n\
           watch)\n\
             printf '%s\\n' '{{\"type\":\"subscription_ready\",\"protocol\":\"logoscore.watch\",\"version\":1,\"module\":\"storage_module\",\"event\":\"storageDownloadDoneV2\"}}'\n\
             while [ ! -f {trigger} ]; do sleep 0.01; done\n\
             operation_id=$(cat {operation_id}); cid=$(cat {cid})\n\
             printf '{{\"type\":\"event\",\"protocol\":\"logoscore.watch\",\"version\":1,\"timestamp\":\"2026-07-14T12:00:00Z\",\"module\":\"storage_module\",\"event\":\"storageDownloadDoneV2\",\"data\":{{\"arg0\":\"{{\\\"protocol\\\":\\\"logos.storage.download\\\",\\\"version\\\":2,\\\"moduleOperationId\\\":\\\"%s\\\",\\\"cid\\\":\\\"%s\\\",\\\"outcome\\\":\\\"succeeded\\\"}}\"}}}}\\n' \"$operation_id\" \"$cid\"\n\
             while :; do sleep 1; done ;;\n\
           call)\n\
             case \"$3\" in\n\
               downloadProtocol) printf '%s\\n' '{{\"status\":\"ok\",\"result\":{{\"success\":true,\"value\":{{\"protocol\":\"logos.storage.download\",\"version\":2,\"moduleOperationIdOwner\":\"caller\",\"cancelTimeoutMs\":15000,\"maxDownloadBytes\":1073741824}},\"error\":null}}}}' ;;\n\
               downloadToUrlV2)\n\
                 printf '%s' \"$5\" > {staging}\n\
                 cp {payload} \"$5\"\n\
                 printf '%s' \"$8\" > {operation_id}\n\
                 printf '%s' \"$4\" > {cid}\n\
                 touch {trigger}\n\
                 printf '{{\"status\":\"ok\",\"result\":{{\"success\":true,\"value\":{{\"protocol\":\"logos.storage.download\",\"version\":2,\"accepted\":true,\"moduleOperationId\":\"%s\",\"cid\":\"%s\"}},\"error\":null}}}}\\n' \"$8\" \"$4\" ;;\n\
               downloadCancelV2) cid=$(cat {cid}); printf '{{\"status\":\"ok\",\"result\":{{\"success\":true,\"value\":{{\"protocol\":\"logos.storage.download\",\"version\":2,\"moduleOperationId\":\"%s\",\"cid\":\"%s\",\"cancelStatus\":\"canceled\"}},\"error\":null}}}}\\n' \"$4\" \"$cid\" ;;\n\
               *) exit 9 ;;\n\
             esac ;;\n\
           *) exit 8 ;;\n\
         esac\n",
        trigger = shell_path(&trigger_path),
        operation_id = shell_path(&operation_id_path),
        cid = shell_path(&cid_path),
        staging = shell_path(&staging_path),
        payload = shell_path(&payload_path),
    );
    fs::write(&program, script)?;
    let mut permissions = fs::metadata(&program)?.permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&program, permissions)?;

    let instance_id = format!("cli-backup-event-{}", std::process::id());
    let logoscore_config = fixture.join("logoscore-config");
    fs::create_dir_all(logoscore_config.join("client"))?;
    fs::write(
        logoscore_config.join("client/config.json"),
        serde_json::to_vec(&json!({
            "instance_id": instance_id,
            "daemon": { "core_service": { "transport": "local" } }
        }))?,
    )?;
    let socket = std::env::temp_dir().join(format!("logos_core_service_{instance_id}"));
    fs::write(&socket, b"test socket identity")?;
    let _socket_cleanup = RemoveFileOnDrop(socket);
    let program_text = program.display().to_string();
    let config_text = logoscore_config.display().to_string();
    let envs = [
        ("LOGOSCORE_BIN", program_text.as_str()),
        ("LOGOSCORE_CONFIG_DIR", config_text.as_str()),
    ];

    let download = run_cli_with_env(
        directory.path(),
        &[
            "backup".to_owned(),
            "download".to_owned(),
            "cid-cli-event".to_owned(),
        ],
        &envs,
    )?;
    let catalog_id = download
        .get("backup_catalog_id")
        .and_then(Value::as_str)
        .context("event download omitted catalog ID")?
        .to_owned();
    if download.get("restored").and_then(Value::as_bool) != Some(false)
        || download
            .pointer("/catalog_entry/remote/cid")
            .and_then(Value::as_str)
            != Some("cid-cli-event")
    {
        bail!("event download bypassed catalog-only result: {download}");
    }
    let staged = Path::new(&fs::read_to_string(&staging_path)?).to_path_buf();
    if staged.exists() {
        bail!("event download left staged bytes at {}", staged.display());
    }
    assert_original_state(directory.path())?;

    let options =
        r#"{"app_settings":"replace","favorites":"replace","idl":"replace","wallet":"replace"}"#;
    let direct_apply = run_cli_output_with_env(
        directory.path(),
        &[
            "backup".to_owned(),
            "apply".to_owned(),
            "cid-cli-event".to_owned(),
            "--options".to_owned(),
            options.to_owned(),
        ],
        &envs,
    )?;
    let direct_result: Value = serde_json::from_slice(&direct_apply.stdout)
        .context("failed direct-CID apply did not preserve JSON stdout")?;
    if direct_apply.status.success()
        || direct_result.get("outcome").and_then(Value::as_str) != Some("rolled_back")
        || direct_result.get("terminal").and_then(Value::as_bool) != Some(true)
        || !direct_result
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("was not found"))
        || !String::from_utf8_lossy(&direct_apply.stderr)
            .contains("backup import ended with outcome `rolled_back`")
    {
        bail!(
            "CLI direct remote-CID apply contract drifted: status={}, stdout={}, stderr={}",
            direct_apply.status,
            String::from_utf8_lossy(&direct_apply.stdout),
            String::from_utf8_lossy(&direct_apply.stderr)
        );
    }
    assert_original_state(directory.path())?;

    let catalog = run_cli_with_env(
        directory.path(),
        &["backup".to_owned(), "list".to_owned()],
        &envs,
    )?;
    if catalog
        .get("entries")
        .and_then(Value::as_array)
        .is_none_or(|entries| entries.len() != 1)
    {
        bail!("event download did not record exactly one catalog entry: {catalog}");
    }
    let preview = run_cli_with_env(
        directory.path(),
        &[
            "backup".to_owned(),
            "preview".to_owned(),
            catalog_id.clone(),
            "--options".to_owned(),
            options.to_owned(),
        ],
        &envs,
    )?;
    if preview.get("outcome").and_then(Value::as_str) != Some("preview") {
        bail!("event-backed catalog preview failed: {preview}");
    }
    assert_original_state(directory.path())?;
    let applied = run_cli_with_env(
        directory.path(),
        &[
            "backup".to_owned(),
            "apply".to_owned(),
            catalog_id,
            "--options".to_owned(),
            options.to_owned(),
        ],
        &envs,
    )?;
    if applied.get("outcome").and_then(Value::as_str) != Some("applied") {
        bail!("event-backed catalog apply failed: {applied}");
    }
    assert_imported_state(directory.path())
}

#[cfg(target_os = "linux")]
#[test]
fn cli_backup_download_completion_wins_signal_during_catalog_commit() -> Result<()> {
    use nix::{sys::signal::Signal, sys::signal::kill, unistd::Pid};

    let directory = tempfile::tempdir()?;
    seed_original_state(directory.path())?;
    let lock_path = directory.path().join(".backup-catalog.lock");
    let catalog_lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    catalog_lock.lock()?;

    let payload = json!({
        "kind": "logos-inspector-settings-backup",
        "version": 1,
        "encrypted": false,
        "state": {
            "settings": {
                "version": 2,
                "theme": "new",
                "channel_source_configs": [],
                "favorites": [{ "value": "new-favorite" }]
            },
            "idls": {
                "version": 1,
                "idls": [{ "key": "idl-new", "json": "{}" }],
                "account_idl_selections": {}
            },
            "wallet": { "profile": { "label": "New wallet" } }
        }
    });
    let (endpoint, server) = one_response_server(serde_json::to_vec(&payload)?)?;
    let mut child = ChildOnDrop::new(
        Command::new(env!("CARGO_BIN_EXE_logos-inspector"))
            .env("LOGOS_INSPECTOR_CONFIG_DIR", directory.path())
            .args([
                "cli",
                "backup",
                "download",
                "cid-signal-commit",
                "--source-mode",
                "rest",
                "--rest-url",
                endpoint.as_str(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to start commit-race backup CLI")?,
    );
    let request = server
        .join()
        .map_err(|_| anyhow::anyhow!("commit-race HTTP fixture panicked"))??;
    anyhow::ensure!(
        request.starts_with("GET /data/cid-signal-commit/network/stream HTTP/1.1\r\n"),
        "commit-race CLI used unexpected Storage request: {request}"
    );

    let cli_pid = i32::try_from(child.child_mut()?.id()).context("CLI PID is too large")?;
    wait_for_process_open_file(cli_pid, &lock_path, Duration::from_secs(5))?;
    let signal_thread = find_cli_signal_thread(cli_pid, Duration::from_secs(5))?;
    kill(Pid::from_raw(cli_pid), Signal::SIGINT).context("failed to signal commit-race CLI")?;
    wait_for_process_thread_gone(cli_pid, signal_thread, Duration::from_secs(5))?;
    catalog_lock.unlock()?;

    wait_for_child_exit(&mut child, Duration::from_secs(8))?;
    let output = child.take()?.wait_with_output()?;
    anyhow::ensure!(
        output.status.success(),
        "completed commit lost to SIGINT: status={}, stdout={}, stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout)
        .context("completed commit did not preserve its JSON receipt")?;
    anyhow::ensure!(
        result.get("downloaded").and_then(Value::as_bool) == Some(true)
            && result.get("cid").and_then(Value::as_str) == Some("cid-signal-commit")
            && result
                .pointer("/catalog_entry/remote/cid")
                .and_then(Value::as_str)
                == Some("cid-signal-commit"),
        "completed commit returned an invalid receipt: {result}"
    );
    anyhow::ensure!(
        directory.path().join("backup_catalog.json").is_file(),
        "completed commit did not persist its catalog"
    );
    assert_original_state(directory.path())
}

#[cfg(target_os = "linux")]
#[test]
fn cli_backup_download_signals_settle_remote_cleanup_before_exit() -> Result<()> {
    use nix::sys::signal::Signal;

    for (signal, label) in [(Signal::SIGINT, "SIGINT"), (Signal::SIGTERM, "SIGTERM")] {
        assert_cli_backup_signal_cleanup(signal, label, true)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn cli_backup_download_signals_preserve_failed_cleanup_evidence() -> Result<()> {
    use nix::sys::signal::Signal;

    for (signal, label) in [(Signal::SIGINT, "SIGINT"), (Signal::SIGTERM, "SIGTERM")] {
        assert_cli_backup_signal_cleanup(signal, label, false)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn assert_cli_backup_signal_cleanup(
    signal: nix::sys::signal::Signal,
    label: &str,
    cancel_should_settle: bool,
) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    use nix::{sys::signal::kill, unistd::Pid};

    let directory = tempfile::tempdir()?;
    seed_original_state(directory.path())?;
    let cleanup_outcome = if cancel_should_settle {
        "settled"
    } else {
        "unconfirmed"
    };
    let fixture = directory.path().join(format!(
        "signal-fixture-{}-{cleanup_outcome}",
        label.to_ascii_lowercase()
    ));
    fs::create_dir_all(&fixture)?;
    let started = fixture.join("download-started");
    let cancel_attempted = fixture.join("cancel-attempted");
    let cancel_settled = fixture.join("cancel-settled");
    let cancel_failure = fixture.join("cancel-failure");
    let remote_active = fixture.join("remote-active");
    let watch_stopped = fixture.join("watch-stopped");
    let staging_path = fixture.join("staging-path");
    let operation_id_path = fixture.join("operation-id");
    let cid_path = fixture.join("cid");
    let watch_pid_path = fixture.join("watch.pid");
    let descendant_pid_path = fixture.join("watch-descendant.pid");
    let trace_path = fixture.join("logoscore.trace");
    let program = fixture.join("logoscore-test");
    if !cancel_should_settle {
        fs::write(&cancel_failure, b"fail cancellation")?;
    }
    let script = format!(
        "#!/bin/sh\n\
         printf '%s\\n' \"$*\" >> {trace}\n\
         if [ \"$1\" = \"--config-dir\" ]; then shift 2; fi\n\
         case \"$1\" in\n\
           list-modules) printf '%s\\n' '[{{\"name\":\"storage_module\",\"status\":\"loaded\"}}]' ;;\n\
           module-info) printf '%s\\n' '{{\"name\":\"storage_module\",\"methods\":[{{\"isInvokable\":true,\"name\":\"downloadProtocol\",\"signature\":\"downloadProtocol()\"}},{{\"isInvokable\":true,\"name\":\"downloadToUrlV2\",\"signature\":\"downloadToUrlV2(QString,QString,bool,int,QString,int)\"}},{{\"isInvokable\":true,\"name\":\"downloadCancelV2\",\"signature\":\"downloadCancelV2(QString)\"}}],\"events\":[{{\"name\":\"storageDownloadDoneV2\",\"signature\":\"storageDownloadDoneV2(QString)\"}}]}}' ;;\n\
           watch)\n\
             printf '%s' \"$$\" > {watch_pid}\n\
             (trap '' TERM INT; while :; do sleep 1; done) &\n\
             printf '%s' \"$!\" > {descendant_pid}\n\
             trap 'touch {watch_stopped}; exit 0' TERM INT\n\
             printf '%s\\n' '{{\"type\":\"subscription_ready\",\"protocol\":\"logoscore.watch\",\"version\":1,\"module\":\"storage_module\",\"event\":\"storageDownloadDoneV2\"}}'\n\
             while :; do sleep 1; done ;;\n\
           call)\n\
             case \"$3\" in\n\
               downloadProtocol) printf '%s\\n' '{{\"status\":\"ok\",\"result\":{{\"success\":true,\"value\":{{\"protocol\":\"logos.storage.download\",\"version\":2,\"moduleOperationIdOwner\":\"caller\",\"cancelTimeoutMs\":15000,\"maxDownloadBytes\":1073741824}},\"error\":null}}}}' ;;\n\
               downloadToUrlV2)\n\
                 printf '%s' \"$5\" > {staging}\n\
                 printf '{{}}' > \"$5\"\n\
                 printf '%s' \"$8\" > {operation_id}\n\
                 printf '%s' \"$4\" > {cid}\n\
                 touch {remote_active} {started}\n\
                 printf '{{\"status\":\"ok\",\"result\":{{\"success\":true,\"value\":{{\"protocol\":\"logos.storage.download\",\"version\":2,\"accepted\":true,\"moduleOperationId\":\"%s\",\"cid\":\"%s\"}},\"error\":null}}}}\\n' \"$8\" \"$4\" ;;\n\
               downloadCancelV2)\n\
                 touch {cancel_attempted}\n\
                 if [ -f {cancel_failure} ]; then exit 10; fi\n\
                 rm -f {remote_active}\n\
                 touch {cancel_settled}\n\
                 cid=$(cat {cid})\n\
                 printf '{{\"status\":\"ok\",\"result\":{{\"success\":true,\"value\":{{\"protocol\":\"logos.storage.download\",\"version\":2,\"moduleOperationId\":\"%s\",\"cid\":\"%s\",\"cancelStatus\":\"canceled\"}},\"error\":null}}}}\\n' \"$4\" \"$cid\" ;;\n\
               *) exit 9 ;;\n\
             esac ;;\n\
           *) exit 8 ;;\n\
         esac\n",
        watch_pid = shell_path(&watch_pid_path),
        descendant_pid = shell_path(&descendant_pid_path),
        watch_stopped = shell_path(&watch_stopped),
        staging = shell_path(&staging_path),
        operation_id = shell_path(&operation_id_path),
        cid = shell_path(&cid_path),
        remote_active = shell_path(&remote_active),
        started = shell_path(&started),
        cancel_attempted = shell_path(&cancel_attempted),
        cancel_settled = shell_path(&cancel_settled),
        cancel_failure = shell_path(&cancel_failure),
        trace = shell_path(&trace_path),
    );
    fs::write(&program, script)?;
    let mut permissions = fs::metadata(&program)?.permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&program, permissions)?;

    let instance_id = format!(
        "cli-backup-signal-{}-{cleanup_outcome}-{}",
        label.to_ascii_lowercase(),
        std::process::id()
    );
    let logoscore_config = fixture.join("logoscore-config");
    fs::create_dir_all(logoscore_config.join("client"))?;
    fs::write(
        logoscore_config.join("client/config.json"),
        serde_json::to_vec(&json!({
            "instance_id": instance_id,
            "daemon": { "core_service": { "transport": "local" } }
        }))?,
    )?;
    let socket = std::env::temp_dir().join(format!("logos_core_service_{instance_id}"));
    fs::write(&socket, b"test socket identity")?;
    let _socket_cleanup = RemoveFileOnDrop(socket);
    let _watch_cleanup = KillProcessGroupOnDrop(watch_pid_path.clone());
    let mut child = ChildOnDrop::new(
        Command::new(env!("CARGO_BIN_EXE_logos-inspector"))
            .env("LOGOS_INSPECTOR_CONFIG_DIR", directory.path())
            .env("LOGOSCORE_BIN", &program)
            .env("LOGOSCORE_CONFIG_DIR", &logoscore_config)
            .args(["cli", "backup", "download", "cid-signal-test"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to start signal-aware backup CLI")?,
    );

    if let Err(wait_error) = wait_for_path(&started, Duration::from_secs(5)) {
        if child.child_mut()?.try_wait()?.is_some() {
            let output = child.take()?.wait_with_output()?;
            bail!(
                "backup CLI exited before dispatch after {wait_error}: status={}, stdout={}, stderr={}, trace={}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
                fs::read_to_string(&trace_path).unwrap_or_else(|error| format!("<{error}>"))
            );
        }
        return Err(wait_error);
    }
    wait_for_path(&watch_pid_path, Duration::from_secs(5))?;
    wait_for_path(&descendant_pid_path, Duration::from_secs(5))?;
    let cli_pid = i32::try_from(child.child_mut()?.id()).context("CLI PID is too large")?;
    kill(Pid::from_raw(cli_pid), signal).context("failed to signal backup CLI")?;
    let signaled_at = Instant::now();
    wait_for_child_exit(&mut child, Duration::from_secs(8))?;
    let output = child.take()?.wait_with_output()?;

    anyhow::ensure!(
        !output.status.success(),
        "{label} backup CLI unexpectedly reported success"
    );
    anyhow::ensure!(
        signaled_at.elapsed() < Duration::from_secs(8),
        "{label} backup CLI exceeded bounded shutdown"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::ensure!(
        stderr.contains(label),
        "{label} backup CLI lost signal-specific shutdown evidence: {}",
        stderr
    );
    if cancel_should_settle {
        anyhow::ensure!(
            cancel_attempted.exists() && cancel_settled.exists() && !remote_active.exists(),
            "{label} backup CLI did not settle the active remote download"
        );
    } else {
        anyhow::ensure!(
            cancel_attempted.exists() && !cancel_settled.exists() && remote_active.exists(),
            "{label} failing-cancel fixture reported false remote settlement"
        );
        anyhow::ensure!(
            stderr.contains("cleanup remains unconfirmed")
                && stderr.contains("storage download cleanup was not confirmed")
                && stderr.contains("cancel=")
                && stderr.contains("watch=ok"),
            "{label} backup CLI hid failed cleanup evidence: {stderr}"
        );
    }
    anyhow::ensure!(
        watch_stopped.exists(),
        "{label} backup CLI did not gracefully stop the event watch"
    );
    let staged = Path::new(&fs::read_to_string(&staging_path)?).to_path_buf();
    anyhow::ensure!(
        !staged.exists(),
        "{label} backup CLI left staging file {}",
        staged.display()
    );
    let watch_pid = read_pid(&watch_pid_path)?;
    let descendant_pid = read_pid(&descendant_pid_path)?;
    wait_for_process_gone(watch_pid, Duration::from_secs(2))?;
    wait_for_process_gone(descendant_pid, Duration::from_secs(2))?;
    wait_for_process_group_gone(watch_pid, Duration::from_secs(2))?;
    anyhow::ensure!(
        !directory.path().join("backup_catalog.json").exists()
            && !directory.path().join("backup-payloads").exists(),
        "{label} backup CLI committed catalog state after interruption"
    );
    assert_original_state(directory.path())
}

fn run_cli(config_dir: &Path, args: &[String]) -> Result<Value> {
    run_cli_with_env(config_dir, args, &[])
}

fn encrypted_backup_payload(plain: &Value, wallet_config: &[u8]) -> Result<Value> {
    const KIND: &str = "logos-inspector-settings-backup";
    const SCHEME: &str = "xchacha20poly1305-wallet-config-v1";
    let salt = [7_u8; 16];
    let nonce = [9_u8; 24];
    let mut material = Vec::with_capacity(KIND.len() + 1 + wallet_config.len());
    material.extend_from_slice(KIND.as_bytes());
    material.push(0);
    material.extend_from_slice(wallet_config);
    let hkdf = Hkdf::<Sha256>::new(Some(&salt), &material);
    let mut key = [0_u8; 32];
    hkdf.expand(b"logos inspector settings backup wallet key", &mut key)
        .map_err(|_| anyhow::anyhow!("failed to derive encrypted test backup key"))?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .context("failed to construct encrypted test backup cipher")?;
    let plaintext = serde_json::to_vec(plain)?;
    let aad = format!("{KIND}:1:{SCHEME}");
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("failed to encrypt test backup"))?;
    Ok(json!({
        "kind": KIND,
        "version": 1,
        "created_at": "1",
        "encrypted": true,
        "encryption": {
            "scheme": SCHEME,
            "salt": BASE64_STANDARD.encode(salt),
            "nonce": BASE64_STANDARD.encode(nonce),
            "key_source": "wallet_config"
        },
        "ciphertext": BASE64_STANDARD.encode(ciphertext)
    }))
}

fn run_cli_with_env(config_dir: &Path, args: &[String], envs: &[(&str, &str)]) -> Result<Value> {
    let output = run_cli_output_with_env(config_dir, args, envs)?;
    if !output.status.success() {
        bail!(
            "CLI failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "CLI stdout did not contain JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn run_cli_output_with_env(
    config_dir: &Path,
    args: &[String],
    envs: &[(&str, &str)],
) -> Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_logos-inspector"))
        .env("LOGOS_INSPECTOR_CONFIG_DIR", config_dir)
        .envs(envs.iter().copied())
        .arg("cli")
        .args(args)
        .output()
        .context("failed to run logos-inspector CLI")
}

fn one_response_server(body: Vec<u8>) -> Result<(String, thread::JoinHandle<Result<String>>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let server = thread::spawn(move || -> Result<String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        bail!("timed out waiting for CLI backup HTTP request");
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error.into()),
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let request = read_headers(&mut stream)?;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )?;
        stream.write_all(&body)?;
        Ok(request)
    });
    Ok((endpoint, server))
}

fn read_headers(stream: &mut TcpStream) -> Result<String> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1024];
    while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(
            chunk
                .get(..count)
                .context("CLI backup HTTP request chunk was invalid")?,
        );
        if bytes.len() > 64 * 1024 {
            bail!("CLI backup HTTP request headers were too large");
        }
    }
    String::from_utf8(bytes).context("CLI backup HTTP request was not UTF-8")
}

fn seed_original_state(base_dir: &Path) -> Result<()> {
    fs::write(base_dir.join("settings.json"), OLD_SETTINGS)?;
    fs::write(base_dir.join("idls.json"), OLD_IDLS)?;
    fs::write(base_dir.join("wallet.json"), OLD_WALLET)?;
    Ok(())
}

fn assert_original_state(base_dir: &Path) -> Result<()> {
    for (name, expected) in [
        ("settings.json", OLD_SETTINGS),
        ("idls.json", OLD_IDLS),
        ("wallet.json", OLD_WALLET),
    ] {
        let actual = fs::read(base_dir.join(name))?;
        if actual != expected {
            bail!("remote download or preview modified {name}");
        }
    }
    Ok(())
}

fn assert_imported_state(base_dir: &Path) -> Result<()> {
    let settings: Value = serde_json::from_slice(&fs::read(base_dir.join("settings.json"))?)?;
    let idls: Value = serde_json::from_slice(&fs::read(base_dir.join("idls.json"))?)?;
    let wallet: Value = serde_json::from_slice(&fs::read(base_dir.join("wallet.json"))?)?;
    if settings.get("theme").and_then(Value::as_str) != Some("new")
        || settings
            .pointer("/favorites/0/value")
            .and_then(Value::as_str)
            != Some("new-favorite")
        || idls.pointer("/idls/0/key").and_then(Value::as_str) != Some("idl-new")
        || wallet.pointer("/profile/label").and_then(Value::as_str) != Some("New wallet")
    {
        bail!("explicit CLI apply did not persist selected backup state");
    }
    Ok(())
}

#[cfg(unix)]
fn shell_path(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\"'\"'"))
}

#[cfg(target_os = "linux")]
fn wait_for_path(path: &Path, timeout: Duration) -> Result<()> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("test path deadline overflow")?;
    while !path.exists() {
        if Instant::now() >= deadline {
            bail!("timed out waiting for {}", path.display());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn wait_for_child_exit(child: &mut ChildOnDrop, timeout: Duration) -> Result<()> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("test child deadline overflow")?;
    loop {
        if child.child_mut()?.try_wait()?.is_some() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for signaled backup CLI");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "linux")]
fn wait_for_process_open_file(pid: i32, path: &Path, timeout: Duration) -> Result<()> {
    let expected = fs::canonicalize(path)?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("test open-file deadline overflow")?;
    loop {
        let directory = fs::read_dir(format!("/proc/{pid}/fd"))?;
        for entry in directory {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            let target = match fs::read_link(entry.path()) {
                Ok(target) => target,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if target == expected {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            bail!("PID {pid} did not open {}", path.display());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "linux")]
fn find_cli_signal_thread(pid: i32, timeout: Duration) -> Result<i32> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("test signal-thread deadline overflow")?;
    loop {
        for entry in fs::read_dir(format!("/proc/{pid}/task"))? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            let thread_id = entry
                .file_name()
                .to_string_lossy()
                .parse::<i32>()
                .context("process task directory did not contain a thread ID")?;
            let name = match fs::read_to_string(entry.path().join("comm")) {
                Ok(name) => name,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if thread_id != pid && name.trim() == "logos-inspector" {
                return Ok(thread_id);
            }
        }
        if Instant::now() >= deadline {
            bail!("PID {pid} did not expose its CLI signal monitor thread");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "linux")]
fn wait_for_process_thread_gone(pid: i32, thread_id: i32, timeout: Duration) -> Result<()> {
    let task_path = std::path::PathBuf::from(format!("/proc/{pid}/task/{thread_id}"));
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("test signal-thread exit deadline overflow")?;
    while task_path.exists() {
        if Instant::now() >= deadline {
            bail!("CLI signal monitor thread {thread_id} did not stop");
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn read_pid(path: &Path) -> Result<i32> {
    fs::read_to_string(path)?
        .trim()
        .parse::<i32>()
        .with_context(|| format!("{} did not contain a PID", path.display()))
}

#[cfg(target_os = "linux")]
fn wait_for_process_gone(pid: i32, timeout: Duration) -> Result<()> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("test process deadline overflow")?;
    let status_path = std::path::PathBuf::from(format!("/proc/{pid}/stat"));
    loop {
        let live = match fs::read_to_string(&status_path) {
            Ok(status) => status
                .rsplit_once(')')
                .and_then(|(_, fields)| fields.split_whitespace().next())
                .is_some_and(|state| state != "Z"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error).context("failed to inspect cleanup process"),
        };
        if !live {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("cleanup left PID {pid} running");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "linux")]
fn wait_for_process_group_gone(process_group: i32, timeout: Duration) -> Result<()> {
    use nix::{errno::Errno, sys::signal::killpg, unistd::Pid};

    let deadline = Instant::now()
        .checked_add(timeout)
        .context("test process-group deadline overflow")?;
    loop {
        match killpg(Pid::from_raw(process_group), None) {
            Err(Errno::ESRCH) => return Ok(()),
            Ok(()) => {}
            Err(error) => return Err(error).context("failed to inspect cleanup process group"),
        }
        if Instant::now() >= deadline {
            bail!("cleanup left process group {process_group} running");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "linux")]
struct ChildOnDrop(Option<Child>);

#[cfg(target_os = "linux")]
impl ChildOnDrop {
    const fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn child_mut(&mut self) -> Result<&mut Child> {
        self.0
            .as_mut()
            .context("backup CLI child was already consumed")
    }

    fn take(&mut self) -> Result<Child> {
        self.0
            .take()
            .context("backup CLI child was already consumed")
    }
}

#[cfg(target_os = "linux")]
impl Drop for ChildOnDrop {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _kill_result = child.kill();
            let _wait_result = child.wait();
        }
    }
}

#[cfg(target_os = "linux")]
struct KillProcessGroupOnDrop(std::path::PathBuf);

#[cfg(target_os = "linux")]
impl Drop for KillProcessGroupOnDrop {
    fn drop(&mut self) {
        use nix::{sys::signal::Signal, unistd::Pid};

        if let Ok(process_group) = read_pid(&self.0) {
            let _cleanup_result =
                nix::sys::signal::killpg(Pid::from_raw(process_group), Signal::SIGKILL);
        }
    }
}

struct RemoveFileOnDrop(std::path::PathBuf);

impl Drop for RemoveFileOnDrop {
    fn drop(&mut self) {
        let _result = fs::remove_file(&self.0);
    }
}
