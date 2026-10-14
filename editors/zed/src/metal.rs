use std::fs;

use zed_extension_api::{
    self as zed, Architecture, DownloadedFileType, GithubReleaseOptions, LanguageServerId, Os, Result,
    settings::LspSettings,
};

const SERVER_NAME: &str = "metal-analyzer";
const GITHUB_REPO: &str = "computer-graphics-tools/metal-analyzer";

struct MetalExtension {
    cached_binary_path: Option<String>,
}

impl MetalExtension {
    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        let lsp_settings = LspSettings::for_worktree(SERVER_NAME, worktree).ok();

        if let Some(path) = lsp_settings.as_ref().and_then(|s| s.binary.as_ref()).and_then(|b| b.path.clone()) {
            return Ok(path);
        }

        if let Some(path) = &self.cached_binary_path
            && fs::metadata(path).is_ok_and(|stat| stat.is_file())
        {
            return Ok(path.clone());
        }

        let (os, arch) = zed::current_platform();
        let binary_name = server_binary_name(os);

        if let Some(path) = worktree.which(binary_name) {
            self.cached_binary_path = Some(path.clone());
            return Ok(path);
        }

        // Prefer a binary already downloaded by a previous run. Reusing it
        // avoids contacting the GitHub API on every launch, which otherwise
        // fails outright under API rate limiting (e.g. a shared egress IP) and
        // leaves the server uninstalled even though a working binary is present.
        // The release check below only runs on a true first install, when no
        // binary exists yet. To force an upgrade, delete the cached
        // `metal-analyzer-*` directory or set `lsp.metal-analyzer.binary.path`.
        if let Some(path) = find_cached_binary(binary_name) {
            self.cached_binary_path = Some(path.clone());
            return Ok(path);
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = zed::latest_github_release(
            GITHUB_REPO,
            GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )
        .inspect_err(|_| {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::None,
            );
        })?;

        let release_asset_name = match (os, arch) {
            (Os::Mac, Architecture::Aarch64) => "metal-analyzer-aarch64-apple-darwin.tar.gz",
            (Os::Mac, Architecture::X8664) => "metal-analyzer-x86_64-apple-darwin.tar.gz",
            (Os::Windows, Architecture::Aarch64) => "metal-analyzer-aarch64-pc-windows-msvc.zip",
            (Os::Windows, Architecture::X8664) => "metal-analyzer-x86_64-pc-windows-msvc.zip",
            _ => {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::None,
                );
                return Err(format!(
                    "{SERVER_NAME} auto-install is currently supported on macOS and Windows only.\n\
                     Install it manually with:\n\
                     \n\
                     cargo install metal-analyzer\n\
                     \n\
                     Or install from source:\n\
                     \n\
                     cargo install --git https://github.com/{GITHUB_REPO} --locked metal-analyzer\n"
                ));
            },
        };

        let version_dir = format!("{SERVER_NAME}-{}", release.version);
        let binary_path = format!("{version_dir}/{binary_name}");
        if fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            self.cached_binary_path = Some(binary_path.clone());
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::None,
            );
            return Ok(binary_path);
        }

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == release_asset_name)
            .ok_or_else(|| {
                format!(
                    "no release asset found matching {release_asset_name} in {}.\n\
                     Install {SERVER_NAME} manually (e.g. `cargo install metal-analyzer`) and add it to PATH,\n\
                     or set `lsp.{SERVER_NAME}.binary.path` in your Zed settings.\n",
                    release.version
                )
            })
            .inspect_err(|_| {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::None,
                );
            })?;

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::Downloading,
        );
        let file_type = match os {
            Os::Windows => DownloadedFileType::Zip,
            Os::Mac | Os::Linux => DownloadedFileType::GzipTar,
        };
        zed::download_file(&asset.download_url, &version_dir, file_type).inspect_err(|_| {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::None,
            );
        })?;
        // Windows has no executable permission bit; `.exe` files run as-is.
        if os != Os::Windows {
            zed::make_file_executable(&binary_path).inspect_err(|_| {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::None,
                );
            })?;
        }
        zed::set_language_server_installation_status(language_server_id, &zed::LanguageServerInstallationStatus::None);

        remove_outdated_versions(&version_dir)?;
        self.cached_binary_path = Some(binary_path.clone());

        Ok(binary_path)
    }

    fn language_server_arguments(
        &self,
        worktree: &zed::Worktree,
    ) -> Vec<String> {
        LspSettings::for_worktree(SERVER_NAME, worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.binary)
            .and_then(|binary| binary.arguments)
            .unwrap_or_default()
    }
}

fn server_binary_name(os: Os) -> &'static str {
    match os {
        Os::Windows => "metal-analyzer.exe",
        Os::Mac | Os::Linux => SERVER_NAME,
    }
}

// Scan the extension work directory for the newest already-downloaded
// `metal-analyzer-<version>/<binary_name>` binary, if any.
fn find_cached_binary(binary_name: &str) -> Option<String> {
    let prefix = format!("{SERVER_NAME}-");
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for entry in fs::read_dir(".").ok()?.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if !name.starts_with(&prefix) || !entry.path().is_dir() {
            continue;
        }
        let binary_path = format!("{name}/{binary_name}");
        if !fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if newest.as_ref().is_none_or(|(newest_time, _)| modified >= *newest_time) {
            newest = Some((modified, binary_path));
        }
    }
    newest.map(|(_, path)| path)
}

fn remove_outdated_versions(current_version_dir: &str) -> Result<()> {
    let entries = fs::read_dir(".").map_err(|error| format!("failed to list extension directory: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read extension entry: {error}"))?;
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if !file_name.starts_with(&format!("{SERVER_NAME}-")) || file_name == current_version_dir {
            continue;
        }

        if entry.path().is_dir() {
            fs::remove_dir_all(entry.path()).ok();
        }
    }

    Ok(())
}

impl zed::Extension for MetalExtension {
    fn new() -> Self {
        MetalExtension {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary_path = self.language_server_binary_path(language_server_id, worktree)?;
        let args = self.language_server_arguments(worktree);

        Ok(zed::Command {
            command: binary_path,
            args,
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(MetalExtension);
