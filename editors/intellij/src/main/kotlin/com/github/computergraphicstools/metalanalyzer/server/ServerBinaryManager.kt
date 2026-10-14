package com.github.computergraphicstools.metalanalyzer.server

import com.github.computergraphicstools.metalanalyzer.settings.MetalAnalyzerSettings
import com.intellij.openapi.application.PathManager
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.util.SystemInfo
import java.io.File
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption
import java.nio.file.attribute.PosixFilePermissions
import java.util.concurrent.TimeUnit
import java.util.zip.ZipInputStream

object ServerBinaryManager {
    private val LOG = Logger.getInstance(ServerBinaryManager::class.java)
    private const val SERVER_NAME = "metal-analyzer"
    private const val GITHUB_REPO = "computer-graphics-tools/metal-analyzer"
    private const val GITHUB_LATEST_RELEASE_API = "https://api.github.com/repos/$GITHUB_REPO/releases/latest"
    private const val TOOLCHAIN_CHECK_TIMEOUT_SECONDS = 8L
    private const val WINDOWS_METAL_COMPILER = "metal.exe"

    private val installRoot: Path
        get() = Path.of(PathManager.getSystemPath(), SERVER_NAME)

    private val serverBinaryName: String
        get() = if (SystemInfo.isWindows) "$SERVER_NAME.exe" else SERVER_NAME

    private val windowsDefaultMetalCompiler: Path
        get() = Path.of(
            System.getenv("ProgramFiles") ?: "C:\\Program Files",
            "Metal Developer Tools",
            "bin",
            WINDOWS_METAL_COMPILER,
        )

    data class ToolchainStatus(
        val available: Boolean,
        val reason: String? = null,
    )

    private data class CommandResult(
        val exitCode: Int,
        val output: String,
    )

    fun resolveServerPath(): String {
        val settings = MetalAnalyzerSettings.getInstance()
        val configured = settings.serverPath

        if (configured != SERVER_NAME) {
            return configured
        }

        findOnPath(serverBinaryName)?.let { return it }

        findCachedBinary()?.let { cached ->
            tryDownloadLatest()?.let { return it }
            return cached
        }

        tryDownloadLatest()?.let { return it }

        throw IllegalStateException(
            "Could not locate $SERVER_NAME, and no prebuilt release binary could be downloaded for this platform. " +
                "Install it manually (e.g. `cargo install metal-analyzer`), then add it to PATH or set the server path in Settings > Languages & Frameworks > Metal Analyzer."
        )
    }

    fun checkToolchainAvailability(): ToolchainStatus {
        if (SystemInfo.isWindows) {
            return checkWindowsToolchainAvailability()
        }

        if (!SystemInfo.isMac) {
            return ToolchainStatus(available = true)
        }

        val metalBinary = runCommand("xcrun", "--find", "metal")
        if (metalBinary.exitCode != 0 || metalBinary.output.trim().isEmpty()) {
            return ToolchainStatus(
                available = false,
                reason = firstNonBlankLine(metalBinary.output) ?: "`xcrun --find metal` failed",
            )
        }

        val sdkPath = runCommand("xcrun", "--sdk", "macosx", "--show-sdk-path")
        if (sdkPath.exitCode != 0 || sdkPath.output.trim().isEmpty()) {
            return ToolchainStatus(
                available = false,
                reason = firstNonBlankLine(sdkPath.output) ?: "`xcrun --sdk macosx --show-sdk-path` failed",
            )
        }

        val probe = runCommand("xcrun", "metal", "-v", "-E", "-", stdinText = "")
        if (probe.exitCode != 0) {
            return ToolchainStatus(
                available = false,
                reason = firstNonBlankLine(probe.output) ?: "`xcrun metal -v -E -` failed",
            )
        }

        return ToolchainStatus(available = true)
    }

    // The Metal Developer Tools for Windows installer does not add itself to PATH,
    // so also look in the default install location.
    private fun checkWindowsToolchainAvailability(): ToolchainStatus {
        if (findOnPath(WINDOWS_METAL_COMPILER) != null || Files.isRegularFile(windowsDefaultMetalCompiler)) {
            return ToolchainStatus(available = true)
        }

        return ToolchainStatus(
            available = false,
            reason = "`$WINDOWS_METAL_COMPILER` not found in PATH or at $windowsDefaultMetalCompiler",
        )
    }

    private fun findOnPath(executableName: String): String? {
        return try {
            val pathEntries = System.getenv("PATH")?.split(File.pathSeparator) ?: return null
            pathEntries
                .map { it.trim().removeSurrounding("\"") }
                .filter { it.isNotEmpty() }
                .map { File(it, executableName) }
                .firstOrNull { it.isFile && it.canExecute() }
                ?.absolutePath
        } catch (e: Exception) {
            LOG.debug("PATH lookup failed", e)
            null
        }
    }

    private fun findCachedBinary(): String? {
        val root = installRoot.toFile()
        if (!root.isDirectory) return null

        return root.listFiles()
            ?.filter { it.isDirectory && it.name.startsWith("$SERVER_NAME-") }
            ?.sortedByDescending { it.lastModified() }
            ?.firstNotNullOfOrNull { dir ->
                val binary = File(dir, serverBinaryName)
                if (binary.canExecute()) binary.absolutePath else null
            }
    }

    private fun tryDownloadLatest(): String? {
        if (!SystemInfo.isMac && !SystemInfo.isWindows) {
            LOG.info("Auto-download is only supported on macOS and Windows")
            return null
        }

        return try {
            downloadLatestBinary()
        } catch (e: Exception) {
            LOG.warn("Failed to download $SERVER_NAME", e)
            null
        }
    }

    private fun downloadLatestBinary(): String? {
        val assetName = releaseAssetName() ?: return null

        val releaseJson = curlGet(GITHUB_LATEST_RELEASE_API)
            ?: throw IllegalStateException("Failed to fetch release metadata from GitHub")
        val tagName = extractJsonString(releaseJson, "tag_name") ?: return null
        val downloadUrl = extractAssetDownloadUrl(releaseJson, assetName) ?: run {
            LOG.warn("Release $tagName has no $assetName asset; install $SERVER_NAME manually and add it to PATH or set the server path")
            return null
        }

        val versionDir = installRoot.resolve("$SERVER_NAME-$tagName")
        val binaryPath = versionDir.resolve(serverBinaryName)

        if (Files.isExecutable(binaryPath)) {
            return binaryPath.toString()
        }

        Files.createDirectories(versionDir)
        val archivePath = versionDir.resolve(assetName)

        curlDownload(downloadUrl, archivePath.toFile())

        if (assetName.endsWith(".zip")) {
            extractZip(archivePath, versionDir)
        } else {
            ProcessBuilder("tar", "-xzf", archivePath.toString(), "-C", versionDir.toString())
                .redirectErrorStream(true)
                .start()
                .waitFor()
        }

        if (!SystemInfo.isWindows) {
            Files.setPosixFilePermissions(binaryPath, PosixFilePermissions.fromString("rwxr-xr-x"))
        }
        Files.deleteIfExists(archivePath)
        removeOutdatedVersions(tagName)

        return if (Files.isExecutable(binaryPath)) binaryPath.toString() else null
    }

    private fun curlGet(url: String): String? {
        val process = ProcessBuilder(
            "curl", "-sSL",
            "-H", "Accept: application/vnd.github+json",
            "-H", "User-Agent: metal-analyzer-intellij-plugin",
            url
        ).redirectErrorStream(false).start()
        val output = process.inputStream.bufferedReader().readText()
        val exitCode = process.waitFor()
        return if (exitCode == 0 && output.isNotEmpty()) output else null
    }

    private fun curlDownload(url: String, destination: File) {
        val process = ProcessBuilder(
            "curl", "-sSL",
            "-H", "User-Agent: metal-analyzer-intellij-plugin",
            "-o", destination.absolutePath,
            url
        ).redirectErrorStream(true).start()
        val exitCode = process.waitFor()
        check(exitCode == 0) { "curl download failed with exit code $exitCode" }
    }

    private fun releaseAssetName(): String? {
        val arch = System.getProperty("os.arch") ?: return null
        if (SystemInfo.isWindows) {
            return when {
                arch == "aarch64" || arch == "arm64" -> "metal-analyzer-aarch64-pc-windows-msvc.zip"
                arch == "x86_64" || arch == "amd64" -> "metal-analyzer-x86_64-pc-windows-msvc.zip"
                else -> {
                    LOG.warn("Unsupported Windows architecture: $arch")
                    null
                }
            }
        }

        return when {
            arch == "aarch64" || arch == "arm64" -> "metal-analyzer-aarch64-apple-darwin.tar.gz"
            arch == "x86_64" || arch == "amd64" -> "metal-analyzer-x86_64-apple-darwin.tar.gz"
            else -> {
                LOG.warn("Unsupported macOS architecture: $arch")
                null
            }
        }
    }

    private fun extractZip(archivePath: Path, outputDir: Path) {
        val root = outputDir.toAbsolutePath().normalize()
        ZipInputStream(Files.newInputStream(archivePath)).use { zip ->
            generateSequence { zip.nextEntry }.forEach { entry ->
                val target = root.resolve(entry.name).normalize()
                check(target.startsWith(root)) { "Zip entry escapes target directory: ${entry.name}" }
                if (entry.isDirectory) {
                    Files.createDirectories(target)
                } else {
                    Files.createDirectories(target.parent)
                    Files.copy(zip, target, StandardCopyOption.REPLACE_EXISTING)
                }
            }
        }
    }

    private fun extractJsonString(json: String, key: String): String? {
        val pattern = """"$key"\s*:\s*"([^"]+)"""".toRegex()
        return pattern.find(json)?.groupValues?.get(1)
    }

    private fun extractAssetDownloadUrl(json: String, assetName: String): String? {
        val namePattern = """"name"\s*:\s*"${Regex.escape(assetName)}"""".toRegex()
        val nameMatch = namePattern.find(json) ?: return null
        val searchRegion = json.substring(maxOf(0, nameMatch.range.first - 2000), minOf(json.length, nameMatch.range.last + 2000))
        return extractJsonString(searchRegion, "browser_download_url")
    }

    private fun removeOutdatedVersions(currentTag: String) {
        val root = installRoot.toFile()
        if (!root.isDirectory) return
        val currentDirName = "$SERVER_NAME-$currentTag"
        root.listFiles()
            ?.filter { it.isDirectory && it.name.startsWith("$SERVER_NAME-") && it.name != currentDirName }
            ?.forEach { it.deleteRecursively() }
    }

    private fun runCommand(
        vararg command: String,
        stdinText: String? = null,
    ): CommandResult {
        return try {
            val process = ProcessBuilder(*command)
                .redirectErrorStream(true)
                .start()

            if (stdinText != null) {
                process.outputStream.bufferedWriter().use { writer ->
                    writer.write(stdinText)
                }
            } else {
                process.outputStream.close()
            }

            val finished = process.waitFor(TOOLCHAIN_CHECK_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            if (!finished) {
                process.destroyForcibly()
                return CommandResult(exitCode = -1, output = "command timed out: ${command.joinToString(" ")}")
            }

            CommandResult(exitCode = process.exitValue(), output = process.inputStream.bufferedReader().readText())
        } catch (e: Exception) {
            CommandResult(exitCode = -1, output = e.message ?: e.toString())
        }
    }

    private fun firstNonBlankLine(output: String): String? {
        return output.lineSequence().map(String::trim).firstOrNull { it.isNotEmpty() }
    }
}
