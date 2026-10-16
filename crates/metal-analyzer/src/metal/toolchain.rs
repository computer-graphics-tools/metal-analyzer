use std::{ffi::OsStr, path::PathBuf};

pub fn background_command(program: impl AsRef<OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

pub fn metal_command() -> std::process::Command {
    #[cfg(windows)]
    {
        let mut command = background_command(windows::find_metal().unwrap_or_else(|| PathBuf::from("metal")));
        command.args(["-x", "metal", "-std=metal4.0"]);
        command
    }
    #[cfg(not(windows))]
    {
        let mut command = background_command("xcrun");
        command.arg("metal");
        command
    }
}

pub fn async_metal_command() -> tokio::process::Command {
    let mut command = tokio::process::Command::from(metal_command());
    command.kill_on_drop(true);
    command
}

pub async fn find_metal_compiler() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        windows::find_metal()
    }
    #[cfg(not(windows))]
    {
        let output = xcrun_output(&["--find", "metal"]).await?;
        let path = PathBuf::from(output);
        Some(dunce::canonicalize(&path).unwrap_or(path))
    }
}

pub async fn has_platform_sdk() -> bool {
    #[cfg(windows)]
    {
        true
    }
    #[cfg(not(windows))]
    {
        xcrun_output(&["--sdk", "macosx", "--show-sdk-path"]).await.is_some()
    }
}

pub fn clang_format_fallback() -> Option<(String, Vec<String>)> {
    #[cfg(windows)]
    {
        windows::find_clang_format().map(|path| (path.display().to_string(), Vec::new()))
    }
    #[cfg(not(windows))]
    {
        Some(("xcrun".to_string(), vec!["clang-format".to_string()]))
    }
}

pub fn install_hint() -> &'static str {
    if cfg!(windows) {
        "Install Metal Developer Tools for Windows: https://developer.apple.com/metal/tools/"
    } else {
        "Try: xcode-select --install; sudo xcode-select -s /Applications/Xcode.app/Contents/Developer; \
         xcodebuild -downloadComponent MetalToolchain"
    }
}

#[cfg(not(windows))]
async fn xcrun_output(args: &[&str]) -> Option<String> {
    let output = tokio::process::Command::from(background_command("xcrun")).args(args).output().await.ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

#[cfg(windows)]
mod windows {
    use std::path::{Path, PathBuf};

    pub(super) fn find_metal() -> Option<PathBuf> {
        let launcher = find_on_path("metal.exe")
            .or_else(|| find_in_program_files(Path::new(r"Metal Developer Tools\bin\metal.exe")))?;
        let launcher = dunce::canonicalize(&launcher).unwrap_or(launcher);
        Some(versioned_compiler(&launcher).unwrap_or(launcher))
    }

    pub(super) fn versioned_compiler(launcher: &Path) -> Option<PathBuf> {
        let versions_dir = launcher.parent()?.parent()?.join("metal");
        std::fs::read_dir(versions_dir)
            .ok()?
            .flatten()
            .filter_map(|entry| {
                let version = entry
                    .file_name()
                    .to_str()?
                    .split('.')
                    .map(|part| part.parse::<u64>().ok())
                    .collect::<Option<Vec<_>>>()?;
                let compiler = entry.path().join("bin").join("metal.exe");
                compiler.is_file().then_some((version, compiler))
            })
            .max_by(|(a, _), (b, _)| a.cmp(b))
            .map(|(_, compiler)| compiler)
    }

    pub(super) fn find_clang_format() -> Option<PathBuf> {
        if let Some(path) = find_in_program_files(Path::new(r"LLVM\bin\clang-format.exe")) {
            return Some(path);
        }

        let arch_dirs: &[&str] = if cfg!(target_arch = "aarch64") {
            &["ARM64", "x64"]
        } else {
            &["x64"]
        };
        let mut candidates = Vec::new();
        for root in program_files_roots() {
            let Ok(versions) = std::fs::read_dir(root.join("Microsoft Visual Studio")) else {
                continue;
            };
            for version in versions.flatten() {
                let Ok(editions) = std::fs::read_dir(version.path()) else {
                    continue;
                };
                for edition in editions.flatten() {
                    let llvm = edition.path().join(r"VC\Tools\Llvm");
                    for arch_dir in arch_dirs {
                        candidates.push(llvm.join(arch_dir).join(r"bin\clang-format.exe"));
                    }
                    candidates.push(llvm.join(r"bin\clang-format.exe"));
                }
            }
        }
        candidates.into_iter().find(|candidate| candidate.is_file())
    }

    fn find_on_path(file_name: &str) -> Option<PathBuf> {
        std::env::split_paths(&std::env::var_os("PATH")?)
            .map(|dir| dir.join(file_name))
            .find(|candidate| candidate.is_file())
    }

    fn find_in_program_files(relative: &Path) -> Option<PathBuf> {
        program_files_roots().map(|root| root.join(relative)).find(|candidate| candidate.is_file())
    }

    fn program_files_roots() -> impl Iterator<Item = PathBuf> {
        ["ProgramFiles", "ProgramW6432"].into_iter().filter_map(std::env::var_os).map(PathBuf::from)
    }
}

#[cfg(test)]
#[path = "../../tests/src/metal/toolchain_tests.rs"]
mod tests;
