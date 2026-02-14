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

        if let Some(path) = lsp_settings
            .as_ref()
            .and_then(|s| s.initialization_options.as_ref())
            .and_then(|v| v.get("metal-analyzer")?.get("serverPath")?.as_str().map(String::from))
        {
            return Ok(path);
        }

        if let Some(path) = lsp_settings
            .as_ref()
            .and_then(|s| s.initialization_options.as_ref())
            .and_then(|v| v.get("metal-analyzer")?.get("binary")?.get("path")?.as_str().map(String::from))
        {
            return Ok(path);
        }

        if let Some(path) = &self.cached_binary_path
            && fs::metadata(path).is_ok_and(|stat| stat.is_file())
        {
            return Ok(path.clone());
        }

        if let Some(path) = worktree.which(SERVER_NAME) {
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
        .map_err(|error| {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::None,
            );
            error
        })?;

        let (os, arch) = zed::current_platform();
        let release_asset_name = match (os, arch) {
            (Os::Mac, Architecture::Aarch64) => "metal-analyzer-aarch64-apple-darwin.tar.gz",
            (Os::Mac, Architecture::X8664) => "metal-analyzer-x86_64-apple-darwin.tar.gz",
            _ => {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::None,
                );
                return Err(format!(
                    "{SERVER_NAME} auto-install is currently supported on macOS only.\n\
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
        let binary_path = format!("{version_dir}/{SERVER_NAME}");
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
            .ok_or_else(|| format!("no release asset found matching {release_asset_name}"))
            .map_err(|error| {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::None,
                );
                error
            })?;

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::Downloading,
        );
        zed::download_file(&asset.download_url, &version_dir, DownloadedFileType::GzipTar).map_err(|error| {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::None,
            );
            error
        })?;
        zed::make_file_executable(&binary_path).map_err(|error| {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::None,
            );
            error
        })?;
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
