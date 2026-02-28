"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
const node_child_process_1 = require("node:child_process");
const node_fs_1 = require("node:fs");
const fs = __importStar(require("node:fs/promises"));
const https = __importStar(require("node:https"));
const os = __importStar(require("node:os"));
const path = __importStar(require("node:path"));
const promises_1 = require("node:stream/promises");
const node_util_1 = require("node:util");
let client;
let isDeactivating = false;
let isRestartingClient = false;
let hasShownUnexpectedShutdownNotice = false;
let clientStateSubscription;
const execFileAsync = (0, node_util_1.promisify)(node_child_process_1.execFile);
const SERVER_NAME = "metal-analyzer";
const GITHUB_REPO = "computer-graphics-tools/metal-analyzer";
const GITHUB_LATEST_RELEASE_API = `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`;
async function activate(context) {
    isDeactivating = false;
    isRestartingClient = false;
    hasShownUnexpectedShutdownNotice = false;
    await recreateClientForConfiguration(context);
    context.subscriptions.push(vscode.commands.registerCommand("metal-analyzer.startServer", () => {
        return startClient(context);
    }), vscode.commands.registerCommand("metal-analyzer.stopServer", () => {
        return stopClient();
    }), vscode.commands.registerCommand("metal-analyzer.restartServer", () => {
        return restartClient();
    }), vscode.commands.registerCommand("metal-analyzer.showOutput", () => {
        client?.outputChannel.show();
    }), vscode.commands.registerCommand("metal-analyzer.openLogs", () => {
        return openLogFile();
    }), vscode.commands.registerCommand("metal-analyzer.serverVersion", () => {
        return showServerVersion();
    }));
    context.subscriptions.push(vscode.workspace.onDidChangeConfiguration((event) => {
        const requiresRestart = event.affectsConfiguration("metal-analyzer.serverPath") ||
            event.affectsConfiguration("metal-analyzer.threadPool.workerThreads") ||
            event.affectsConfiguration("metal-analyzer.threadPool.formattingThreads");
        if (!requiresRestart) {
            return;
        }
        void recreateClientForConfiguration(context);
    }));
}
async function deactivate() {
    isDeactivating = true;
    if (client) {
        await client.stop();
        client = undefined;
    }
}
async function notifyUnexpectedShutdown() {
    if (hasShownUnexpectedShutdownNotice) {
        return;
    }
    hasShownUnexpectedShutdownNotice = true;
    const action = await vscode.window.showErrorMessage("metal-analyzer shut down unexpectedly. Metal language features are unavailable until the server restarts.", "Restart metal-analyzer");
    if (action === "Restart metal-analyzer") {
        await restartClient();
    }
}
async function restartClient() {
    if (!client || isRestartingClient) {
        return;
    }
    isRestartingClient = true;
    try {
        if (client.state !== node_1.State.Stopped) {
            await client.stop();
        }
        await client.start();
    }
    catch (error) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Failed to restart metal-analyzer: ${errorMessage}`);
    }
    finally {
        isRestartingClient = false;
    }
}
async function startClient(context) {
    if (client && client.state === node_1.State.Running) {
        return;
    }
    if (client) {
        try {
            await client.start();
        }
        catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`Failed to start metal-analyzer: ${errorMessage}`);
        }
        return;
    }
    await recreateClientForConfiguration(context);
}
async function stopClient() {
    if (!client || client.state === node_1.State.Stopped) {
        return;
    }
    try {
        await client.stop();
    }
    catch (error) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Failed to stop metal-analyzer: ${errorMessage}`);
    }
}
async function openLogFile() {
    const logPath = path.join(os.homedir(), ".metal-analyzer", "metal-analyzer.log");
    try {
        await fs.access(logPath);
        const uri = vscode.Uri.file(logPath);
        await vscode.window.showTextDocument(uri);
    }
    catch {
        void vscode.window.showWarningMessage(`Log file not found: ${logPath}`);
    }
}
function showServerVersion() {
    if (!client || client.state !== node_1.State.Running) {
        void vscode.window.showInformationMessage("metal-analyzer: server is not running");
        return;
    }
    const serverInfo = client.initializeResult?.serverInfo;
    const version = serverInfo?.version ?? "unknown";
    const name = serverInfo?.name ?? "metal-analyzer";
    void vscode.window.showInformationMessage(`${name} v${version}`);
}
function createLanguageClient(serverPath) {
    const initializationOptions = buildServerInitializationOptions();
    const serverOptions = {
        command: serverPath,
        args: [],
    };
    const clientOptions = {
        documentSelector: [{ scheme: "file", language: "metal" }],
        initializationOptions,
        synchronize: {
            configurationSection: "metal-analyzer",
        },
        outputChannelName: "metal-analyzer",
        traceOutputChannel: vscode.window.createOutputChannel("metal-analyzer (LSP Trace)"),
    };
    return new node_1.LanguageClient("metal-analyzer", "metal-analyzer", serverOptions, clientOptions);
}
function buildServerInitializationOptions() {
    const config = vscode.workspace.getConfiguration("metal-analyzer");
    return {
        "metal-analyzer": {
            formatting: {
                enabled: config.get("formatting.enabled", true),
                command: config.get("formatting.command", "clang-format"),
                args: config.get("formatting.args", []),
            },
            diagnostics: {
                onType: config.get("diagnostics.onType", true),
                onSave: config.get("diagnostics.onSave", true),
                debounceMs: config.get("diagnostics.debounceMs", 500),
                scope: config.get("diagnostics.scope", "openFiles"),
            },
            indexing: {
                enabled: config.get("indexing.enabled", true),
                concurrency: config.get("indexing.concurrency", 1),
                maxFileSizeKb: config.get("indexing.maxFileSizeKb", 512),
                projectGraphDepth: config.get("indexing.projectGraphDepth", 3),
                projectGraphMaxNodes: config.get("indexing.projectGraphMaxNodes", 256),
                excludePaths: config.get("indexing.excludePaths", []),
            },
            compiler: {
                includePaths: config.get("compiler.includePaths", []),
                extraFlags: config.get("compiler.extraFlags", []),
                platform: config.get("compiler.platform", "auto"),
            },
            logging: {
                level: config.get("logging.level", "info"),
            },
            threadPool: {
                workerThreads: config.get("threadPool.workerThreads", 0),
                formattingThreads: config.get("threadPool.formattingThreads", 1),
            },
        },
    };
}
function registerClientStateSubscription(context, languageClient) {
    clientStateSubscription?.dispose();
    clientStateSubscription = languageClient.onDidChangeState((event) => {
        if (event.newState === node_1.State.Running) {
            hasShownUnexpectedShutdownNotice = false;
            return;
        }
        const stoppedUnexpectedly = event.newState === node_1.State.Stopped &&
            !isDeactivating &&
            !isRestartingClient;
        if (!stoppedUnexpectedly) {
            return;
        }
        void notifyUnexpectedShutdown();
    });
    context.subscriptions.push(clientStateSubscription);
}
async function recreateClientForConfiguration(context) {
    if (isRestartingClient) {
        return;
    }
    isRestartingClient = true;
    try {
        const config = vscode.workspace.getConfiguration("metal-analyzer");
        const configuredServerPath = config.get("serverPath", SERVER_NAME);
        const serverPath = await resolveServerPath(context, configuredServerPath);
        const oldClient = client;
        if (oldClient) {
            if (oldClient.state !== node_1.State.Stopped) {
                await oldClient.stop();
            }
            await oldClient.dispose();
        }
        client = createLanguageClient(serverPath);
        registerClientStateSubscription(context, client);
        await client.start();
    }
    catch (error) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Failed to start metal-analyzer with updated configuration: ${errorMessage}`);
    }
    finally {
        isRestartingClient = false;
    }
}
async function resolveServerPath(context, configuredServerPath) {
    if (configuredServerPath !== SERVER_NAME) {
        return configuredServerPath;
    }
    if (await isCommandAvailable(SERVER_NAME)) {
        return SERVER_NAME;
    }
    const installRoot = getInstallRoot(context);
    const cachedBinaryPath = await findNewestInstalledBinary(installRoot);
    try {
        return await vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: "Installing metal-analyzer",
        }, async () => ensureLatestDownloadedBinary(installRoot));
    }
    catch (error) {
        if (cachedBinaryPath) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            vscode.window.showWarningMessage(`metal-analyzer update failed, using cached binary: ${errorMessage}`);
            return cachedBinaryPath;
        }
        const errorMessage = error instanceof Error ? error.message : String(error);
        throw new Error(`Unable to locate ${SERVER_NAME}: ${errorMessage}`);
    }
}
function getInstallRoot(context) {
    return path.join(context.globalStorageUri.fsPath, SERVER_NAME);
}
async function isCommandAvailable(command) {
    const lookupCommand = process.platform === "win32" ? "where" : "which";
    try {
        await execFileAsync(lookupCommand, [command]);
        return true;
    }
    catch {
        return false;
    }
}
function releaseAssetNameForCurrentPlatform() {
    if (process.platform !== "darwin") {
        throw new Error("metal-analyzer auto-install currently supports macOS only. Install manually and set metal-analyzer.serverPath.");
    }
    if (process.arch === "arm64") {
        return "metal-analyzer-aarch64-apple-darwin.tar.gz";
    }
    if (process.arch === "x64") {
        return "metal-analyzer-x86_64-apple-darwin.tar.gz";
    }
    throw new Error(`Unsupported macOS architecture: ${process.arch}. Install manually and set metal-analyzer.serverPath.`);
}
async function ensureLatestDownloadedBinary(installRoot) {
    await fs.mkdir(installRoot, { recursive: true });
    const release = await fetchLatestRelease();
    const assetName = releaseAssetNameForCurrentPlatform();
    const matchingAsset = release.assets.find((asset) => asset.name === assetName);
    if (!matchingAsset) {
        throw new Error(`No release asset found for ${assetName}`);
    }
    const versionDirName = `${SERVER_NAME}-${release.tag_name}`;
    const versionDirPath = path.join(installRoot, versionDirName);
    const binaryPath = path.join(versionDirPath, SERVER_NAME);
    if (await isExecutable(binaryPath)) {
        return binaryPath;
    }
    await fs.mkdir(versionDirPath, { recursive: true });
    const archivePath = path.join(versionDirPath, assetName);
    await downloadFile(matchingAsset.browser_download_url, archivePath);
    await extractArchive(archivePath, versionDirPath);
    await fs.chmod(binaryPath, 0o755);
    await fs.rm(archivePath, { force: true });
    await removeOutdatedInstalledVersions(installRoot, versionDirName);
    return binaryPath;
}
async function fetchLatestRelease() {
    const payload = await httpGetBuffer(GITHUB_LATEST_RELEASE_API, {
        Accept: "application/vnd.github+json",
        "User-Agent": "metal-analyzer-vscode-extension",
    });
    let parsedPayload;
    try {
        parsedPayload = JSON.parse(payload.toString("utf8"));
    }
    catch (error) {
        throw new Error(`Failed to parse GitHub release metadata: ${error instanceof Error ? error.message : String(error)}`);
    }
    if (!isGithubRelease(parsedPayload)) {
        throw new Error("GitHub release metadata is missing required fields");
    }
    return parsedPayload;
}
function isGithubRelease(value) {
    if (typeof value !== "object" || value === null) {
        return false;
    }
    const candidate = value;
    if (typeof candidate.tag_name !== "string" ||
        !Array.isArray(candidate.assets)) {
        return false;
    }
    return candidate.assets.every((asset) => typeof asset?.name === "string" &&
        typeof asset?.browser_download_url === "string");
}
async function httpGetBuffer(url, headers) {
    return new Promise((resolve, reject) => {
        const request = https.get(url, { headers }, (response) => {
            const statusCode = response.statusCode ?? 0;
            if ([301, 302, 303, 307, 308].includes(statusCode) &&
                typeof response.headers.location === "string") {
                const redirectedUrl = new URL(response.headers.location, url).toString();
                response.resume();
                httpGetBuffer(redirectedUrl, headers).then(resolve).catch(reject);
                return;
            }
            if (statusCode !== 200) {
                response.resume();
                reject(new Error(`GitHub API request failed with status ${statusCode}`));
                return;
            }
            const chunks = [];
            response.on("data", (chunk) => {
                chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
            });
            response.on("end", () => resolve(Buffer.concat(chunks)));
            response.on("error", reject);
        });
        request.on("error", reject);
    });
}
async function downloadFile(url, destinationPath) {
    return new Promise((resolve, reject) => {
        const request = https.get(url, {
            headers: {
                Accept: "application/octet-stream",
                "User-Agent": "metal-analyzer-vscode-extension",
            },
        }, (response) => {
            const statusCode = response.statusCode ?? 0;
            if ([301, 302, 303, 307, 308].includes(statusCode) &&
                typeof response.headers.location === "string") {
                const redirectedUrl = new URL(response.headers.location, url).toString();
                response.resume();
                downloadFile(redirectedUrl, destinationPath)
                    .then(resolve)
                    .catch(reject);
                return;
            }
            if (statusCode !== 200) {
                response.resume();
                reject(new Error(`Download failed with status ${statusCode}`));
                return;
            }
            const fileStream = (0, node_fs_1.createWriteStream)(destinationPath);
            (0, promises_1.pipeline)(response, fileStream).then(resolve).catch(reject);
        });
        request.on("error", reject);
    });
}
async function extractArchive(archivePath, outputDirectory) {
    await execFileAsync("tar", ["-xzf", archivePath, "-C", outputDirectory]);
}
async function isExecutable(filePath) {
    try {
        await fs.access(filePath, node_fs_1.constants.X_OK);
        return true;
    }
    catch {
        return false;
    }
}
async function findNewestInstalledBinary(installRoot) {
    let entries = [];
    try {
        const directoryEntries = await fs.readdir(installRoot, {
            withFileTypes: true,
        });
        const candidateStats = await Promise.all(directoryEntries
            .filter((entry) => entry.isDirectory() && entry.name.startsWith(`${SERVER_NAME}-`))
            .map(async (entry) => {
            const entryPath = path.join(installRoot, entry.name);
            const stat = await fs.stat(entryPath);
            return { path: entryPath, modifiedAtMs: stat.mtimeMs };
        }));
        entries = candidateStats;
    }
    catch {
        return undefined;
    }
    const sortedEntries = entries.sort((left, right) => right.modifiedAtMs - left.modifiedAtMs);
    for (const entry of sortedEntries) {
        const candidateBinaryPath = path.join(entry.path, SERVER_NAME);
        if (await isExecutable(candidateBinaryPath)) {
            return candidateBinaryPath;
        }
    }
    return undefined;
}
async function removeOutdatedInstalledVersions(installRoot, currentVersionDirName) {
    const entries = await fs.readdir(installRoot, { withFileTypes: true });
    await Promise.all(entries
        .filter((entry) => entry.isDirectory() &&
        entry.name.startsWith(`${SERVER_NAME}-`) &&
        entry.name !== currentVersionDirName)
        .map((entry) => fs.rm(path.join(installRoot, entry.name), {
        recursive: true,
        force: true,
    })));
}
//# sourceMappingURL=extension.js.map