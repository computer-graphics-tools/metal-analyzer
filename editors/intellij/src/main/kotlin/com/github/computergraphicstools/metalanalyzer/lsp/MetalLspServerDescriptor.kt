package com.github.computergraphicstools.metalanalyzer.lsp

import com.github.computergraphicstools.metalanalyzer.server.ServerBinaryManager
import com.github.computergraphicstools.metalanalyzer.settings.MetalAnalyzerSettings
import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.ProjectWideLspServerDescriptor

class MetalLspServerDescriptor(project: Project) :
    ProjectWideLspServerDescriptor(project, "Metal Analyzer") {

    override fun isSupportedFile(file: VirtualFile): Boolean =
        file.extension == "metal"

    override fun createCommandLine(): GeneralCommandLine {
        val serverPath = ServerBinaryManager.resolveServerPath()
        return GeneralCommandLine(serverPath)
    }

    override fun createInitializationOptions(): Any {
        val settings = MetalAnalyzerSettings.getInstance()
        return buildInitializationOptions(settings)
    }

    companion object {
        fun buildInitializationOptions(settings: MetalAnalyzerSettings): Map<String, Any> {
            return mapOf(
                "metal-analyzer" to mapOf(
                    "formatting" to mapOf(
                        "enabled" to settings.formattingEnabled,
                        "command" to settings.formattingCommand,
                        "args" to settings.formattingArgs,
                    ),
                    "diagnostics" to mapOf(
                        "onType" to settings.diagnosticsOnType,
                        "onSave" to settings.diagnosticsOnSave,
                        "debounceMs" to settings.diagnosticsDebounceMs,
                        "scope" to settings.diagnosticsScope,
                    ),
                    "indexing" to mapOf(
                        "enabled" to settings.indexingEnabled,
                        "concurrency" to settings.indexingConcurrency,
                        "maxFileSizeKb" to settings.indexingMaxFileSizeKb,
                        "projectGraphDepth" to settings.indexingProjectGraphDepth,
                        "projectGraphMaxNodes" to settings.indexingProjectGraphMaxNodes,
                        "excludePaths" to settings.indexingExcludePaths,
                    ),
                    "compiler" to mapOf(
                        "includePaths" to settings.compilerIncludePaths,
                        "extraFlags" to settings.compilerExtraFlags,
                        "platform" to settings.compilerPlatform,
                    ),
                    "logging" to mapOf(
                        "level" to settings.loggingLevel,
                    ),
                    "threadPool" to mapOf(
                        "workerThreads" to settings.threadPoolWorkerThreads,
                        "formattingThreads" to settings.threadPoolFormattingThreads,
                    ),
                )
            )
        }
    }
}
