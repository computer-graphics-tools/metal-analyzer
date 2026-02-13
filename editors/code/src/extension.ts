import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  State,
} from "vscode-languageclient/node";
import { execFile } from "node:child_process";
import { constants as fsConstants, createWriteStream } from "node:fs";
import * as fs from "node:fs/promises";
import * as https from "node:https";
import * as os from "node:os";
import * as path from "node:path";
import { pipeline } from "node:stream/promises";
import { promisify } from "node:util";

let client: LanguageClient | undefined;
let isDeactivating = false;
let isRestartingClient = false;
let hasShownUnexpectedShutdownNotice = false;
let clientStateSubscription: vscode.Disposable | undefined;
const execFileAsync = promisify(execFile);

const SERVER_NAME = "metal-analyzer";
const GITHUB_REPO = "computer-graphics-tools/metal-analyzer";
const GITHUB_LATEST_RELEASE_API = `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`;

type GithubReleaseAsset = {
  name: string;
  browser_download_url: string;
};

type GithubRelease = {
  tag_name: string;
  assets: GithubReleaseAsset[];
};

export async function activate(context: vscode.ExtensionContext) {
  isDeactivating = false;
  isRestartingClient = false;
  hasShownUnexpectedShutdownNotice = false;

  await recreateClientForConfiguration(context);

  context.subscriptions.push(
    vscode.commands.registerCommand("metal-analyzer.startServer", () => {
      return startClient(context);
    }),
    vscode.commands.registerCommand("metal-analyzer.stopServer", () => {
      return stopClient();
    }),
    vscode.commands.registerCommand("metal-analyzer.restartServer", () => {
      return restartClient();
    }),
    vscode.commands.registerCommand("metal-analyzer.showOutput", () => {
      client?.outputChannel.show();
    }),
    vscode.commands.registerCommand("metal-analyzer.openLogs", () => {
      return openLogFile();
    }),
    vscode.commands.registerCommand("metal-analyzer.serverVersion", () => {
      return showServerVersion();
    }),
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      const requiresRestart =
        event.affectsConfiguration("metal-analyzer.serverPath") ||
        event.affectsConfiguration("metal-analyzer.threadPool.workerThreads") ||
        event.affectsConfiguration(
          "metal-analyzer.threadPool.formattingThreads",
        );
      if (!requiresRestart) {
        return;
      }

      void recreateClientForConfiguration(context);
    }),
  );
}

export async function deactivate() {
  isDeactivating = true;

  if (client) {
    await client.stop();
    client = undefined;
  }
}

async function notifyUnexpectedShutdown(): Promise<void> {
  if (hasShownUnexpectedShutdownNotice) {
    return;
  }
  hasShownUnexpectedShutdownNotice = true;

  const action = await vscode.window.showErrorMessage(
    "metal-analyzer shut down unexpectedly. Metal language features are unavailable until the server restarts.",
    "Restart metal-analyzer",
  );

  if (action === "Restart metal-analyzer") {
    await restartClient();
  }
}

async function restartClient(): Promise<void> {
  if (!client || isRestartingClient) {
    return;
  }

  isRestartingClient = true;
  try {
    if (client.state !== State.Stopped) {
      await client.stop();
    }
    await client.start();
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(
      `Failed to restart metal-analyzer: ${errorMessage}`,
    );
  } finally {
    isRestartingClient = false;
  }
}

async function startClient(context: vscode.ExtensionContext): Promise<void> {
  if (client && client.state === State.Running) {
    return;
  }

  if (client) {
    try {
      await client.start();
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(
        `Failed to start metal-analyzer: ${errorMessage}`,
      );
    }
    return;
  }

  await recreateClientForConfiguration(context);
}

async function stopClient(): Promise<void> {
  if (!client || client.state === State.Stopped) {
    return;
  }

  try {
    await client.stop();
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(
      `Failed to stop metal-analyzer: ${errorMessage}`,
    );
  }
}

async function openLogFile(): Promise<void> {
  const logPath = path.join(
    os.homedir(),
    ".metal-analyzer",
    "metal-analyzer.log",
  );
  try {
    await fs.access(logPath);
    const uri = vscode.Uri.file(logPath);
    await vscode.window.showTextDocument(uri);
  } catch {
    void vscode.window.showWarningMessage(`Log file not found: ${logPath}`);
  }
}

function showServerVersion(): void {
  if (!client || client.state !== State.Running) {
    void vscode.window.showInformationMessage(
      "metal-analyzer: server is not running",
    );
    return;
  }

  const serverInfo = client.initializeResult?.serverInfo;
  const version = serverInfo?.version ?? "unknown";
  const name = serverInfo?.name ?? "metal-analyzer";
  void vscode.window.showInformationMessage(`${name} v${version}`);
}

function createLanguageClient(serverPath: string): LanguageClient {
  const initializationOptions = buildServerInitializationOptions();
  const serverOptions: ServerOptions = {
    command: serverPath,
    args: [],
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "metal" }],
    initializationOptions,
    synchronize: {
      configurationSection: "metal-analyzer",
    },
  };

  return new LanguageClient(
    "metal-analyzer",
    "metal-analyzer",
    serverOptions,
    clientOptions,
  );
}

function buildServerInitializationOptions(): Record<string, unknown> {
  const config = vscode.workspace.getConfiguration("metal-analyzer");

  return {
    "metal-analyzer": {
      formatting: {
        enabled: config.get<boolean>("formatting.enabled", true),
        command: config.get<string>("formatting.command", "clang-format"),
        args: config.get<string[]>("formatting.args", []),
      },
      diagnostics: {
        onType: config.get<boolean>("diagnostics.onType", true),
        onSave: config.get<boolean>("diagnostics.onSave", true),
        debounceMs: config.get<number>("diagnostics.debounceMs", 500),
        scope: config.get<string>("diagnostics.scope", "openFiles"),
      },
      indexing: {
        enabled: config.get<boolean>("indexing.enabled", true),
        concurrency: config.get<number>("indexing.concurrency", 1),
        maxFileSizeKb: config.get<number>("indexing.maxFileSizeKb", 512),
        projectGraphDepth: config.get<number>("indexing.projectGraphDepth", 3),
        projectGraphMaxNodes: config.get<number>(
          "indexing.projectGraphMaxNodes",
          256,
        ),
        excludePaths: config.get<string[]>("indexing.excludePaths", []),
      },
      compiler: {
        includePaths: config.get<string[]>("compiler.includePaths", []),
        extraFlags: config.get<string[]>("compiler.extraFlags", []),
        platform: config.get<string>("compiler.platform", "auto"),
      },
      logging: {
        level: config.get<string>("logging.level", "info"),
      },
      threadPool: {
        workerThreads: config.get<number>("threadPool.workerThreads", 0),
        formattingThreads: config.get<number>(
          "threadPool.formattingThreads",
          1,
        ),
      },
    },
  };
}

function registerClientStateSubscription(
  context: vscode.ExtensionContext,
  languageClient: LanguageClient,
): void {
  clientStateSubscription?.dispose();
  clientStateSubscription = languageClient.onDidChangeState((event) => {
    if (event.newState === State.Running) {
      hasShownUnexpectedShutdownNotice = false;
      return;
    }

    const stoppedUnexpectedly =
      event.newState === State.Stopped &&
      !isDeactivating &&
      !isRestartingClient;
    if (!stoppedUnexpectedly) {
      return;
    }

    void notifyUnexpectedShutdown();
  });
  context.subscriptions.push(clientStateSubscription);
}

async function recreateClientForConfiguration(
  context: vscode.ExtensionContext,
): Promise<void> {
  if (isRestartingClient) {
    return;
  }

  isRestartingClient = true;
  try {
    const config = vscode.workspace.getConfiguration("metal-analyzer");
    const configuredServerPath = config.get<string>("serverPath", SERVER_NAME);
    const serverPath = await resolveServerPath(context, configuredServerPath);

    const oldClient = client;
    if (oldClient) {
      if (oldClient.state !== State.Stopped) {
        await oldClient.stop();
      }
      await oldClient.dispose();
    }

    client = createLanguageClient(serverPath);
    registerClientStateSubscription(context, client);
    await client.start();
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(
      `Failed to start metal-analyzer with updated configuration: ${errorMessage}`,
    );
  } finally {
    isRestartingClient = false;
  }
}

async function resolveServerPath(
  context: vscode.ExtensionContext,
  configuredServerPath: string,
): Promise<string> {
  if (configuredServerPath !== SERVER_NAME) {
    return configuredServerPath;
  }

  if (await isCommandAvailable(SERVER_NAME)) {
    return SERVER_NAME;
  }

  const installRoot = getInstallRoot(context);
  const cachedBinaryPath = await findNewestInstalledBinary(installRoot);

  try {
    return await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "Installing metal-analyzer",
      },
      async () => ensureLatestDownloadedBinary(installRoot),
    );
  } catch (error) {
    if (cachedBinaryPath) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      vscode.window.showWarningMessage(
        `metal-analyzer update failed, using cached binary: ${errorMessage}`,
      );
      return cachedBinaryPath;
    }

    const errorMessage = error instanceof Error ? error.message : String(error);
    throw new Error(`Unable to locate ${SERVER_NAME}: ${errorMessage}`);
  }
}

function getInstallRoot(context: vscode.ExtensionContext): string {
  return path.join(context.globalStorageUri.fsPath, SERVER_NAME);
}

async function isCommandAvailable(command: string): Promise<boolean> {
  const lookupCommand = process.platform === "win32" ? "where" : "which";
  try {
    await execFileAsync(lookupCommand, [command]);
    return true;
  } catch {
    return false;
  }
}

function releaseAssetNameForCurrentPlatform(): string {
  if (process.platform !== "darwin") {
    throw new Error(
      "metal-analyzer auto-install currently supports macOS only. Install manually and set metal-analyzer.serverPath.",
    );
  }

  if (process.arch === "arm64") {
    return "metal-analyzer-aarch64-apple-darwin.tar.gz";
  }
  if (process.arch === "x64") {
    return "metal-analyzer-x86_64-apple-darwin.tar.gz";
  }

  throw new Error(
    `Unsupported macOS architecture: ${process.arch}. Install manually and set metal-analyzer.serverPath.`,
  );
}

async function ensureLatestDownloadedBinary(
  installRoot: string,
): Promise<string> {
  await fs.mkdir(installRoot, { recursive: true });

  const release = await fetchLatestRelease();
  const assetName = releaseAssetNameForCurrentPlatform();
  const matchingAsset = release.assets.find(
    (asset) => asset.name === assetName,
  );
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

async function fetchLatestRelease(): Promise<GithubRelease> {
  const payload = await httpGetBuffer(GITHUB_LATEST_RELEASE_API, {
    Accept: "application/vnd.github+json",
    "User-Agent": "metal-analyzer-vscode-extension",
  });

  let parsedPayload: unknown;
  try {
    parsedPayload = JSON.parse(payload.toString("utf8"));
  } catch (error) {
    throw new Error(
      `Failed to parse GitHub release metadata: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }

  if (!isGithubRelease(parsedPayload)) {
    throw new Error("GitHub release metadata is missing required fields");
  }

  return parsedPayload;
}

function isGithubRelease(value: unknown): value is GithubRelease {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const candidate = value as Partial<GithubRelease>;
  if (
    typeof candidate.tag_name !== "string" ||
    !Array.isArray(candidate.assets)
  ) {
    return false;
  }

  return candidate.assets.every(
    (asset) =>
      typeof asset?.name === "string" &&
      typeof asset?.browser_download_url === "string",
  );
}

async function httpGetBuffer(
  url: string,
  headers: Record<string, string>,
): Promise<Buffer> {
  return new Promise<Buffer>((resolve, reject) => {
    const request = https.get(url, { headers }, (response) => {
      const statusCode = response.statusCode ?? 0;
      if (
        [301, 302, 303, 307, 308].includes(statusCode) &&
        typeof response.headers.location === "string"
      ) {
        const redirectedUrl = new URL(
          response.headers.location,
          url,
        ).toString();
        response.resume();
        httpGetBuffer(redirectedUrl, headers).then(resolve).catch(reject);
        return;
      }

      if (statusCode !== 200) {
        response.resume();
        reject(
          new Error(`GitHub API request failed with status ${statusCode}`),
        );
        return;
      }

      const chunks: Buffer[] = [];
      response.on("data", (chunk) => {
        chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
      });
      response.on("end", () => resolve(Buffer.concat(chunks)));
      response.on("error", reject);
    });

    request.on("error", reject);
  });
}

async function downloadFile(
  url: string,
  destinationPath: string,
): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    const request = https.get(
      url,
      {
        headers: {
          Accept: "application/octet-stream",
          "User-Agent": "metal-analyzer-vscode-extension",
        },
      },
      (response) => {
        const statusCode = response.statusCode ?? 0;
        if (
          [301, 302, 303, 307, 308].includes(statusCode) &&
          typeof response.headers.location === "string"
        ) {
          const redirectedUrl = new URL(
            response.headers.location,
            url,
          ).toString();
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

        const fileStream = createWriteStream(destinationPath);
        pipeline(response, fileStream).then(resolve).catch(reject);
      },
    );

    request.on("error", reject);
  });
}

async function extractArchive(
  archivePath: string,
  outputDirectory: string,
): Promise<void> {
  await execFileAsync("tar", ["-xzf", archivePath, "-C", outputDirectory]);
}

async function isExecutable(filePath: string): Promise<boolean> {
  try {
    await fs.access(filePath, fsConstants.X_OK);
    return true;
  } catch {
    return false;
  }
}

async function findNewestInstalledBinary(
  installRoot: string,
): Promise<string | undefined> {
  let entries: Array<{ path: string; modifiedAtMs: number }> = [];
  try {
    const directoryEntries = await fs.readdir(installRoot, {
      withFileTypes: true,
    });
    const candidateStats = await Promise.all(
      directoryEntries
        .filter(
          (entry) =>
            entry.isDirectory() && entry.name.startsWith(`${SERVER_NAME}-`),
        )
        .map(async (entry) => {
          const entryPath = path.join(installRoot, entry.name);
          const stat = await fs.stat(entryPath);
          return { path: entryPath, modifiedAtMs: stat.mtimeMs };
        }),
    );
    entries = candidateStats;
  } catch {
    return undefined;
  }

  const sortedEntries = entries.sort(
    (left, right) => right.modifiedAtMs - left.modifiedAtMs,
  );

  for (const entry of sortedEntries) {
    const candidateBinaryPath = path.join(entry.path, SERVER_NAME);
    if (await isExecutable(candidateBinaryPath)) {
      return candidateBinaryPath;
    }
  }

  return undefined;
}

async function removeOutdatedInstalledVersions(
  installRoot: string,
  currentVersionDirName: string,
): Promise<void> {
  const entries = await fs.readdir(installRoot, { withFileTypes: true });
  await Promise.all(
    entries
      .filter(
        (entry) =>
          entry.isDirectory() &&
          entry.name.startsWith(`${SERVER_NAME}-`) &&
          entry.name !== currentVersionDirName,
      )
      .map((entry) =>
        fs.rm(path.join(installRoot, entry.name), {
          recursive: true,
          force: true,
        }),
      ),
  );
}
