//! Locating and launching the Metal toolchain.
//!
//! On macOS every tool is reached through `xcrun`. On Windows, Apple's Metal
//! Developer Tools (<https://developer.apple.com/metal/tools/>) install a
//! standalone `metal.exe` that the installer does not add to `PATH`, and no
//! SDK is required because the Metal headers ship with the compiler.

use std::{ffi::OsStr, path::PathBuf};

/// Create a [`std::process::Command`] for a non-interactive helper tool.
///
/// On Windows this suppresses the console window that would otherwise pop up
/// for every compiler invocation when the server runs without a console.
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

/// Create a command that runs the Metal compiler (`xcrun metal` on macOS,
/// `metal.exe` on Windows). Arguments added by the caller go to the compiler.
pub fn metal_command() -> std::process::Command {
    #[cfg(windows)]
    {
        background_command(windows::find_metal().unwrap_or_else(|| PathBuf::from("metal")))
    }
    #[cfg(not(windows))]
    {
        let mut command = background_command("xcrun");
        command.arg("metal");
        command
    }
}

/// Async variant of [`metal_command`]. The child is killed if dropped.
pub fn async_metal_command() -> tokio::process::Command {
    let mut command = tokio::process::Command::from(metal_command());
    command.kill_on_drop(true);
    command
}

/// Resolve the canonical path of the active Metal compiler binary.
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

/// Returns `true` if the platform SDK needed by the compiler is installed.
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

/// Formatter to try when the default `clang-format` is not on `PATH`, as a
/// program plus leading arguments: `xcrun clang-format` on macOS, or an LLVM
/// or Visual Studio install on Windows.
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

/// Hint shown to users when the Metal toolchain cannot be found.
pub fn install_hint() -> &'static str {
    if cfg!(windows) {
        "Install the Metal Developer Tools for Windows from https://developer.apple.com/metal/tools/ \
         (or add the folder containing metal.exe to PATH)"
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

    /// Locate the Metal compiler: `metal.exe` on `PATH`, then the default
    /// `%ProgramFiles%\Metal Developer Tools` install location.
    pub(super) fn find_metal() -> Option<PathBuf> {
        let launcher = find_on_path("metal.exe")
            .or_else(|| find_in_program_files(Path::new(r"Metal Developer Tools\bin\metal.exe")))?;
        let launcher = dunce::canonicalize(&launcher).unwrap_or(launcher);
        Some(versioned_compiler(&launcher).unwrap_or(launcher))
    }

    /// `<install>\bin\metal.exe` is a launcher that forwards to
    /// `<install>\metal\<version>\bin\metal.exe`, but it drops argument
    /// quoting, so every path containing a space is split in two. Invoke the
    /// newest versioned compiler directly instead.
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

    /// Locate `clang-format.exe` from an LLVM install or Visual Studio's
    /// "C++ Clang tools" component.
    pub(super) fn find_clang_format() -> Option<PathBuf> {
        if let Some(path) = find_in_program_files(Path::new(r"LLVM\bin\clang-format.exe")) {
            return Some(path);
        }

        // <ProgramFiles>\Microsoft Visual Studio\<version>\<edition>\VC\Tools\Llvm\<arch>\bin
        // On ARM64, x64 builds also work through Windows' x64 emulation.
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
