use std::{
    collections::BTreeMap,
    fs,
    io::{BufReader, Read as _},
    path::{Component, Path, PathBuf},
    process::{Command, Output},
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tar::Archive;

use crate::support::command_runner::{
    CommandControl, CommandRunPolicy, DEFAULT_COMMAND_CAPTURE_LIMIT, run_command,
    run_command_controlled,
};

use super::{LocalNodePackageCommit, action_engine::LocalNodeActionEngine, process::find_command};

const INDEXER_PACKAGE_NAME: &str = "lez_indexer_module";
const INDEXER_PACKAGE_TYPE: &str = "core";
const OFFICIAL_REPOSITORY_NAME: &str = "logos-modules-official";
const OFFICIAL_REPOSITORY_URL: &str = "https://raw.githubusercontent.com/logos-co/logos-modules-release/refs/heads/main/logos-repo.json";
const OFFICIAL_DOWNLOAD_PATH_PREFIX: &str = "/logos-co/logos-modules-release/releases/download/";
const DEFAULT_MODULES_DIR: &str = "/opt/logos-node/modules";
const PACKAGE_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);
const PACKAGE_CATALOG_TIMEOUT: Duration = Duration::from_secs(30);
const PACKAGE_OUTPUT_LIMIT: usize = 1024 * 1024;
const MAX_PACKAGE_MANIFEST_SIZE: u64 = 256 * 1024;
const MAX_LGX_UNCOMPRESSED_INSPECTION_SIZE: u64 = 32 * 1024 * 1024;

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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct LocalModuleCatalogReport {
    pub modules_dir: String,
    pub repositories: Vec<LocalModuleRepositoryReport>,
    pub packages: Vec<LocalModulePackageCatalogEntry>,
    pub installed: Vec<LocalNodeInstalledPackageReport>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct LocalModuleRepositoryReport {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub url: String,
    pub enabled: bool,
    pub is_default: bool,
    pub resolve_error: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct LocalModulePackageCatalogEntry {
    pub name: String,
    pub description: String,
    pub package_type: String,
    pub category: String,
    pub repository_name: String,
    pub repository_display_name: String,
    pub repository_url: String,
    pub versions: Vec<LocalNodePackageRelease>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct LocalModuleInstallRequest {
    pub modules_dir: String,
    pub source: LocalModuleInstallSource,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum LocalModuleInstallSource {
    Repository {
        repository_name: String,
        repository_url: String,
        package_name: String,
        version: String,
        root_hash: String,
    },
    LocalFile {
        file_path: String,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct LocalModuleInstallReport {
    pub modules_dir: String,
    pub installed: Vec<LocalNodeInstalledPackageReport>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DownloadedLocalModulePackage {
    name: String,
    version: String,
    root_hash: String,
    file_path: PathBuf,
}

#[derive(Debug, Default)]
struct LocalModuleInstalledReport {
    installed: Vec<LocalNodeInstalledPackageReport>,
    warnings: Vec<String>,
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

    fn catalog_command(&self) -> Result<Command> {
        let mut command = Command::new(self.lgpd()?);
        command.arg("list").arg("--json");
        Ok(command)
    }

    fn repositories_command(&self) -> Result<Command> {
        let mut command = Command::new(self.lgpd()?);
        command.arg("repo").arg("list").arg("--json");
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

    fn module_download_command(
        &self,
        package_name: &str,
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
            .arg(package_name);
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

pub(crate) fn local_module_catalog(
    requested_modules_dir: Option<&str>,
) -> Result<LocalModuleCatalogReport> {
    let modules_dir = resolve_modules_dir(requested_modules_dir)?;
    let toolchain = PackageToolchain::system();
    let repositories = query_repositories(&toolchain)?;
    let packages = query_core_module_catalog(&toolchain)?;
    let installed = if toolchain.lgpm.is_some() {
        query_installed_modules(&toolchain, &modules_dir)?
    } else {
        LocalModuleInstalledReport::default()
    };
    Ok(LocalModuleCatalogReport {
        modules_dir: modules_dir.display().to_string(),
        repositories,
        packages,
        installed: installed.installed,
        warnings: installed.warnings,
    })
}

pub(crate) fn install_local_module(
    request: &LocalModuleInstallRequest,
    download_control: CommandControl,
    package_commit: &mut LocalNodePackageCommit,
) -> Result<LocalModuleInstallReport> {
    install_local_module_with(
        &PackageToolchain::system(),
        request,
        download_control,
        package_commit,
    )
}

fn install_local_module_with(
    toolchain: &PackageToolchain,
    request: &LocalModuleInstallRequest,
    download_control: CommandControl,
    package_commit: &mut LocalNodePackageCommit,
) -> Result<LocalModuleInstallReport> {
    let modules_dir = canonical_modules_dir(Path::new(request.modules_dir.trim()))?;
    let before = query_installed_modules(toolchain, &modules_dir)?;
    let mut warnings = before.warnings;
    let package_path = match &request.source {
        LocalModuleInstallSource::Repository {
            repository_name,
            repository_url,
            package_name,
            version,
            root_hash,
        } => {
            let packages = query_core_module_catalog(toolchain)?;
            let entry =
                find_catalog_package(&packages, repository_name, repository_url, package_name)?;
            let release = find_catalog_release(&entry, version, root_hash)?;
            let temporary = tempfile::Builder::new()
                .prefix("logos-inspector-module-package-")
                .tempdir()
                .context("failed to create module package download directory")?;
            let downloaded = download_module_package_with(
                toolchain,
                &entry,
                release,
                temporary.path(),
                download_control,
            )?;
            let path = downloaded.file_path.clone();
            let expected = ModulePackageIdentity {
                name: downloaded.name,
                version: downloaded.version,
                root_hash: downloaded.root_hash,
            };
            let output =
                install_module_file_with(toolchain, &path, &modules_dir, package_commit.begin()?)?;
            let after = query_installed_modules(toolchain, &modules_dir)?;
            warnings.extend(after.warnings);
            warnings.extend(package_manager_warnings(&output));
            normalize_package_warnings(&mut warnings);
            let installed = installed_identity(&after.installed, &expected)
                .context("lgpm completed but the selected module identity is not installed")?;
            return Ok(LocalModuleInstallReport {
                modules_dir: modules_dir.display().to_string(),
                installed: vec![installed.clone()],
                warnings,
            });
        }
        LocalModuleInstallSource::LocalFile { file_path } => {
            let path = validate_local_package_file(file_path)?;
            path
        }
    };
    let output = install_module_file_with(
        toolchain,
        &package_path,
        &modules_dir,
        package_commit.begin()?,
    )?;
    let after = query_installed_modules(toolchain, &modules_dir)?;
    warnings.extend(after.warnings);
    warnings.extend(package_manager_warnings(&output));
    normalize_package_warnings(&mut warnings);
    let installed = changed_installed_modules(&before.installed, &after.installed);
    Ok(LocalModuleInstallReport {
        modules_dir: modules_dir.display().to_string(),
        installed,
        warnings,
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

fn query_repositories(toolchain: &PackageToolchain) -> Result<Vec<LocalModuleRepositoryReport>> {
    let output = run_package_command(
        toolchain.repositories_command()?,
        "lgpd repo list",
        PACKAGE_CATALOG_TIMEOUT,
    )?;
    parse_repositories(&output.stdout)
}

fn query_core_module_catalog(
    toolchain: &PackageToolchain,
) -> Result<Vec<LocalModulePackageCatalogEntry>> {
    let output = run_package_command(
        toolchain.catalog_command()?,
        "lgpd list",
        PACKAGE_CATALOG_TIMEOUT,
    )?;
    parse_core_module_catalog(&output.stdout)
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

fn query_installed_modules(
    toolchain: &PackageToolchain,
    modules_dir: &Path,
) -> Result<LocalModuleInstalledReport> {
    let output = run_package_command(
        toolchain.installed_command(modules_dir)?,
        "lgpm list",
        PACKAGE_CATALOG_TIMEOUT,
    )?;
    parse_installed_modules(&output.stdout, modules_dir)
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

fn download_module_package_with(
    toolchain: &PackageToolchain,
    package: &LocalModulePackageCatalogEntry,
    release: &LocalNodePackageRelease,
    output_dir: &Path,
    control: CommandControl,
) -> Result<DownloadedLocalModulePackage> {
    validate_module_catalog_entry(package)?;
    validate_module_release(&package.name, release)?;
    validate_absolute_directory(output_dir, "package download directory", true)?;
    run_package_command_controlled(
        toolchain.module_download_command(&package.name, release, output_dir)?,
        "lgpd download module package",
        control,
    )?;

    let file_path = output_dir.join(module_package_filename(&package.name, &release.version));
    validate_downloaded_module_file(&file_path, release)?;
    Ok(DownloadedLocalModulePackage {
        name: package.name.clone(),
        version: release.version.clone(),
        root_hash: release.root_hash.clone(),
        file_path,
    })
}

fn install_module_file_with(
    toolchain: &PackageToolchain,
    package_path: &Path,
    modules_dir: &Path,
    control: CommandControl,
) -> Result<Output> {
    validate_absolute_directory(modules_dir, "Logos modules directory", false)?;
    validate_local_package_path(package_path)?;
    // Do not pass `--allow-unsigned`: package-manager performs its configured
    // verification policy and returns any unsigned-package warning to the caller.
    run_package_command_controlled(
        toolchain.install_command(package_path, modules_dir)?,
        "lgpm install module package",
        control,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModulePackageIdentity {
    name: String,
    version: String,
    root_hash: String,
}

fn find_catalog_package<'a>(
    packages: &'a [LocalModulePackageCatalogEntry],
    repository_name: &str,
    repository_url: &str,
    package_name: &str,
) -> Result<&'a LocalModulePackageCatalogEntry> {
    validate_repository_name(repository_name)?;
    validate_repository_url(repository_url)?;
    validate_package_name(package_name)?;
    let mut matches = packages.iter().filter(|package| {
        package.repository_name == repository_name
            && package.repository_url == repository_url
            && package.name == package_name
    });
    let package = matches
        .next()
        .context("selected module package is no longer available in the configured catalog")?;
    if matches.next().is_some() {
        bail!("configured catalog contains duplicate module package identities");
    }
    Ok(package)
}

fn find_catalog_release<'a>(
    package: &'a LocalModulePackageCatalogEntry,
    version: &str,
    root_hash: &str,
) -> Result<&'a LocalNodePackageRelease> {
    validate_version(version)?;
    validate_hash(root_hash, "package root hash")?;
    let mut matches = package
        .versions
        .iter()
        .filter(|release| release.version == version && release.root_hash == root_hash);
    let release = matches
        .next()
        .context("selected module release is no longer available in the configured catalog")?;
    if matches.next().is_some() {
        bail!("configured catalog contains duplicate module release identities");
    }
    Ok(release)
}

fn installed_identity<'a>(
    installed: &'a [LocalNodeInstalledPackageReport],
    identity: &ModulePackageIdentity,
) -> Option<&'a LocalNodeInstalledPackageReport> {
    installed.iter().find(|package| {
        package.name == identity.name
            && package.version == identity.version
            && package.root_hash == identity.root_hash
    })
}

fn changed_installed_modules(
    before: &[LocalNodeInstalledPackageReport],
    after: &[LocalNodeInstalledPackageReport],
) -> Vec<LocalNodeInstalledPackageReport> {
    let before_by_name = before
        .iter()
        .map(|package| {
            (
                package.name.as_str(),
                (package.version.as_str(), package.root_hash.as_str()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    after
        .iter()
        .filter(|package| {
            before_by_name
                .get(package.name.as_str())
                .is_none_or(|identity| {
                    identity.0 != package.version || identity.1 != package.root_hash
                })
        })
        .cloned()
        .collect()
}

fn package_manager_warnings(output: &Output) -> Vec<String> {
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("package is unsigned") {
        vec![
            "Package manager accepted an unsigned package under its current signature policy."
                .to_owned(),
        ]
    } else {
        Vec::new()
    }
}

fn normalize_package_warnings(warnings: &mut Vec<String>) {
    warnings.sort();
    warnings.dedup();
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
            capture_limit: DEFAULT_COMMAND_CAPTURE_LIMIT,
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
            capture_limit: DEFAULT_COMMAND_CAPTURE_LIMIT,
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

fn validate_module_catalog_entry(package: &LocalModulePackageCatalogEntry) -> Result<()> {
    validate_package_name(&package.name)?;
    if package.package_type != INDEXER_PACKAGE_TYPE {
        bail!("only core packages can be installed into the Logos modules directory");
    }
    validate_repository_name(&package.repository_name)?;
    validate_repository_url(&package.repository_url)?;
    Ok(())
}

fn validate_module_release(package_name: &str, release: &LocalNodePackageRelease) -> Result<()> {
    validate_package_name(package_name)?;
    validate_version(&release.version)?;
    validate_hash(&release.root_hash, "package root hash")?;
    validate_hash(&release.sha256, "package SHA-256")?;
    if release.size == 0 {
        bail!("package size must be positive");
    }
    let url = url::Url::parse(&release.url).context("package download URL is invalid")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().ends_with(&format!(
            "/{}",
            module_package_filename(package_name, &release.version)
        ))
    {
        bail!("package download URL is not a secure module release artifact");
    }
    Ok(())
}

fn validate_downloaded_module_file(
    file_path: &Path,
    release: &LocalNodePackageRelease,
) -> Result<()> {
    let metadata = fs::symlink_metadata(file_path).with_context(|| {
        format!(
            "lgpd did not create expected package `{}`",
            file_path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
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
    let checksum = sha256_file(file_path)?;
    if !checksum.eq_ignore_ascii_case(&release.sha256) {
        bail!("downloaded package checksum does not match the catalog");
    }
    Ok(())
}

fn validate_local_package_file(value: &str) -> Result<PathBuf> {
    let value = value.trim();
    if value.is_empty() {
        bail!("local module package file is required");
    }
    let path = Path::new(value);
    validate_local_package_path(path)?;
    let canonical = fs::canonicalize(path).with_context(|| {
        format!(
            "failed to resolve local module package `{}`",
            path.display()
        )
    })?;
    validate_local_package_path(&canonical)?;
    validate_local_core_package(&canonical)?;
    Ok(canonical)
}

fn validate_local_package_path(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("local module package file must be an absolute path");
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        bail!("local module package file must not contain relative path components");
    }
    if path.extension().and_then(|value| value.to_str()) != Some("lgx") {
        bail!("local module package file must use the .lgx extension");
    }
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "failed to inspect local module package `{}`",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("local module package file must be a regular file");
    }
    Ok(())
}

fn validate_local_core_package(path: &Path) -> Result<()> {
    let manifest = read_lgx_manifest(path)?;
    validate_package_name(&manifest.name)
        .context("local module package manifest name is invalid")?;
    validate_version(&manifest.version)
        .context("local module package manifest version is invalid")?;
    if manifest.package_type != INDEXER_PACKAGE_TYPE {
        bail!(
            "local package `{}` has type `{}`; only core packages can be installed into the Logos modules directory",
            manifest.name,
            manifest.package_type
        );
    }
    Ok(())
}

fn read_lgx_manifest(path: &Path) -> Result<RawLgxManifest> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open local module package `{}`", path.display()))?;
    let decoder = GzDecoder::new(BufReader::new(file));
    let mut archive = Archive::new(decoder.take(MAX_LGX_UNCOMPRESSED_INSPECTION_SIZE));
    let mut entries = archive
        .entries()
        .context("local module package is not a readable LGX archive")?;
    let Some(entry) = entries.next() else {
        bail!("local module package does not contain manifest.json");
    };
    let mut entry = entry.context("local module package contains an unreadable LGX entry")?;
    let entry_path = entry
        .path()
        .context("local module package contains an invalid LGX entry path")?;
    if entry_path.as_ref() != Path::new("manifest.json") {
        bail!("local module package manifest.json must be its first archive entry");
    }
    if !entry.header().entry_type().is_file() {
        bail!("local module package manifest.json is not a regular file");
    }
    if entry.size() > MAX_PACKAGE_MANIFEST_SIZE {
        bail!("local module package manifest.json exceeds the inspection size limit");
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(entry.size()).context("local module package manifest is too large")?,
    );
    entry
        .read_to_end(&mut bytes)
        .context("failed to read local module package manifest.json")?;
    // Multi-platform LGX files can contain large variant payloads after the
    // canonical manifest. `lgpm` validates the full archive before it writes.
    parse_json::<RawLgxManifest>(&bytes, "local module package manifest")
}

fn sha256_file(path: &Path) -> Result<String> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open package `{}` for checksum", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to read package `{}` for checksum", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
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

fn validate_package_name(value: &str) -> Result<()> {
    validate_identifier(value, "package name")
}

fn validate_repository_name(value: &str) -> Result<()> {
    validate_identifier(value, "package repository name")
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || value.starts_with('-')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        bail!("{label} contains unsupported characters");
    }
    Ok(())
}

fn validate_repository_url(value: &str) -> Result<()> {
    let url = url::Url::parse(value).context("package repository URL is invalid")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("package repository URL must be an HTTPS URL without credentials");
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

fn module_package_filename(package_name: &str, version: &str) -> String {
    format!("{package_name}-{version}.lgx")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRepository {
    name: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    description: String,
    url: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    is_default: bool,
    #[serde(default)]
    resolve_error: String,
}

const fn default_enabled() -> bool {
    true
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
struct RawLgxManifest {
    name: String,
    #[serde(rename = "type")]
    package_type: String,
    version: String,
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

fn parse_repositories(bytes: &[u8]) -> Result<Vec<LocalModuleRepositoryReport>> {
    let mut repositories = parse_json::<Vec<RawRepository>>(bytes, "lgpd repo list")?
        .into_iter()
        .map(|repository| LocalModuleRepositoryReport {
            name: repository.name,
            display_name: repository.display_name,
            description: repository.description,
            url: repository.url,
            enabled: repository.enabled,
            is_default: repository.is_default,
            resolve_error: repository.resolve_error,
        })
        .collect::<Vec<_>>();
    repositories.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.url.cmp(&right.url))
    });
    Ok(repositories)
}

fn parse_core_module_catalog(bytes: &[u8]) -> Result<Vec<LocalModulePackageCatalogEntry>> {
    let mut packages = parse_json::<Vec<RawCatalogPackage>>(bytes, "lgpd list")?
        .into_iter()
        .filter(|package| package.package_type == INDEXER_PACKAGE_TYPE)
        .map(parse_core_module_entry)
        .collect::<Result<Vec<_>>>()?;
    packages.sort_by(|left, right| {
        left.repository_display_name
            .cmp(&right.repository_display_name)
            .then_with(|| left.repository_name.cmp(&right.repository_name))
            .then_with(|| left.repository_url.cmp(&right.repository_url))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(packages)
}

fn parse_core_module_entry(raw: RawCatalogPackage) -> Result<LocalModulePackageCatalogEntry> {
    let RawCatalogPackage {
        name,
        description,
        package_type,
        category,
        repository_name,
        repository_display_name,
        repository_url,
        versions,
    } = raw;
    let mut entry = LocalModulePackageCatalogEntry {
        name,
        description,
        package_type,
        category,
        repository_name,
        repository_display_name,
        repository_url,
        versions: Vec::new(),
    };
    validate_module_catalog_entry(&entry)?;
    entry.versions = versions
        .into_iter()
        .map(|release| parse_module_release(&entry.name, &entry.package_type, release))
        .collect::<Result<Vec<_>>>()?;
    if entry.versions.is_empty() {
        bail!("module package has no releases");
    }
    entry.versions.sort_by(|left, right| {
        right
            .released_at
            .cmp(&left.released_at)
            .then_with(|| right.version.cmp(&left.version))
            .then_with(|| left.root_hash.cmp(&right.root_hash))
    });
    Ok(entry)
}

fn parse_module_release(
    package_name: &str,
    package_type: &str,
    raw_release: RawCatalogRelease,
) -> Result<LocalNodePackageRelease> {
    if raw_release.manifest.name != package_name
        || raw_release.manifest.package_type != package_type
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
    validate_module_release(package_name, &release)?;
    Ok(release)
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

fn parse_installed_modules(bytes: &[u8], modules_dir: &Path) -> Result<LocalModuleInstalledReport> {
    let text = std::str::from_utf8(bytes).context("lgpm list output is not UTF-8")?;
    if text.trim() == "No installed modules found" {
        return Ok(LocalModuleInstalledReport::default());
    }
    let mut installed = Vec::new();
    let mut warnings = Vec::new();
    for package in parse_json::<Vec<RawInstalledPackage>>(bytes, "lgpm list")?
        .into_iter()
        .filter(|package| package.package_type == INDEXER_PACKAGE_TYPE)
    {
        let name = package.name.clone();
        match parse_installed_module(package, modules_dir) {
            Ok(package) => installed.push(package),
            Err(error) => warnings.push(format!(
                "Ignored invalid installed core module `{name}`: {error:#}"
            )),
        }
    }
    installed.sort_by(|left, right| left.name.cmp(&right.name));
    let mut names = BTreeMap::new();
    for package in &installed {
        if names.insert(package.name.as_str(), ()).is_some() {
            bail!("lgpm returned duplicate core module installations");
        }
    }
    normalize_package_warnings(&mut warnings);
    Ok(LocalModuleInstalledReport {
        installed,
        warnings,
    })
}

fn parse_installed_module(
    raw: RawInstalledPackage,
    modules_dir: &Path,
) -> Result<LocalNodeInstalledPackageReport> {
    validate_package_name(&raw.name)?;
    validate_version(&raw.version)?;
    validate_hash(&raw.hashes.root, "installed package root hash")?;
    let expected_install_dir = modules_dir.join(&raw.name);
    let install_dir = Path::new(&raw.install_dir);
    let main_file_path = Path::new(&raw.main_file_path);
    if install_dir != expected_install_dir {
        bail!("installed core module is outside configured modules directory");
    }
    if !main_file_path.is_absolute()
        || main_file_path == install_dir
        || !main_file_path.starts_with(install_dir)
    {
        bail!("installed core module main file is outside its install directory");
    }
    let report = LocalNodeInstalledPackageReport {
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
    };
    validate_installed_module_artifact(&report, modules_dir)?;
    Ok(report)
}

fn validate_installed_module_artifact(
    installed: &LocalNodeInstalledPackageReport,
    modules_dir: &Path,
) -> Result<()> {
    let modules_dir = canonical_modules_dir(modules_dir)?;
    let expected_install_dir = modules_dir.join(&installed.name);
    let install_dir = fs::canonicalize(&installed.install_dir).with_context(|| {
        format!(
            "installed core module directory `{}` is unavailable",
            installed.install_dir
        )
    })?;
    if !install_dir.is_dir() || install_dir != expected_install_dir {
        bail!("installed core module directory does not match configured modules directory");
    }
    let main_file_path = fs::canonicalize(&installed.main_file_path).with_context(|| {
        format!(
            "installed core module main file `{}` is unavailable",
            installed.main_file_path
        )
    })?;
    if !main_file_path.is_file()
        || main_file_path == install_dir
        || !main_file_path.starts_with(&install_dir)
    {
        bail!("installed core module main file is not a regular file in its install directory");
    }
    Ok(())
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

    fn module_catalog_value(package_name: &str, sha256: &str, size: u64) -> Value {
        json!([{
            "author": "",
            "category": "metrics",
            "description": "OpenMetrics module",
            "name": package_name,
            "repositoryDisplayName": "Example Modules",
            "repositoryName": "example-modules",
            "repositoryUrl": "https://example.test/logos-repo.json",
            "type": INDEXER_PACKAGE_TYPE,
            "versions": [{
                "manifest": {
                    "hashes": { "root": ROOT_HASH },
                    "name": package_name,
                    "type": INDEXER_PACKAGE_TYPE,
                    "version": "1.0.0"
                },
                "publisherRef": "example-module-v1.0.0",
                "releasedAt": "2026-07-20T12:00:00Z",
                "rootHash": ROOT_HASH,
                "sha256": sha256,
                "size": size,
                "url": format!("https://example.test/releases/{package_name}-1.0.0.lgx")
            }]
        }])
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
    fn generic_catalog_preserves_configured_core_release_identity() -> Result<()> {
        let mut catalog = module_catalog_value("openmetrics", SHA256, 42);
        let ui_package = json!({
            "author": "",
            "category": "wallet",
            "description": "Wallet UI",
            "name": "wallet-ui",
            "repositoryDisplayName": "Example Modules",
            "repositoryName": "example-modules",
            "repositoryUrl": "https://example.test/logos-repo.json",
            "type": "ui_qml",
            "versions": [{
                "manifest": {
                    "hashes": { "root": ROOT_HASH },
                    "name": "wallet-ui",
                    "type": "ui_qml",
                    "version": "1.0.0"
                },
                "publisherRef": "wallet-ui-v1.0.0",
                "releasedAt": "2026-07-20T12:00:00Z",
                "rootHash": ROOT_HASH,
                "sha256": SHA256,
                "size": 42,
                "url": "https://example.test/releases/wallet-ui-1.0.0.lgx"
            }]
        });
        let rows = catalog
            .as_array_mut()
            .context("generic module catalog fixture should be an array")?;
        rows.push(ui_package);

        let packages = parse_core_module_catalog(&serde_json::to_vec(&catalog)?)?;
        if packages.len() != 1
            || packages[0].name != "openmetrics"
            || packages[0].repository_name != "example-modules"
            || packages[0].versions.len() != 1
            || packages[0].versions[0].root_hash != ROOT_HASH
        {
            bail!("generic core package catalog identity was not preserved: {packages:?}");
        }
        Ok(())
    }

    #[test]
    fn downloaded_generic_module_requires_matching_checksum() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let package_path = directory.path().join("openmetrics-1.0.0.lgx");
        fs::write(&package_path, b"wrong")?;
        let package = parse_core_module_catalog(&serde_json::to_vec(&module_catalog_value(
            "openmetrics",
            &hex::encode(Sha256::digest(b"module")),
            5,
        ))?)?
        .into_iter()
        .next()
        .context("missing generic module catalog entry")?;
        let release = package
            .versions
            .first()
            .context("missing generic release")?;

        let error = validate_downloaded_module_file(&package_path, release).err();
        if error.is_none_or(|error| !error.to_string().contains("checksum")) {
            bail!("generic module checksum mismatch was not rejected");
        }
        Ok(())
    }

    #[test]
    fn local_module_file_rejects_ui_package_before_installation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let package_path = directory.path().join("chat-ui-0.2.0.lgx");
        write_lgx_manifest(&package_path, "chat_ui", "ui_qml", "0.2.0")?;

        let error = validate_local_package_file(
            package_path
                .to_str()
                .context("temporary package path is not UTF-8")?,
        )
        .err();
        if error.is_none_or(|error| !error.to_string().contains("only core packages")) {
            bail!("local UI package was not rejected before package-manager installation");
        }
        Ok(())
    }

    #[test]
    fn local_module_file_accepts_core_package_manifest_for_package_manager_validation() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let package_path = directory.path().join("openmetrics-1.0.0.lgx");
        write_lgx_manifest(&package_path, "openmetrics", INDEXER_PACKAGE_TYPE, "1.0.0")?;

        let validated = validate_local_package_file(
            package_path
                .to_str()
                .context("temporary package path is not UTF-8")?,
        )?;
        if validated != fs::canonicalize(&package_path)? {
            bail!("local core package path was not canonicalized");
        }
        Ok(())
    }

    #[test]
    fn local_module_file_reads_manifest_before_large_variant_payload() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let package_path = directory.path().join("openmetrics-1.0.0.lgx");
        let file = fs::File::create(&package_path)?;
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);
        let manifest = serde_json::to_vec(&json!({
            "name": "openmetrics",
            "type": INDEXER_PACKAGE_TYPE,
            "version": "1.0.0"
        }))?;
        let mut manifest_header = tar::Header::new_gnu();
        manifest_header.set_size(manifest.len() as u64);
        manifest_header.set_mode(0o644);
        manifest_header.set_cksum();
        archive.append_data(&mut manifest_header, "manifest.json", manifest.as_slice())?;

        let large_payload = vec![0_u8; usize::try_from(MAX_LGX_UNCOMPRESSED_INSPECTION_SIZE)? + 1];
        let mut payload_header = tar::Header::new_gnu();
        payload_header.set_size(large_payload.len() as u64);
        payload_header.set_mode(0o644);
        payload_header.set_cksum();
        archive.append_data(
            &mut payload_header,
            "variants/linux-amd64/openmetrics_plugin.so",
            large_payload.as_slice(),
        )?;
        let encoder = archive.into_inner()?;
        encoder.finish()?;

        let validated = validate_local_package_file(
            package_path
                .to_str()
                .context("temporary package path is not UTF-8")?,
        )?;
        if validated != fs::canonicalize(&package_path)? {
            bail!("large local core package path was not canonicalized");
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

    #[test]
    fn installed_module_catalog_keeps_valid_entries_when_another_core_artifact_is_stale()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let modules_dir = directory.path().join("modules");
        let valid_dir = modules_dir.join("openmetrics");
        let valid_main = valid_dir.join("openmetrics_plugin.so");
        let stale_dir = modules_dir.join("stale_module");
        fs::create_dir_all(&valid_dir)?;
        fs::create_dir_all(&stale_dir)?;
        fs::write(&valid_main, b"module")?;
        let installed = json!([
            {
                "author": "",
                "category": "metrics",
                "description": "OpenMetrics",
                "hashes": { "root": ROOT_HASH },
                "installDir": valid_dir,
                "installType": "user",
                "mainFilePath": valid_main,
                "name": "openmetrics",
                "type": INDEXER_PACKAGE_TYPE,
                "version": "1.0.0"
            },
            {
                "author": "",
                "category": "legacy",
                "description": "Stale module",
                "hashes": { "root": ROOT_HASH },
                "installDir": stale_dir,
                "installType": "user",
                "mainFilePath": stale_dir.join("missing.so"),
                "name": "stale_module",
                "type": INDEXER_PACKAGE_TYPE,
                "version": "1.0.0"
            }
        ]);

        let report = parse_installed_modules(&serde_json::to_vec(&installed)?, &modules_dir)?;
        if report.installed.len() != 1
            || report.installed[0].name != "openmetrics"
            || report.warnings.len() != 1
            || !report.warnings[0].contains("stale_module")
        {
            bail!("stale core artifact prevented a valid catalog: {report:?}");
        }
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

    #[cfg(unix)]
    #[test]
    fn controlled_generic_module_install_revalidates_catalog_and_reports_package_policy_warning()
    -> Result<()> {
        let root = tempfile::tempdir()?;
        let modules_dir = root.path().join("modules");
        fs::create_dir_all(&modules_dir)?;
        let lgpd = root.path().join("lgpd");
        let lgpm = root.path().join("lgpm");
        let state_path = root.path().join("installed-state");
        let install_dir = modules_dir.join("openmetrics");
        let main_file_path = install_dir.join("openmetrics_plugin.so");
        let package_sha = hex::encode(Sha256::digest(b"module"));
        let catalog_json = serde_json::to_string(&module_catalog_value(
            "openmetrics",
            &package_sha,
            b"module".len() as u64,
        ))?;
        let installed_json = serde_json::to_string(&json!([{
            "author": "",
            "category": "metrics",
            "description": "OpenMetrics",
            "hashes": { "root": ROOT_HASH },
            "installDir": install_dir,
            "installType": "user",
            "mainFilePath": main_file_path,
            "name": "openmetrics",
            "type": INDEXER_PACKAGE_TYPE,
            "version": "1.0.0"
        }]))?;
        write_executable(
            &lgpd,
            &format!(
                "#!/bin/sh\nif [ \"$1\" = \"list\" ] && [ \"$2\" = \"--json\" ]; then\n  printf '%s\\n' '{catalog_json}'\n  exit 0\nfi\noutput=''\nversion=''\nroot_hash=''\npackage=''\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --output) shift; output=\"$1\" ;;\n    --version) shift; version=\"$1\" ;;\n    --root-hash) shift; root_hash=\"$1\" ;;\n    download) shift; package=\"$1\"; break ;;\n  esac\n  shift\ndone\n[ \"$package\" = \"openmetrics\" ] || exit 8\n[ \"$version\" = \"1.0.0\" ] || exit 9\n[ \"$root_hash\" = \"{ROOT_HASH}\" ] || exit 10\nprintf 'module' > \"$output/openmetrics-$version.lgx\"\n"
            ),
        )?;
        write_executable(
            &lgpm,
            &format!(
                "#!/bin/sh\nif [ \"$1\" = \"--modules-dir\" ]; then\n  shift\n  shift\nfi\ncase \"$1\" in\n  list)\n    if [ -f '{state_path}' ]; then\n      printf '%s\\n' '{installed_json}'\n    else\n      printf '%s\\n' 'No installed modules found'\n    fi\n    ;;\n  install)\n    case \" $* \" in\n      *\" --allow-unsigned \"*) exit 11 ;;\n    esac\n    mkdir -p '{install_dir}'\n    printf '%s' 'module' > '{main_file_path}'\n    touch '{state_path}'\n    printf '%s\\n' 'Warning: Package is unsigned' >&2\n    ;;\n  *) exit 12 ;;\nesac\n",
                state_path = state_path.display(),
                install_dir = install_dir.display(),
                main_file_path = main_file_path.display(),
            ),
        )?;
        let toolchain = PackageToolchain {
            lgpd: Some(lgpd),
            lgpm: Some(lgpm),
        };
        let request = LocalModuleInstallRequest {
            modules_dir: modules_dir.display().to_string(),
            source: LocalModuleInstallSource::Repository {
                repository_name: "example-modules".to_owned(),
                repository_url: "https://example.test/logos-repo.json".to_owned(),
                package_name: "openmetrics".to_owned(),
                version: "1.0.0".to_owned(),
                root_hash: ROOT_HASH.to_owned(),
            },
        };
        let control = CommandControl::new(
            CancellationToken::new(),
            Instant::now() + Duration::from_secs(5),
        )
        .with_isolated_test_budget();
        let mut package_commit = LocalNodePackageCommit::new(|| {
            Ok((
                CommandControl::new(
                    CancellationToken::new(),
                    Instant::now() + Duration::from_secs(5),
                )
                .with_isolated_test_budget(),
                (),
            ))
        });

        let report = install_local_module_with(&toolchain, &request, control, &mut package_commit)?;
        if report.installed.len() != 1
            || report.installed[0].name != "openmetrics"
            || report.installed[0].version != "1.0.0"
            || report.installed[0].root_hash != ROOT_HASH
            || report.warnings
                != [
                    "Package manager accepted an unsigned package under its current signature policy.",
                ]
        {
            bail!("generic module installation report lost verified identity: {report:?}");
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

    fn write_lgx_manifest(
        path: &Path,
        name: &str,
        package_type: &str,
        version: &str,
    ) -> Result<()> {
        let file = fs::File::create(path)?;
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);
        let manifest = serde_json::to_vec(&json!({
            "name": name,
            "type": package_type,
            "version": version
        }))?;
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, "manifest.json", manifest.as_slice())?;
        let encoder = archive.into_inner()?;
        encoder.finish()?;
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
