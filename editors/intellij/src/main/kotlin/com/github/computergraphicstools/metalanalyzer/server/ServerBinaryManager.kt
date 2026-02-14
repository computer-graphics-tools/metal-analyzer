package com.github.computergraphicstools.metalanalyzer.server

import com.github.computergraphicstools.metalanalyzer.settings.MetalAnalyzerSettings
import com.intellij.openapi.application.PathManager
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.util.SystemInfo
import java.io.File
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.attribute.PosixFilePermissions
import java.util.concurrent.TimeUnit

object ServerBinaryManager {
    private val LOG = Logger.getInstance(ServerBinaryManager::class.java)
    private const val SERVER_NAME = "metal-analyzer"
    private const val GITHUB_REPO = "computer-graphics-tools/metal-analyzer"
    private const val GITHUB_LATEST_RELEASE_API = "https://api.github.com/repos/$GITHUB_REPO/releases/latest"
    private const val TOOLCHAIN_CHECK_TIMEOUT_SECONDS = 8L

    private val installRoot: Path
        get() = Path.of(PathManager.getSystemPath(), SERVER_NAME)

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

        findOnPath()?.let { return it }

        findCachedBinary()?.let { cached ->
            tryDownloadLatest()?.let { return it }
            return cached
        }

        tryDownloadLatest()?.let { return it }

        throw IllegalStateException(
            "Could not locate $SERVER_NAME. Install it manually and set the server path in Settings > Languages & Frameworks > Metal Analyzer."
        )
    }

    fun checkToolchainAvailability(): ToolchainStatus {
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

    private fun findOnPath(): String? {
        return try {
            val process = ProcessBuilder("which", SERVER_NAME)
                .redirectErrorStream(true)
                .start()
            val output = process.inputStream.bufferedReader().readText().trim()
            val exitCode = process.waitFor()
            if (exitCode == 0 && output.isNotEmpty() && File(output).canExecute()) output else null
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
                val binary = File(dir, SERVER_NAME)
                if (binary.canExecute()) binary.absolutePath else null
            }
    }

    private fun tryDownloadLatest(): String? {
        if (!SystemInfo.isMac) {
            LOG.info("Auto-download is only supported on macOS")
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
        val downloadUrl = extractAssetDownloadUrl(releaseJson, assetName) ?: return null

        val versionDir = installRoot.resolve("$SERVER_NAME-$tagName")
        val binaryPath = versionDir.resolve(SERVER_NAME)

        if (Files.isExecutable(binaryPath)) {
            return binaryPath.toString()
        }

        Files.createDirectories(versionDir)
        val archivePath = versionDir.resolve(assetName)

        curlDownload(downloadUrl, archivePath.toFile())

        ProcessBuilder("tar", "-xzf", archivePath.toString(), "-C", versionDir.toString())
            .redirectErrorStream(true)
            .start()
            .waitFor()

        Files.setPosixFilePermissions(binaryPath, PosixFilePermissions.fromString("rwxr-xr-x"))
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
        return when {
            arch == "aarch64" || arch == "arm64" -> "metal-analyzer-aarch64-apple-darwin.tar.gz"
            arch == "x86_64" || arch == "amd64" -> "metal-analyzer-x86_64-apple-darwin.tar.gz"
            else -> {
                LOG.warn("Unsupported macOS architecture: $arch")
                null
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
