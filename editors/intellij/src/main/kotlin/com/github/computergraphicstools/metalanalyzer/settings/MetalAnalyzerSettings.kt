package com.github.computergraphicstools.metalanalyzer.settings

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.util.xmlb.XmlSerializerUtil

@Service(Service.Level.APP)
@State(name = "MetalAnalyzerSettings", storages = [Storage("MetalAnalyzerSettings.xml")])
class MetalAnalyzerSettings : PersistentStateComponent<MetalAnalyzerSettings> {

    // Server
    var serverPath: String = "metal-analyzer"

    // Formatting
    var formattingEnabled: Boolean = true
    var formattingCommand: String = "clang-format"
    var formattingArgs: MutableList<String> = mutableListOf()

    // Diagnostics
    var diagnosticsOnType: Boolean = true
    var diagnosticsOnSave: Boolean = true
    var diagnosticsDebounceMs: Int = 500
    var diagnosticsScope: String = "openFiles"

    // Indexing
    var indexingEnabled: Boolean = true
    var indexingConcurrency: Int = 1
    var indexingMaxFileSizeKb: Int = 512
    var indexingProjectGraphDepth: Int = 3
    var indexingProjectGraphMaxNodes: Int = 256
    var indexingExcludePaths: MutableList<String> = mutableListOf()

    // Compiler
    var compilerIncludePaths: MutableList<String> = mutableListOf()
    var compilerExtraFlags: MutableList<String> = mutableListOf()
    var compilerPlatform: String = "auto"

    // Logging
    var loggingLevel: String = "info"

    // Thread Pool
    var threadPoolWorkerThreads: Int = 0
    var threadPoolFormattingThreads: Int = 1

    override fun getState(): MetalAnalyzerSettings = this

    override fun loadState(state: MetalAnalyzerSettings) {
        XmlSerializerUtil.copyBean(state, this)
    }

    companion object {
        fun getInstance(): MetalAnalyzerSettings =
            ApplicationManager.getApplication().getService(MetalAnalyzerSettings::class.java)
    }
}
