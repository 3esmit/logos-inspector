use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Output},
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

use crate::support::command_runner::{
    CommandControl, CommandRunPolicy, run_command, run_command_controlled,
};

use super::{action_engine::LocalNodeActionEngine, process::find_command};

const INDEXER_PACKAGE_NAME: &str = "lez_indexer_module";
const INDEXER_PACKAGE_TYPE: &str = "core";
const OFFICIAL_REPOSITORY_NAME: &str = "logos-modules-official";
const OFFICIAL_REPOSITORY_URL: &str = "https://raw.githubusercontent.com/logos-co/logos-modules-release/refs/heads/main/logos-repo.json";
const OFFICIAL_DOWNLOAD_PATH_PREFIX: &str = "/logos-co/logos-modules-release/releases/download/";
const DEFAULT_MODULES_DIR: &str = "/opt/logos-node/modules";
const PACKAGE_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);
const PACKAGE_CATALOG_TIMEOUT: Duration = Duration::from_secs(30);
const PACKAGE_OUTPUT_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LocalNodePackageCatalogReport {
    pub modules_dir: String,
    pub package: LocalNodePackageCatalogEntry,
    pub installed: Option<LocalNodeInstalledPackageReport>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LocalNodePackageCatalogEntry {
    pub name: String,
    pub description: String,
    pub package_type: String,
    pub category: String,
    pub repository_name: String,
    pub repository_display_name: String,
    pub repository_url: String,
    pub versions: Vec<LocalNodePackageRelease>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LocalNodePackageRelease {
    pub version: String,
    pub released_at: String,
    pub root_hash: String,
    pub sha256: String,
    pub size: u64,
    pub url: String,
    pub publisher_ref: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LocalNodeInstalledPackageReport {
    pub name: String,
    pub version: String,
    pub description: String,
    pub package_type: String,
    pub category: String,
    pub author: String,
    pub install_type: String,
    pub install_dir: String,
    pub main_file_path: String,
    pub root_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DownloadedLocalNodePackage {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) root_hash: String,
    pub(crate) size: u64,
    pub(crate) file_path: PathBuf,
}

#[derive(Debug, Clone)]
struct PackageToolchain {
    lgpd: Option<PathBuf>,
    lgpm: Option<PathBuf>,
}

impl PackageToolchain {
    fn system() -> Self {
        Self {
            lgpd: find_command("lgpd").map(PathBuf::from),
            lgpm: find_command("lgpm").map(PathBuf::from),
        }
    }

    fn lgpd(&self) -> Result<&Path> {
        self.lgpd
            .as_deref()
            .context("lgpd is required to query or download Logos packages")
    }

    fn lgpm(&self) -> Result<&Path> {
        self.lgpm
            .as_deref()
            .context("lgpm is required to install Logos packages")
    }

    fn info_command(&self) -> Result<Command> {
        let mut command = Command::new(self.lgpd()?);
        command.arg("info").arg(INDEXER_PACKAGE_NAME).arg("--json");
        Ok(command)
    }

    fn installed_command(&self, modules_dir: &Path) -> Result<Command> {
        let mut command = Command::new(self.lgpm()?);
        command
            .arg("--modules-dir")
            .arg(modules_dir)
            .arg("list")
            .arg("--json");
        Ok(command)
    }

    fn download_command(
        &self,
        release: &LocalNodePackageRelease,
        output_dir: &Path,
    ) -> Result<Command> {
        let mut command = Command::new(self.lgpd()?);
        command
            .arg("--version")
            .arg(&release.version)
            .arg("--root-hash")
            .arg(&release.root_hash)
            .arg("--output")
            .arg(output_dir)
            .arg("download")
            .arg(INDEXER_PACKAGE_NAME);
        Ok(command)
    }

    fn install_command(&self, package_path: &Path, modules_dir: &Path) -> Result<Command> {
        let mut command = Command::new(self.lgpm()?);
        command
            .arg("--modules-dir")
            .arg(modules_dir)
            .arg("install")
            .arg("--file")
            .arg(package_path);
        Ok(command)
    }
}

pub(super) fn local_node_package_catalog(
    requested_modules_dir: Option<&str>,
) -> Result<LocalNodePackageCatalogReport> {
    let modules_dir = resolve_modules_dir(requested_modules_dir)?;
    let toolchain = PackageToolchain::system();
    let package = query_catalog(&toolchain)?;
    let installed = if toolchain.lgpm.is_some() {
        query_installed(&toolchain, &modules_dir)?
    } else {
        None
    };
    Ok(LocalNodePackageCatalogReport {
        modules_dir: modules_dir.display().to_string(),
        package,
        installed,
    })
}

pub(crate) fn download_official_indexer_module(
    release: &LocalNodePackageRelease,
    output_dir: &Path,
    control: CommandControl,
) -> Result<DownloadedLocalNodePackage> {
    download_official_indexer_module_with(&PackageToolchain::system(), release, output_dir, control)
}

pub(crate) fn install_official_indexer_module(
    package: &DownloadedLocalNodePackage,
    modules_dir: &Path,
    control: CommandControl,
) -> Result<LocalNodeInstalledPackageReport> {
    install_official_indexer_module_with(&PackageToolchain::system(), package, modules_dir, control)
}

fn query_catalog(toolchain: &PackageToolchain) -> Result<LocalNodePackageCatalogEntry> {
    let output = run_package_command(
        toolchain.info_command()?,
        "lgpd info lez_indexer_module",
        PACKAGE_CATALOG_TIMEOUT,
    )?;
    parse_catalog(&output.stdout)
}

fn query_installed(
    toolchain: &PackageToolchain,
    modules_dir: &Path,
) -> Result<Option<LocalNodeInstalledPackageReport>> {
    let output = run_package_command(
        toolchain.installed_command(modules_dir)?,
        "lgpm list",
        PACKAGE_CATALOG_TIMEOUT,
    )?;
    let installed = parse_installed(&output.stdout, modules_dir)?;
    Ok(installed.filter(|installed| validate_installed_artifact(installed, modules_dir).is_ok()))
}

fn download_official_indexer_module_with(
    toolchain: &PackageToolchain,
    release: &LocalNodePackageRelease,
    output_dir: &Path,
    control: CommandControl,
) -> Result<DownloadedLocalNodePackage> {
    validate_release(release)?;
    validate_absolute_directory(output_dir, "package download directory", true)?;
    run_package_command_controlled(
        toolchain.download_command(release, output_dir)?,
        "lgpd download lez_indexer_module",
        control,
    )?;

    let file_path = output_dir.join(package_filename(&release.version));
    let metadata = fs::metadata(&file_path).with_context(|| {
        format!(
            "lgpd did not create expected package `{}`",
            file_path.display()
        )
    })?;
    if !metadata.is_file() {
        bail!(
            "lgpd package output `{}` is not a regular file",
            file_path.display()
        );
    }
    if metadata.len() != release.size {
        bail!(
            "downloaded package size {} does not match catalog size {}",
            metadata.len(),
            release.size
        );
    }
    Ok(DownloadedLocalNodePackage {
        name: INDEXER_PACKAGE_NAME.to_owned(),
        version: release.version.clone(),
        root_hash: release.root_hash.clone(),
        size: release.size,
        file_path,
    })
}

fn install_official_indexer_module_with(
    toolchain: &PackageToolchain,
    package: &DownloadedLocalNodePackage,
    modules_dir: &Path,
    control: CommandControl,
) -> Result<LocalNodeInstalledPackageReport> {
    validate_downloaded_package(package)?;
    validate_absolute_directory(modules_dir, "Logos modules directory", false)?;
    run_package_command_controlled(
        toolchain.install_command(&package.file_path, modules_dir)?,
        "lgpm install lez_indexer_module",
        control.clone(),
    )?;
    let output = run_package_command_controlled(
        toolchain.installed_command(modules_dir)?,
        "lgpm list",
        control,
    )?;
    let installed = parse_installed(&output.stdout, modules_dir)?
        .context("lgpm completed but lez_indexer_module is not installed")?;
    if installed.version != package.version || installed.root_hash != package.root_hash {
        bail!("installed lez_indexer_module identity does not match downloaded package");
    }
    validate_installed_artifact(&installed, modules_dir)?;
    Ok(installed)
}

fn run_package_command(mut command: Command, label: &str, timeout: Duration) -> Result<Output> {
    clear_untrusted_package_environment(&mut command);
    run_command(
        command,
        CommandRunPolicy {
            label,
            timeout,
            poll_interval: PACKAGE_COMMAND_POLL_INTERVAL,
            redactions: &[],
            output_limit: PACKAGE_OUTPUT_LIMIT,
        },
    )
}

fn run_package_command_controlled(
    mut command: Command,
    label: &str,
    control: CommandControl,
) -> Result<Output> {
    clear_untrusted_package_environment(&mut command);
    run_command_controlled(
        command,
        CommandRunPolicy {
            label,
            timeout: Duration::ZERO,
            poll_interval: PACKAGE_COMMAND_POLL_INTERVAL,
            redactions: &[],
            output_limit: PACKAGE_OUTPUT_LIMIT,
        },
        control,
    )
}

fn clear_untrusted_package_environment(command: &mut Command) {
    command.env_remove("LGPD_CONFIG");
    command.env_remove("LGPD_REPOSITORY");
    command.env_remove("LGPM_MODULES_DIR");
    command.env_remove("LGPM_UI_PLUGINS_DIR");
}

fn resolve_modules_dir(requested: Option<&str>) -> Result<PathBuf> {
    let requested = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let configured = if requested.is_some() {
        requested
    } else {
        LocalNodeActionEngine::system()?
            .runtime_profile()?
            .and_then(|profile| profile.modules_dir)
            .filter(|value| !value.trim().is_empty())
    }
    .unwrap_or_else(|| DEFAULT_MODULES_DIR.to_owned());
    canonical_modules_dir(Path::new(&configured))
}

pub(super) fn canonical_modules_dir(path: &Path) -> Result<PathBuf> {
    validate_absolute_directory(path, "Logos modules directory", false)?;
    if !path.exists() {
        return Ok(path.to_path_buf());
    }
    if !path.is_dir() {
        bail!(
            "Logos modules directory `{}` is not a directory",
            path.display()
        );
    }
    fs::canonicalize(path).with_context(|| {
        format!(
            "failed to resolve Logos modules directory `{}`",
            path.display()
        )
    })
}

pub(super) fn installed_package_modules_dir(
    installed: &LocalNodeInstalledPackageReport,
) -> Result<PathBuf> {
    let install_dir = Path::new(&installed.install_dir);
    if install_dir.file_name().and_then(|value| value.to_str()) != Some(INDEXER_PACKAGE_NAME) {
        bail!("installed lez_indexer_module directory has an unexpected package name");
    }
    let modules_dir = install_dir
        .parent()
        .context("installed lez_indexer_module directory has no modules directory")?;
    canonical_modules_dir(modules_dir)
}

pub(super) fn package_path_modules_dir(package_path: &str) -> Option<PathBuf> {
    let package_path = fs::canonicalize(Path::new(package_path)).ok()?;
    if !package_path.is_file() {
        return None;
    }
    let install_dir = package_path.ancestors().find(|ancestor| {
        ancestor.file_name().and_then(|value| value.to_str()) == Some(INDEXER_PACKAGE_NAME)
    })?;
    install_dir.parent().map(Path::to_path_buf)
}

fn validate_installed_artifact(
    installed: &LocalNodeInstalledPackageReport,
    modules_dir: &Path,
) -> Result<()> {
    let modules_dir = canonical_modules_dir(modules_dir)?;
    let expected_install_dir = modules_dir.join(INDEXER_PACKAGE_NAME);
    let install_dir = fs::canonicalize(&installed.install_dir).with_context(|| {
        format!(
            "installed lez_indexer_module directory `{}` is unavailable",
            installed.install_dir
        )
    })?;
    if !install_dir.is_dir() || install_dir != expected_install_dir {
        bail!("installed lez_indexer_module directory does not match configured modules directory");
    }
    let main_file_path = fs::canonicalize(&installed.main_file_path).with_context(|| {
        format!(
            "installed lez_indexer_module main file `{}` is unavailable",
            installed.main_file_path
        )
    })?;
    if !main_file_path.is_file()
        || main_file_path == install_dir
        || !main_file_path.starts_with(&install_dir)
    {
        bail!(
            "installed lez_indexer_module main file is not a regular file in its install directory"
        );
    }
    Ok(())
}

fn validate_absolute_directory(path: &Path, label: &str, must_exist: bool) -> Result<()> {
    if !path.is_absolute() {
        bail!("{label} must be an absolute path");
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        bail!("{label} must not contain relative path components");
    }
    if !path
        .components()
        .any(|component| matches!(component, Component::Normal(_)))
    {
        bail!("{label} must not be the filesystem root");
    }
    if must_exist && !path.is_dir() {
        bail!("{label} `{}` is not a directory", path.display());
    }
    Ok(())
}

fn validate_release(release: &LocalNodePackageRelease) -> Result<()> {
    validate_version(&release.version)?;
    validate_hash(&release.root_hash, "package root hash")?;
    validate_hash(&release.sha256, "package SHA-256")?;
    if release.size == 0 {
        bail!("package size must be positive");
    }
    let url = url::Url::parse(&release.url).context("package download URL is invalid")?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.path().starts_with(OFFICIAL_DOWNLOAD_PATH_PREFIX)
        || !url
            .path()
            .ends_with(&format!("/{}", package_filename(&release.version)))
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("package download URL is not an official Logos release artifact");
    }
    Ok(())
}

fn validate_downloaded_package(package: &DownloadedLocalNodePackage) -> Result<()> {
    if package.name != INDEXER_PACKAGE_NAME {
        bail!("only lez_indexer_module may be installed through this package flow");
    }
    validate_version(&package.version)?;
    validate_hash(&package.root_hash, "package root hash")?;
    if package.size == 0 {
        bail!("package size must be positive");
    }
    if !package.file_path.is_absolute() || !package.file_path.is_file() {
        bail!("downloaded package path must be an absolute regular file");
    }
    if package.file_path.file_name().and_then(|name| name.to_str())
        != Some(package_filename(&package.version).as_str())
    {
        bail!("downloaded package filename does not match package version");
    }
    if fs::metadata(&package.file_path)?.len() != package.size {
        bail!("downloaded package size changed before installation");
    }
    Ok(())
}

fn validate_version(version: &str) -> Result<()> {
    if version.is_empty()
        || version.len() > 128
        || version.starts_with('-')
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+' | b'_'))
    {
        bail!("package version contains unsupported characters");
    }
    Ok(())
}

fn validate_hash(value: &str, label: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("{label} must contain 64 hexadecimal characters");
    }
    Ok(())
}

fn package_filename(version: &str) -> String {
    format!("{INDEXER_PACKAGE_NAME}-{version}.lgx")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCatalogPackage {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "type")]
    package_type: String,
    #[serde(default)]
    category: String,
    repository_name: String,
    #[serde(default)]
    repository_display_name: String,
    repository_url: String,
    versions: Vec<RawCatalogRelease>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCatalogRelease {
    manifest: RawCatalogManifest,
    #[serde(default)]
    publisher_ref: String,
    #[serde(default)]
    released_at: String,
    root_hash: String,
    sha256: String,
    size: u64,
    url: String,
}

#[derive(Debug, Deserialize)]
struct RawCatalogManifest {
    name: String,
    #[serde(rename = "type")]
    package_type: String,
    version: String,
    hashes: RawPackageHashes,
}

#[derive(Debug, Default, Deserialize)]
struct RawPackageHashes {
    #[serde(default)]
    root: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawInstalledPackage {
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "type")]
    package_type: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    install_type: String,
    install_dir: String,
    main_file_path: String,
    #[serde(default)]
    hashes: RawPackageHashes,
}

fn parse_catalog(bytes: &[u8]) -> Result<LocalNodePackageCatalogEntry> {
    let raw: RawCatalogPackage = parse_json(bytes, "lgpd info")?;
    if raw.name != INDEXER_PACKAGE_NAME
        || raw.package_type != INDEXER_PACKAGE_TYPE
        || raw.repository_name != OFFICIAL_REPOSITORY_NAME
        || raw.repository_url != OFFICIAL_REPOSITORY_URL
    {
        bail!("lgpd returned a non-official lez_indexer_module catalog entry");
    }
    if raw.versions.is_empty() {
        bail!("official lez_indexer_module has no available versions");
    }
    let versions = raw
        .versions
        .into_iter()
        .map(|raw_release| {
            if raw_release.manifest.name != INDEXER_PACKAGE_NAME
                || raw_release.manifest.package_type != INDEXER_PACKAGE_TYPE
                || raw_release.manifest.version.is_empty()
                || raw_release.manifest.hashes.root != raw_release.root_hash
            {
                bail!("lgpd release manifest does not match its catalog identity");
            }
            let release = LocalNodePackageRelease {
                version: raw_release.manifest.version,
                released_at: raw_release.released_at,
                root_hash: raw_release.root_hash,
                sha256: raw_release.sha256,
                size: raw_release.size,
                url: raw_release.url,
                publisher_ref: raw_release.publisher_ref,
            };
            validate_release(&release)?;
            Ok(release)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(LocalNodePackageCatalogEntry {
        name: raw.name,
        description: raw.description,
        package_type: raw.package_type,
        category: raw.category,
        repository_name: raw.repository_name,
        repository_display_name: raw.repository_display_name,
        repository_url: raw.repository_url,
        versions,
    })
}

fn parse_installed(
    bytes: &[u8],
    modules_dir: &Path,
) -> Result<Option<LocalNodeInstalledPackageReport>> {
    let text = std::str::from_utf8(bytes).context("lgpm list output is not UTF-8")?;
    if text.trim() == "No installed modules found" {
        return Ok(None);
    }
    let installed: Vec<RawInstalledPackage> = parse_json(bytes, "lgpm list")?;
    let mut matches = installed
        .into_iter()
        .filter(|package| package.name == INDEXER_PACKAGE_NAME);
    let Some(raw) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        bail!("lgpm returned duplicate lez_indexer_module installations");
    }
    if raw.package_type != INDEXER_PACKAGE_TYPE {
        bail!("installed lez_indexer_module is not a core package");
    }
    validate_version(&raw.version)?;
    validate_hash(&raw.hashes.root, "installed package root hash")?;
    let expected_install_dir = modules_dir.join(INDEXER_PACKAGE_NAME);
    let install_dir = Path::new(&raw.install_dir);
    let main_file_path = Path::new(&raw.main_file_path);
    if install_dir != expected_install_dir {
        bail!("installed lez_indexer_module is outside configured modules directory");
    }
    if !main_file_path.is_absolute()
        || main_file_path == install_dir
        || !main_file_path.starts_with(install_dir)
    {
        bail!("installed lez_indexer_module main file is outside its install directory");
    }
    Ok(Some(LocalNodeInstalledPackageReport {
        name: raw.name,
        version: raw.version,
        description: raw.description,
        package_type: raw.package_type,
        category: raw.category,
        author: raw.author,
        install_type: raw.install_type,
        install_dir: raw.install_dir,
        main_file_path: raw.main_file_path,
        root_hash: raw.hashes.root,
    }))
}

fn parse_json<T>(bytes: &[u8], label: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    if bytes.len() > PACKAGE_OUTPUT_LIMIT {
        bail!("{label} JSON output exceeded {PACKAGE_OUTPUT_LIMIT} bytes");
    }
    serde_json::from_slice(bytes).with_context(|| format!("failed to parse {label} JSON output"))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::{fs::Permissions, os::unix::fs::PermissionsExt as _, time::Instant};

    use anyhow::{Context as _, Result, bail};
    use serde_json::{Value, json};
    #[cfg(unix)]
    use tokio_util::sync::CancellationToken;

    use super::*;

    const ROOT_HASH: &str = "4b9e874dda8be655169fed00be09d3d1867760001ecf5a6799fa66a35b9e2a6b";
    const SHA256: &str = "bd403571c1daaf4fa1b7e475974a7d08ecfc00537eee1fb93870e2320996c3ba";

    fn replace_json_pointer(value: &mut Value, pointer: &str, replacement: Value) -> Result<()> {
        let target = value
            .pointer_mut(pointer)
            .with_context(|| format!("missing test JSON pointer `{pointer}`"))?;
        *target = replacement;
        Ok(())
    }

    fn catalog_value() -> Value {
        json!({
            "author": "",
            "category": "blockchain",
            "description": "Logos Execution Zone Indexer Module for Logos Core",
            "name": INDEXER_PACKAGE_NAME,
            "repositoryDisplayName": "Logos Official",
            "repositoryName": OFFICIAL_REPOSITORY_NAME,
            "repositoryUrl": OFFICIAL_REPOSITORY_URL,
            "type": INDEXER_PACKAGE_TYPE,
            "versions": [{
                "manifest": {
                    "hashes": { "root": ROOT_HASH },
                    "name": INDEXER_PACKAGE_NAME,
                    "type": INDEXER_PACKAGE_TYPE,
                    "version": "1.0.0"
                },
                "publisherRef": "lez_indexer_module-v1.0.0",
                "releasedAt": "2026-07-02T15:30:56Z",
                "rootHash": ROOT_HASH,
                "sha256": SHA256,
                "size": 42025161,
                "url": "https://github.com/logos-co/logos-modules-release/releases/download/lez_indexer_module-v1.0.0/lez_indexer_module-1.0.0.lgx"
            }]
        })
    }

    #[test]
    fn parses_only_official_indexer_catalog_identity() -> Result<()> {
        let package = parse_catalog(&serde_json::to_vec(&catalog_value())?)?;
        if package.name != INDEXER_PACKAGE_NAME
            || package.repository_name != OFFICIAL_REPOSITORY_NAME
            || package.versions.len() != 1
            || package
                .versions
                .first()
                .map(|release| release.root_hash.as_str())
                != Some(ROOT_HASH)
        {
            bail!("catalog report lost official package identity: {package:?}");
        }

        let mut wrong_repository = catalog_value();
        replace_json_pointer(
            &mut wrong_repository,
            "/repositoryUrl",
            json!("https://example.com/logos-repo.json"),
        )?;
        let error = parse_catalog(&serde_json::to_vec(&wrong_repository)?).err();
        if error.is_none_or(|error| !error.to_string().contains("non-official")) {
            bail!("non-official catalog entry was not rejected");
        }

        let mut wrong_hash = catalog_value();
        replace_json_pointer(
            &mut wrong_hash,
            "/versions/0/manifest/hashes/root",
            json!(SHA256),
        )?;
        let error = parse_catalog(&serde_json::to_vec(&wrong_hash)?).err();
        if error.is_none_or(|error| !error.to_string().contains("catalog identity")) {
            bail!("catalog release hash mismatch was not rejected");
        }
        Ok(())
    }

    #[test]
    fn installed_report_is_scoped_to_configured_modules_directory() -> Result<()> {
        let modules_dir = Path::new("/opt/logos-node/modules");
        let installed = json!([{
            "author": "",
            "category": "blockchain",
            "description": "Indexer",
            "hashes": { "root": ROOT_HASH },
            "installDir": "/opt/logos-node/modules/lez_indexer_module",
            "installType": "user",
            "mainFilePath": "/opt/logos-node/modules/lez_indexer_module/lez_indexer_module_plugin.so",
            "name": INDEXER_PACKAGE_NAME,
            "type": INDEXER_PACKAGE_TYPE,
            "version": "1.0.0"
        }]);
        let report = parse_installed(&serde_json::to_vec(&installed)?, modules_dir)?
            .context("expected installed report")?;
        if report.version != "1.0.0" || report.root_hash != ROOT_HASH {
            bail!("installed package identity was not preserved: {report:?}");
        }
        if parse_installed(b"No installed modules found\n", modules_dir)?.is_some() {
            bail!("empty lgpm result was treated as installed");
        }

        let mut outside = installed;
        replace_json_pointer(
            &mut outside,
            "/0/installDir",
            json!("/tmp/lez_indexer_module"),
        )?;
        replace_json_pointer(
            &mut outside,
            "/0/mainFilePath",
            json!("/tmp/lez_indexer_module/plugin.so"),
        )?;
        let error = parse_installed(&serde_json::to_vec(&outside)?, modules_dir).err();
        if error.is_none_or(|error| !error.to_string().contains("outside configured")) {
            bail!("out-of-scope package installation was not rejected");
        }
        Ok(())
    }

    #[test]
    fn installed_artifact_requires_existing_regular_main_file() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let modules_dir = directory.path().join("modules");
        let install_dir = modules_dir.join(INDEXER_PACKAGE_NAME);
        let main_file_path = install_dir.join("lez_indexer_module_plugin.so");
        fs::create_dir_all(&install_dir)?;
        let report = LocalNodeInstalledPackageReport {
            name: INDEXER_PACKAGE_NAME.to_owned(),
            version: "1.0.0".to_owned(),
            description: "Indexer".to_owned(),
            package_type: INDEXER_PACKAGE_TYPE.to_owned(),
            category: "blockchain".to_owned(),
            author: String::new(),
            install_type: "user".to_owned(),
            install_dir: install_dir.display().to_string(),
            main_file_path: main_file_path.display().to_string(),
            root_hash: ROOT_HASH.to_owned(),
        };

        let error = validate_installed_artifact(&report, &modules_dir).err();
        if error.is_none_or(|error| !error.to_string().contains("main file")) {
            bail!("missing installed main file was not rejected");
        }
        fs::write(&main_file_path, b"module")?;
        validate_installed_artifact(&report, &modules_dir)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn installed_catalog_ignores_stale_lgpm_artifact() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let modules_dir = directory.path().join("modules");
        let install_dir = modules_dir.join(INDEXER_PACKAGE_NAME);
        let main_file_path = install_dir.join("lez_indexer_module_plugin.so");
        fs::create_dir_all(&install_dir)?;
        let installed_json = serde_json::to_string(&json!([{
            "author": "",
            "category": "blockchain",
            "description": "Indexer",
            "hashes": { "root": ROOT_HASH },
            "installDir": install_dir,
            "installType": "user",
            "mainFilePath": main_file_path,
            "name": INDEXER_PACKAGE_NAME,
            "type": INDEXER_PACKAGE_TYPE,
            "version": "1.0.0"
        }]))?;
        let lgpm = directory.path().join("lgpm");
        write_executable(
            &lgpm,
            &format!("#!/bin/sh\nprintf '%s\\n' '{installed_json}'\n"),
        )?;
        let toolchain = PackageToolchain {
            lgpd: None,
            lgpm: Some(lgpm),
        };

        anyhow::ensure!(query_installed(&toolchain, &modules_dir)?.is_none());
        fs::write(&main_file_path, b"module")?;
        anyhow::ensure!(query_installed(&toolchain, &modules_dir)?.is_some());
        Ok(())
    }

    #[test]
    fn package_commands_pin_official_repository_release_and_directories() -> Result<()> {
        let toolchain = PackageToolchain {
            lgpd: Some(PathBuf::from("/usr/bin/lgpd")),
            lgpm: Some(PathBuf::from("/usr/bin/lgpm")),
        };
        let package = parse_catalog(&serde_json::to_vec(&catalog_value())?)?;
        let release = package.versions.first().context("missing release")?;

        assert_command(
            toolchain.info_command()?,
            "/usr/bin/lgpd",
            &["info", INDEXER_PACKAGE_NAME, "--json"],
        )?;
        assert_command(
            toolchain.download_command(release, Path::new("/tmp/packages"))?,
            "/usr/bin/lgpd",
            &[
                "--version",
                "1.0.0",
                "--root-hash",
                ROOT_HASH,
                "--output",
                "/tmp/packages",
                "download",
                INDEXER_PACKAGE_NAME,
            ],
        )?;
        assert_command(
            toolchain.install_command(
                Path::new("/tmp/packages/lez_indexer_module-1.0.0.lgx"),
                Path::new("/opt/logos-node/modules"),
            )?,
            "/usr/bin/lgpm",
            &[
                "--modules-dir",
                "/opt/logos-node/modules",
                "install",
                "--file",
                "/tmp/packages/lez_indexer_module-1.0.0.lgx",
            ],
        )?;
        Ok(())
    }

    #[test]
    fn package_inputs_reject_relative_paths_and_option_like_versions() -> Result<()> {
        let relative = resolve_modules_dir(Some("modules")).err();
        if relative.is_none_or(|error| !error.to_string().contains("absolute path")) {
            bail!("relative modules directory was not rejected");
        }
        let mut release = parse_catalog(&serde_json::to_vec(&catalog_value())?)?
            .versions
            .into_iter()
            .next()
            .context("missing release")?;
        release.version = "--repo".to_owned();
        let error = validate_release(&release).err();
        if error.is_none_or(|error| !error.to_string().contains("unsupported characters")) {
            bail!("option-like package version was not rejected");
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn controlled_download_and_install_return_verified_typed_reports() -> Result<()> {
        let root = tempfile::tempdir()?;
        let output_dir = root.path().join("downloads");
        let modules_dir = root.path().join("modules");
        fs::create_dir_all(&output_dir)?;
        fs::create_dir_all(&modules_dir)?;
        let lgpd = root.path().join("lgpd");
        let lgpm = root.path().join("lgpm");
        write_executable(
            &lgpd,
            r#"#!/bin/sh
output=""
version=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) shift; output="$1" ;;
        --version) shift; version="$1" ;;
    esac
    shift
done
printf 'abc' > "$output/lez_indexer_module-$version.lgx"
"#,
        )?;
        let installed_json = serde_json::to_string(&json!([{
            "author": "",
            "category": "blockchain",
            "description": "Indexer",
            "hashes": { "root": ROOT_HASH },
            "installDir": modules_dir.join(INDEXER_PACKAGE_NAME),
            "installType": "user",
            "mainFilePath": modules_dir
                .join(INDEXER_PACKAGE_NAME)
                .join("lez_indexer_module_plugin.so"),
            "name": INDEXER_PACKAGE_NAME,
            "type": INDEXER_PACKAGE_TYPE,
            "version": "1.0.0"
        }]))?;
        write_executable(
            &lgpm,
            &format!(
                "#!/bin/sh\ncase \" $* \" in\n  *\" list \"*) printf '%s\\n' '{installed_json}' ;;\n  *\" install \"*) mkdir -p '{install_dir}'; printf '%s' 'module' > '{main_file_path}'; printf '%s\\n' 'Installed' ;;\n  *) exit 2 ;;\nesac\n",
                install_dir = modules_dir.join(INDEXER_PACKAGE_NAME).display(),
                main_file_path = modules_dir
                    .join(INDEXER_PACKAGE_NAME)
                    .join("lez_indexer_module_plugin.so")
                    .display(),
            ),
        )?;
        let toolchain = PackageToolchain {
            lgpd: Some(lgpd),
            lgpm: Some(lgpm),
        };
        let mut release = parse_catalog(&serde_json::to_vec(&catalog_value())?)?
            .versions
            .into_iter()
            .next()
            .context("missing release")?;
        release.size = 3;
        let control = CommandControl::new(
            CancellationToken::new(),
            Instant::now() + Duration::from_secs(5),
        )
        .with_isolated_test_budget();

        let downloaded = download_official_indexer_module_with(
            &toolchain,
            &release,
            &output_dir,
            control.clone(),
        )?;
        if downloaded.file_path != output_dir.join(package_filename("1.0.0"))
            || downloaded.root_hash != ROOT_HASH
        {
            bail!("download report lost verified identity: {downloaded:?}");
        }
        let installed =
            install_official_indexer_module_with(&toolchain, &downloaded, &modules_dir, control)?;
        if installed.version != downloaded.version
            || installed.root_hash != downloaded.root_hash
            || Path::new(&installed.install_dir) != modules_dir.join(INDEXER_PACKAGE_NAME)
        {
            bail!("install report did not match download: {installed:?}");
        }
        Ok(())
    }

    #[test]
    fn catalog_report_serializes_explicit_not_installed_state() -> Result<()> {
        let report = LocalNodePackageCatalogReport {
            modules_dir: DEFAULT_MODULES_DIR.to_owned(),
            package: parse_catalog(&serde_json::to_vec(&catalog_value())?)?,
            installed: None,
        };
        let value = serde_json::to_value(report)?;
        if value.get("installed") != Some(&Value::Null)
            || value.pointer("/package/versions/0/version") != Some(&json!("1.0.0"))
            || value.get("modules_dir") != Some(&json!(DEFAULT_MODULES_DIR))
        {
            bail!("package catalog wire contract drifted: {value}");
        }
        Ok(())
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, contents: &str) -> Result<()> {
        fs::write(path, contents)?;
        fs::set_permissions(path, Permissions::from_mode(0o700))?;
        Ok(())
    }

    fn assert_command(command: Command, program: &str, args: &[&str]) -> Result<()> {
        if command.get_program() != program {
            bail!("unexpected command program: {:?}", command.get_program());
        }
        let actual = command.get_args().map(OsString::from).collect::<Vec<_>>();
        let expected = args.iter().map(OsString::from).collect::<Vec<_>>();
        if actual != expected {
            bail!("unexpected command arguments: {actual:?}");
        }
        Ok(())
    }
}
