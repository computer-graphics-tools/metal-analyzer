package com.github.computergraphicstools.metalanalyzer.lsp

import com.github.computergraphicstools.metalanalyzer.server.ServerBinaryManager
import com.intellij.notification.Notification
import com.intellij.notification.NotificationType
import com.intellij.notification.Notifications
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.util.Key
import com.intellij.openapi.util.text.StringUtil
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.platform.lsp.api.LspServerSupportProvider.LspServerStarter

class MetalLspServerSupportProvider : LspServerSupportProvider {
    override fun fileOpened(project: Project, file: VirtualFile, serverStarter: LspServerStarter) {
        if (file.extension != "metal") {
            return
        }
        serverStarter.ensureServerStarted(MetalLspServerDescriptor(project))
        maybeNotifyMissingToolchain(project)
    }

    private fun maybeNotifyMissingToolchain(project: Project) {
        if (project.getUserData(TOOLCHAIN_CHECK_STARTED_KEY) == true) {
            return
        }
        project.putUserData(TOOLCHAIN_CHECK_STARTED_KEY, true)

        ApplicationManager.getApplication().executeOnPooledThread {
            val status = ServerBinaryManager.checkToolchainAvailability()
            if (status.available) {
                return@executeOnPooledThread
            }

            val details = status.reason?.let { reason ->
                "<br/><br/>Details: <code>${StringUtil.escapeXmlEntities(reason)}</code>"
            } ?: ""

            Notifications.Bus.notify(
                Notification(
                    NOTIFICATION_GROUP_ID,
                    "Metal toolchain missing",
                    "Metal compiler toolchain or SDK is unavailable. Diagnostics, indexing, and go-to-definition require both.<br/>" +
                        "Try:<br/>" +
                        "<code>xcode-select --install</code><br/>" +
                        "<code>sudo xcode-select -s /Applications/Xcode.app/Contents/Developer</code><br/>" +
                        "<code>xcodebuild -downloadComponent MetalToolchain</code>$details",
                    NotificationType.ERROR,
                ),
                project,
            )
        }
    }

    companion object {
        private val TOOLCHAIN_CHECK_STARTED_KEY = Key.create<Boolean>("metal.analyzer.toolchain.check.started")
        private const val NOTIFICATION_GROUP_ID = "Metal Analyzer"
    }
}
