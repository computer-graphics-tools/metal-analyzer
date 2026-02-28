package com.github.computergraphicstools.metalanalyzer.settings

import com.intellij.openapi.options.BoundConfigurable
import com.intellij.openapi.ui.DialogPanel
import com.intellij.ui.dsl.builder.*

class MetalAnalyzerConfigurable : BoundConfigurable("Metal Analyzer") {

    private val settings get() = MetalAnalyzerSettings.getInstance()

    override fun createPanel(): DialogPanel = panel {
        group("Server") {
            row("Server path:") {
                textField()
                    .bindText(settings::serverPath)
                    .comment("Path to the metal-analyzer binary, or \"metal-analyzer\" to use PATH / auto-download")
                    .columns(COLUMNS_LARGE)
            }
        }

        group("Formatting") {
            row {
                checkBox("Enable formatting")
                    .bindSelected(settings::formattingEnabled)
            }
            row("Formatter command:") {
                textField()
                    .bindText(settings::formattingCommand)
                    .columns(COLUMNS_MEDIUM)
            }
            row("Formatter arguments:") {
                textField()
                    .bindText(
                        { settings.formattingArgs.joinToString(" ") },
                        { settings.formattingArgs = it.split(" ").filter(String::isNotBlank).toMutableList() }
                    )
                    .comment("Space-separated arguments")
                    .columns(COLUMNS_LARGE)
            }
        }

        group("Diagnostics") {
            row {
                checkBox("Diagnose on type")
                    .bindSelected(settings::diagnosticsOnType)
            }
            row {
                checkBox("Diagnose on save")
                    .bindSelected(settings::diagnosticsOnSave)
            }
            row("Debounce (ms):") {
                spinner(50..5000, 50)
                    .bindIntValue(settings::diagnosticsDebounceMs)
            }
            row("Scope:") {
                comboBox(listOf("openFiles", "workspace"))
                    .bindItem(settings::diagnosticsScope.toNullableProperty())
            }
        }

        group("Indexing") {
            row {
                checkBox("Enable indexing")
                    .bindSelected(settings::indexingEnabled)
            }
            row("Concurrency:") {
                spinner(1..32)
                    .bindIntValue(settings::indexingConcurrency)
            }
            row("Max file size (KB):") {
                spinner(16..65536, 64)
                    .bindIntValue(settings::indexingMaxFileSizeKb)
            }
            row("Project graph depth:") {
                spinner(0..8)
                    .bindIntValue(settings::indexingProjectGraphDepth)
            }
            row("Project graph max nodes:") {
                spinner(16..4096, 16)
                    .bindIntValue(settings::indexingProjectGraphMaxNodes)
            }
            row("Exclude paths:") {
                textField()
                    .bindText(
                        { settings.indexingExcludePaths.joinToString(", ") },
                        { settings.indexingExcludePaths = it.split(",").map(String::trim).filter(String::isNotBlank).toMutableList() }
                    )
                    .comment("Comma-separated list of paths to exclude")
                    .columns(COLUMNS_LARGE)
            }
        }

        group("Compiler") {
            row("Include paths:") {
                textField()
                    .bindText(
                        { settings.compilerIncludePaths.joinToString(", ") },
                        { settings.compilerIncludePaths = it.split(",").map(String::trim).filter(String::isNotBlank).toMutableList() }
                    )
                    .comment("Comma-separated additional include paths")
                    .columns(COLUMNS_LARGE)
            }
            row("Extra flags:") {
                textField()
                    .bindText(
                        { settings.compilerExtraFlags.joinToString(" ") },
                        { settings.compilerExtraFlags = it.split(" ").filter(String::isNotBlank).toMutableList() }
                    )
                    .comment("Space-separated extra compiler flags")
                    .columns(COLUMNS_LARGE)
            }
            row("Platform:") {
                comboBox(listOf("auto", "macos", "ios", "tvos", "watchos", "xros"))
                    .bindItem(settings::compilerPlatform.toNullableProperty())
            }
        }

        group("Logging") {
            row("Level:") {
                comboBox(listOf("error", "warn", "info", "debug", "trace"))
                    .bindItem(settings::loggingLevel.toNullableProperty())
            }
        }

        group("Thread Pool") {
            row("Worker threads:") {
                spinner(0..64)
                    .bindIntValue(settings::threadPoolWorkerThreads)
                    .comment("0 = use available parallelism (requires server restart)")
            }
            row("Formatting threads:") {
                spinner(1..8)
                    .bindIntValue(settings::threadPoolFormattingThreads)
                    .comment("Requires server restart")
            }
        }
    }
}
