const { readStartupColorMode, startupTheme } = require("./startup-theme.cjs");
const fs = require("node:fs");
const path = require("node:path");
const readline = require("node:readline");
const crypto = require("node:crypto");
const { spawn, exec } = require("node:child_process");

const applyUpdateArgIndex = process.argv.indexOf("--_tron-apply-update");
if (applyUpdateArgIndex >= 0) {
  const { runApplyUpdate, UPDATE_HELPER_READY } = require("./updater.cjs");
  const installerPath = process.argv[applyUpdateArgIndex + 1] || "";
  void runApplyUpdate(installerPath, {
    logPath: process.argv[applyUpdateArgIndex + 2],
    installDir: process.argv[applyUpdateArgIndex + 3],
    parentPid: process.argv[applyUpdateArgIndex + 4] ? Number(process.argv[applyUpdateArgIndex + 4]) : undefined,
    onReady: () => process.stdout.write(`${UPDATE_HELPER_READY}\n`)
  }).then((code) => process.exit(code));
} else {
const { app, BrowserWindow, WebContentsView, clipboard, dialog, ipcMain, Menu, nativeTheme, net, Notification, shell, Tray } = require("electron");
const { startupPalette } = require("./startup-page.cjs");
const { STARTUP_ARG, ICON_PATH } = require("./desktop-constants.cjs");
const { createWindowsDesktop } = require("./windows-desktop.cjs");
const { createLinuxDesktop } = require("./linux-desktop.cjs");
const { resolveDesktopDataDir } = require("./helpers.cjs");
const desktopDataDir = resolveDesktopDataDir({ isPackaged: app.isPackaged });

// Windows toast notifications require a stable AppUserModelID matching the
// installer's appId; without it notifications are silently dropped.
if (process.platform === "win32") {
  app.setAppUserModelId("com.tron.desktop");
}

// Chromium must receive these paths before readiness and the single-instance lock.
if (process.platform === "win32" && app.isPackaged) {
  for (const [name, directory] of Object.entries({
    userData: path.join(desktopDataDir, "electron"),
    sessionData: path.join(desktopDataDir, "electron"),
    logs: path.join(desktopDataDir, "logs"),
    crashDumps: path.join(desktopDataDir, "crash-dumps")
  })) {
    fs.mkdirSync(directory, { recursive: true });
    app.setPath(name, directory);
  }
}

if (["win32", "linux"].includes(process.platform) && !app.requestSingleInstanceLock()) {
  app.quit();
} else {
const { createApplicationLogger } = require("./application-log.cjs");
const { createUpdater, UPDATE_STATUS_EVENT } = require("./updater.cjs");

const {
  HOST,
  resolveBackendHost,
  checkTargetPageEmbeddingStatus,
  findFreePort,
  isHttpUrl,
  loadPort,
  resolveBackendConfigPath,
  resolveBackendLogPath,
  resolveControlDatabasePath,
  resolveFrontendDistDir,
  resolveResourcesDir,
  resolveSharedDatabasePath,
  resolveSharedGraphStoragePath,
  resolveSidecarPath,
  resolveUpdateServerUrl,
  sanitizeUrl,
  waitBackendReady
} = require("./helpers.cjs");
const {
  createWebviewCommentPayload,
  dispatchWebviewComment,
  buildWebviewContextMenuTemplate
} = require("./webview-context-menu.cjs");
const { handleFsCommand } = require("./fs-gateway.cjs");

const MAIN_WINDOW_LABEL = "tron-main";
const MIN_STARTUP_SCREEN_MS = 1200;
const ALLOWED_INJECTION_EVENTS = new Set([
  "tron:frame-ready",
  "tron:dom-event",
  "tron:inspection-started",
  "tron:node-inspected",
  "tron:inspection-exited",
  "tron:navigation-requested",
  "tron:navigation-changed"
]);
const commentCapablePageIdsByWebContentsId = new Map();
let isQuitting = false;
let mainWindowRequested = false;
let residentDesktop = null;
let startupScene = null;

function showMainWindow() {
  mainWindowRequested = true;
  if (isQuitting) return;
  let window = state.mainWindow;
  if (!window || window.isDestroyed()) {
    // Bootstrap owns initial window creation and sidecar startup.
    if (!state.backendReady) return;
    window = createMainWindow();
    void window.loadURL(state.backendUrl);
  }
  if (window.isMinimized()) window.restore();
  window.show();
  window.focus();
}

function quitDesktop() {
  isQuitting = true;
  app.quit();
}

function setCommentAvailability(webContentsId, pageId, available) {
  if (available) {
    commentCapablePageIdsByWebContentsId.set(webContentsId, pageId);
  } else {
    commentCapablePageIdsByWebContentsId.delete(webContentsId);
  }
}

const injectionTemplate = fs.readFileSync(
  path.resolve(__dirname, "..", "injection", "tron-desktop-injection.js"),
  "utf8"
);

const state = {
  backendPort: 0,
  backendHost: HOST,
  clientHost: HOST,
  backendUrl: "",
  backendProcess: null,
  backendReady: false,
  controlDatabasePath: "",
  databasePath: "",
  backendLogPath: "",
  appDataDir: "",
  frontendDistDir: "",
  graphPath: "",
  profileDataDir: "",
  profileId: "",
  injectionEnabled: false,
  mainWindow: null,
  updater: null,
  environment: { mode: "local" },
  wsl: null,
  switchingEnvironment: false
};

function sendUpdaterStatus(status) {
  if (!state.mainWindow || state.mainWindow.isDestroyed()) return;
  state.mainWindow.webContents.send(UPDATE_STATUS_EVENT, status);
}

function requireUpdater() {
  if (!state.updater) throw new Error("desktop updater is not initialized");
  return state.updater;
}

function buildWebviewInjectionScript(pageId, enabled) {
  return injectionTemplate
    .replace("__TRON_APP_ORIGIN__", JSON.stringify(state.backendUrl))
    .replace("__TRON_PAGE_ID__", JSON.stringify(pageId))
    .replace("__TRON_INJECTION_ENABLED__", enabled ? "true" : "false");
}

const detachedBrowserWindows = new Set();

function configureBrowserTabHost(window) {
  const targetPreloadPath = path.resolve(__dirname, "..", "preload", "target-preload.cjs");
  window.webContents.on("will-attach-webview", (_event, webPreferences) => {
    webPreferences.contextIsolation = true;
    webPreferences.nodeIntegration = false;
    webPreferences.preload = targetPreloadPath;
    webPreferences.sandbox = false;
    webPreferences.webSecurity = true;
  });

  window.webContents.on("did-attach-webview", (_event, guestContents) => {
    guestContents.setWindowOpenHandler(({ url }) => {
      const targetUrl = sanitizeUrl(url);
      if (isHttpUrl(targetUrl)) {
        void guestContents.loadURL(targetUrl);
      }
      return { action: "deny" };
    });

    guestContents.on("context-menu", (event, params) => {
      event.preventDefault();
      if (guestContents.isDestroyed()) {
        return;
      }

      const template = buildWebviewContextMenuTemplate(guestContents, params, {
        addComment: (x, y) => {
          const pageId = commentCapablePageIdsByWebContentsId.get(guestContents.id);
          if (!pageId) {
            return;
          }

          void dispatchWebviewComment(
            guestContents.mainFrame,
            createWebviewCommentPayload(pageId, x, y)
          ).then((dispatched) => {
            if (!dispatched) {
              console.error("failed to dispatch webview comment command");
            }
          });
        },
        allowInspect: isDevMode(),
        canComment: commentCapablePageIdsByWebContentsId.has(guestContents.id),
        copyText: (value) => clipboard.writeText(value),
        isHttpUrl,
        locale: app.getLocale(),
        openExternal: openUrlInSystemBrowser
      });
      Menu.buildFromTemplate(template).popup({
        frame: params.frame || undefined,
        sourceType: params.menuSourceType,
        window
      });
    });

    guestContents.once("destroyed", () => {
      commentCapablePageIdsByWebContentsId.delete(guestContents.id);
    });
  });

}

function createMainWindow() {
  const palette = startupPalette(startupTheme(readStartupColorMode(app), nativeTheme));
  const window = new BrowserWindow({
    height: 920,
    show: false,
    backgroundColor: palette.background,
    title: "Tron",
    width: 1440,
    // macOS controls appear on hover without reserving space beside the brand.
    titleBarStyle: process.platform === "darwin" ? "customButtonsOnHover" : "hidden",
    ...(process.platform !== "darwin"
      ? { titleBarOverlay: { height: 40, color: "#00000000", symbolColor: palette.foreground } }
      : {}),
    ...(["win32", "linux"].includes(process.platform) ? { icon: ICON_PATH } : {}),
    webPreferences: {
      webviewTag: true,
      contextIsolation: true,
      nodeIntegration: false,
      preload: path.resolve(__dirname, "..", "preload", "app-preload.cjs"),
      sandbox: false,
      ...(state.environment.mode === "wsl" ? {
        partition: `persist:tron-wsl-${crypto.createHash("sha256").update(state.environment.distribution).digest("hex").slice(0, 16)}`
      } : {})
    }
  });

  state.mainWindow = window;
  const sendWindowChrome = () => {
    window.webContents.send("tron-desktop:window-chrome", { fullscreen: window.isFullScreen() });
  };
  window.webContents.on("dom-ready", sendWindowChrome);
  window.on("enter-full-screen", sendWindowChrome);
  window.on("leave-full-screen", sendWindowChrome);
  const onBeforeInput = (event, input) => {
    if (process.platform === "darwin" || input.type !== "keyDown" || input.isAutoRepeat
      || input.alt || input.meta || input.control || input.shift || input.key !== "F11") return;
    event.preventDefault();
    window.setFullScreen(!window.isFullScreen());
  };
  window.webContents.on("before-input-event", onBeforeInput);
  startupScene = require("./startup-scene.cjs").createStartupScene({ app, window, WebContentsView, ipcMain, nativeTheme, onBeforeInput });

  window.on("close", (event) => {
    if (isQuitting || (process.platform !== "darwin" && !residentDesktop?.isResident())) {
      return;
    }

    event.preventDefault();
    window.hide();
  });

  // Windows shutdown/logoff must terminate the sidecar instead of hiding.
  window.on("session-end", () => {
    isQuitting = true;
    stopBackend();
  });

  configureBrowserTabHost(window);

  window.webContents.on("did-create-window", (child) => {
    detachedBrowserWindows.add(child);
    configureBrowserTabHost(child);
    // The child hosts a React portal from the main renderer. It never bootstraps
    // the application, receives privileged IPC, or navigates its top-level frame.
    child.webContents.on("will-navigate", (event) => event.preventDefault());
    child.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
    child.on("closed", () => detachedBrowserWindows.delete(child));
  });
  window.webContents.setWindowOpenHandler(({ url, frameName }) => {
    if (url === "about:blank" && /^tron-detached-[0-9a-f-]{36}$/.test(frameName)) {
      return {
        action: "allow",
        overrideBrowserWindowOptions: {
          title: "Tron", minWidth: 400, minHeight: 320,
          webPreferences: { webviewTag: true, contextIsolation: true, nodeIntegration: false,
            sandbox: false, webSecurity: true, preload: "" }
        }
      };
    }
    const targetUrl = sanitizeUrl(url);
    if (!isHttpUrl(targetUrl)) {
      return { action: "deny" };
    }
    openUrlInSystemBrowser(targetUrl);
    return { action: "deny" };
  });

  window.on("closed", () => {
    for (const child of detachedBrowserWindows) child.destroy();
    detachedBrowserWindows.clear();
    if (state.mainWindow === window) {
      state.mainWindow = null;
    }
  });

  return window;
}

async function showStartupMessage(message, { failed = false } = {}) {
  if (!state.mainWindow) {
    return;
  }
  const html = startupPageHtml(message, { failed });
  await startupScene.show(html);
}

async function keepStartupScreenVisible(startedAt) {
  const remainingMs = MIN_STARTUP_SCREEN_MS - (Date.now() - startedAt);
  if (remainingMs > 0) {
    await new Promise((resolve) => setTimeout(resolve, remainingMs));
  }
}

function startupPageHtml(message, { failed = false } = {}) {
  return require("./startup-page.cjs").startupPageHtml({
    message,
    dataDir: state.appDataDir,
    logPath: state.backendLogPath,
    locale: app.getLocale(),
    theme: startupTheme(readStartupColorMode(app), nativeTheme),
    failed
  });
}

function startBackend(resourcesDir) {
  const sidecarPath = resolveSidecarPath(resourcesDir);
  const appDataDir = state.appDataDir;
  const configPath = resolveBackendConfigPath(appDataDir, resourcesDir, {
    relocateStorage: process.platform === "win32" && app.isPackaged,
    backendInitialize: process.platform !== "win32"
  });
  const logPath = resolveBackendLogPath(appDataDir);
  fs.mkdirSync(path.dirname(logPath), { recursive: true });
  const logStream = fs.createWriteStream(logPath, { flags: "a" });
  logStream.on("error", (error) => {
    console.error(`tron-backend log stream failed: ${describeError(error)}`);
  });
  const writeLog = (source, message) => {
    const line = `[${new Date().toISOString()}] [${source}] ${String(message)}`;
    if (!logStream.destroyed && !logStream.writableEnded) {
      logStream.write(`${line}\n`);
    }
  };

  writeLog("lifecycle", `using application config: ${configPath}`);
  writeLog("lifecycle", `starting sidecar: ${sidecarPath}`);
  const child = spawn(sidecarPath, [], {
    env: {
      ...process.env,
      ...(process.platform === "darwin" ? { MallocLargeCache: process.env.MallocLargeCache ?? "0" } : {}),
      TRON_BACKEND_DATABASE_URL: state.databasePath,
      TRON_BACKEND_CONTROL_DATABASE_URL: state.controlDatabasePath,
      TRON_BACKEND_APPLICATION_CONFIG_PATH: configPath,
      TRON_BACKEND_FRONTEND_DIST_DIR: state.frontendDistDir,
      TRON_BACKEND_GRAPH_STORAGE_PATH: state.graphPath,
      TRON_BACKEND_PORT: String(state.backendPort),
      TRON_BACKEND_BIND_ADDRESS: state.backendHost,
      TRON_BACKEND_LOG_STDERR: "false"
    },
    cwd: appDataDir,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });

  const stdout = readline.createInterface({ input: child.stdout });
  const stderr = readline.createInterface({ input: child.stderr });
  stdout.on("line", (line) => writeLog("stdout", line));
  stderr.on("line", (line) => writeLog("stderr", line));
  child.once("error", (error) => writeLog("lifecycle", `process error: ${describeError(error)}`));
  child.once("close", (code, signal) => {
    writeLog("lifecycle", `exited with code=${code ?? "null"} signal=${signal ?? "null"}`);
    stdout.close();
    stderr.close();
    logStream.end();
  });

  state.backendProcess = child;
  state.backendLogPath = logPath;
  child.once("exit", () => {
    if (state.backendProcess === child) {
      state.backendProcess = null;
    }
  });
}

function stopBackend() {
  state.wsl?.stop();
  if (!state.backendProcess) {
    return;
  }
  const child = state.backendProcess;
  state.backendProcess = null;
  child.kill();
}

function isDevMode() {
  return !app.isPackaged || process.env.TRON_ELECTRON_DEVTOOLS === "1";
}

function shouldOpenMainDevTools() {
  return process.env.TRON_ELECTRON_DEVTOOLS === "1";
}

async function bootstrapRuntime() {
  const resourcesDir = resolveResourcesDir(app);
  const appDataDir = desktopDataDir;
  state.appDataDir = appDataDir;
  fs.mkdirSync(appDataDir, { recursive: true });

  state.backendHost = resolveBackendHost({ isPackaged: app.isPackaged });
  state.clientHost = HOST;
  state.backendPort = await findFreePort(loadPort(), state.backendHost);
  state.backendUrl = `http://${state.clientHost}:${state.backendPort}`;
  state.profileId = "";
  state.profileDataDir = appDataDir;
  fs.mkdirSync(state.profileDataDir, { recursive: true });
  state.controlDatabasePath = resolveControlDatabasePath(appDataDir);
  state.databasePath = resolveSharedDatabasePath(appDataDir);
  state.backendLogPath = resolveBackendLogPath(appDataDir);
  state.graphPath = resolveSharedGraphStoragePath(appDataDir);
  state.frontendDistDir = resolveFrontendDistDir(resourcesDir);
  if (process.platform !== "darwin") Menu.setApplicationMenu(null);
  if (process.platform === "win32") {
    const { WslBackend } = require("./wsl-backend.cjs");
    const { fetchWslDownload } = require("./wsl-download.cjs");
    state.wsl = new WslBackend({
      dataDir: appDataDir,
      getVersion: () => app.getVersion(),
      getVersionServerUrl: () => resolveUpdateServerUrl(resourcesDir),
      frontendDir: state.frontendDistDir,
      sidecarPath: resolveSidecarPath(resourcesDir),
      fetchImpl: (url, options) => net.fetch(url, options),
      fetchDownloadImpl: (url, options) => fetchWslDownload(net, url, options),
      onStatus: (status) => {
        state.mainWindow?.webContents.send("backend-environment:status", status);
        if (status.backendExited && state.backendReady && state.environment.mode === "wsl") {
          state.backendReady = false;
          void handleBackendFailure(new Error(status.message)).catch(console.error);
        }
      }
    });
    try { state.environment = state.wsl.selection(); }
    catch (error) { await handleBackendFailure(error); return; }
  }
  state.updater = createUpdater({
    dataDir: appDataDir,
    getVersion: () => app.getVersion(),
    getVersionServerUrl: () => resolveUpdateServerUrl(resourcesDir),
    onStatus: sendUpdaterStatus,
    onLog: createApplicationLogger(path.join(appDataDir, "logs")),
    openInstaller: (filePath) => {
      if (process.platform === "linux") { shell.showItemInFolder(filePath); return ""; }
      return shell.openPath(filePath);
    },
    exit: (code) => {
      isQuitting = true;
      stopBackend();
      residentDesktop?.dispose();
      app.exit(code);
    }
  });

  if (process.platform === "win32") {
    residentDesktop = createWindowsDesktop({ app, Tray, Menu, showWindow: showMainWindow, quit: quitDesktop });
  } else if (process.platform === "linux") {
    residentDesktop = createLinuxDesktop({ app, Tray, Menu, showWindow: showMainWindow, quit: quitDesktop, dataDir: appDataDir });
  }
  residentDesktop?.initialize();

  const mainWindow = createMainWindow();
  await showStartupMessage(app.getLocale().startsWith("zh") ? "正在启动本地服务…" : "Starting local services…");
  if (!residentDesktop?.isResident() || !app.isPackaged || !process.argv.includes(STARTUP_ARG) || mainWindowRequested) {
    mainWindow.show();
  }
  const startupScreenStartedAt = Date.now();

  try {
    if (state.environment.mode === "wsl") {
      state.backendLogPath = `${state.environment.distribution}:~/.tron/logs/tron-backend.log`;
      await showStartupMessage(app.getLocale().startsWith("zh") ? `正在连接 WSL：${state.environment.distribution}` : `Connecting to WSL: ${state.environment.distribution}`);
      const session = await state.wsl.start(state.environment.distribution, state.backendPort);
      state.backendPort = session.port;
      state.backendUrl = session.backendUrl;
      state.appDataDir = session.dataDir;
      state.profileDataDir = session.dataDir;
      state.databasePath = `${session.dataDir}/tron.sqlite3`;
      state.controlDatabasePath = `${session.dataDir}/control/profiles.sqlite`;
      state.graphPath = `${session.dataDir}/tron.graph.grafeo`;
      state.backendLogPath = `${session.dataDir}/logs/tron-backend.log`;
      state.frontendDistDir = `${session.releaseDir}/frontend`;
    } else {
      startBackend(resourcesDir);
    }
  } catch (error) {
    await handleBackendFailure(error);
    return;
  }

  state.backendReady = await waitBackendReady(state.backendUrl);
  if (!state.backendReady) {
    stopBackend();
    await handleBackendFailure(new Error("Backend readiness timeout"));
    return;
  }

  await keepStartupScreenVisible(startupScreenStartedAt);
  await mainWindow.loadURL(state.backendUrl);
  startupScene.dismissIfNotReadyAfter();
  if (shouldOpenMainDevTools()) {
    mainWindow.webContents.openDevTools({ mode: "detach" });
  }
  if (mainWindow.isVisible()) mainWindow.focus();
}

async function bootstrap() {
  try { await bootstrapRuntime(); }
  catch (error) { await handleBackendFailure(error); }
}

async function selectBackendEnvironment(selection) {
  if (!state.wsl) throw new Error("wsl-windows-only");
  if (state.switchingEnvironment) throw new Error("wsl-operation-in-progress");
  if (state.wsl.busy) throw new Error("wsl-operation-in-progress");
  if (!["local", "wsl"].includes(selection.mode)) throw new Error("invalid-backend-environment");
  state.switchingEnvironment = true;
  try {
    if (selection.mode === "wsl") await state.wsl.prepare(selection.distribution);
    state.wsl.saveSelection(selection);
    // A new renderer partition clears in-memory tabs, queries, credentials and
    // file-root grants together. Each environment keeps its own persisted UI.
    app.relaunch();
    setTimeout(quitDesktop, 150);
    return true;
  } catch (error) { state.switchingEnvironment = false; throw error; }
}

async function chooseBackendEnvironment() {
  const zh = app.getLocale().startsWith("zh");
  let distributions = [], detail = "";
  try { distributions = (await state.wsl.list()).filter((item) => item.wslVersion === 2); }
  catch (error) { detail = describeError(error); }
  const result = await dialog.showMessageBox({
    type: "info", title: "Tron",
    message: zh ? "选择后端环境" : "Choose backend environment",
    detail: detail || (zh ? "WSL 模式会下载 Linux 后端，安装运行依赖，并将数据保存在所选发行版内。支持 Ubuntu 24.04+、Debian 13+。切换会重启 Tron，正在运行的任务会停止，已有数据保留在原环境。" : "WSL mode downloads the Linux backend, installs runtime dependencies and stores data in the selected distribution. Requires Ubuntu 24.04+ or Debian 13+. Switching restarts Tron and stops active tasks. Existing data stays in its original environment."),
    buttons: [zh ? "本机" : "Local", ...distributions.map((item) => `WSL: ${item.name}`), zh ? "取消" : "Cancel"],
    cancelId: distributions.length + 1
  });
  if (result.response === distributions.length + 1) return false;
  await selectBackendEnvironment(result.response === 0 ? { mode: "local" } : { mode: "wsl", distribution: distributions[result.response - 1].name });
  return true;
}

async function handleBackendFailure(error) {
  state.backendReady = false;
  stopBackend();
  await showStartupMessage(`Backend failed to start. ${describeError(error)}\nLogs: ${state.backendLogPath}`, { failed: true });
  if (!state.wsl) return;
  const zh = app.getLocale().startsWith("zh");
  const result = await dialog.showMessageBox({
    type: "error", title: "Tron",
    message: zh ? "无法连接后端" : "Could not connect to backend",
    detail: `${describeError(error)}\n${state.backendLogPath}`,
    buttons: [zh ? "选择后端环境" : "Choose environment", zh ? "重试" : "Retry", zh ? "退出" : "Quit"], cancelId: 2
  });
  if (result.response === 0) {
    try { if (!await chooseBackendEnvironment()) await handleBackendFailure(error); }
    catch (reason) { await handleBackendFailure(reason); }
  } else if (result.response === 1) { app.relaunch(); quitDesktop(); }
  else quitDesktop();
}

function describeError(error) {
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return String(error || "unknown error");
}

function isWSL() {
  try {
    return /microsoft/i.test(fs.readFileSync("/proc/version", "utf8"));
  } catch {
    return false;
  }
}

// ShellExecuteEx 的 P/Invoke 声明，供 WSL 场景经 PowerShell 调用
const TRON_SHELL_OPEN_CSHARP = `using System;
using System.Runtime.InteropServices;
public static class TronShellOpen
{
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  private struct SHELLEXECUTEINFO
  {
    public int cbSize;
    public uint fMask;
    public IntPtr hwnd;
    [MarshalAs(UnmanagedType.LPWStr)] public string lpVerb;
    [MarshalAs(UnmanagedType.LPWStr)] public string lpFile;
    [MarshalAs(UnmanagedType.LPWStr)] public string lpParameters;
    [MarshalAs(UnmanagedType.LPWStr)] public string lpDirectory;
    public int nShow;
    public IntPtr hInstApp;
    public IntPtr lpIDList;
    [MarshalAs(UnmanagedType.LPWStr)] public string lpClass;
    public IntPtr hkeyClass;
    public uint dwHotKey;
    public IntPtr hIcon;
    public IntPtr hProcess;
  }

  [DllImport("shell32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  private static extern bool ShellExecuteEx(ref SHELLEXECUTEINFO info);

  public static void Open(string target)
  {
    SHELLEXECUTEINFO info = new SHELLEXECUTEINFO();
    info.cbSize = Marshal.SizeOf(typeof(SHELLEXECUTEINFO));
    info.fMask = 0x04000000u; // SEE_MASK_FLAG_LOG_USAGE
    info.lpFile = target;
    info.nShow = 1; // SW_SHOWNORMAL
    if (!ShellExecuteEx(ref info))
    {
      throw new Exception("ShellExecuteEx failed: " + Marshal.GetLastWin32Error());
    }
  }
}`;

function buildWslPowerShellOpenCommand(url) {
  const safeUrl = url.replace(/'/g, "''");
  const script = `$csharp = @'
${TRON_SHELL_OPEN_CSHARP}
'@
Add-Type -TypeDefinition $csharp
[TronShellOpen]::Open('${safeUrl}')`;
  const encoded = Buffer.from(script, "utf16le").toString("base64");
  return `powershell.exe -NoProfile -NonInteractive -EncodedCommand ${encoded}`;
}

function releaseMainWindowFocus() {
  const window = state.mainWindow;
  if (!window || window.isDestroyed() || !window.isFocused()) {
    return;
  }
  window.blur();
}

function openUrlInSystemBrowser(url) {
  const escapedUrl = url.replace(/"/g, "");
  if (isWSL()) {
    releaseMainWindowFocus();
    exec(buildWslPowerShellOpenCommand(url), { windowsHide: true }, (error) => {
      if (!error) {
        return;
      }
      exec(`cmd.exe /c start "" "${escapedUrl}"`, { windowsHide: true }, (cmdError) => {
        if (cmdError) {
          exec(`wslview "${escapedUrl}"`, { windowsHide: true }, (wslviewError) => {
            if (wslviewError) {
              shell.openExternal(url).catch(() => {});
            }
          });
        }
      });
    });
    return;
  }

  if (process.platform === "win32") {
    releaseMainWindowFocus();
    shell.openExternal(url, { logUsage: true }).catch(() => {
      exec(`start "" "${escapedUrl}"`, { windowsHide: true }, () => {});
    });
    return;
  }

  // macOS / 原生 Linux
  shell.openExternal(url).catch(() => {});
}

function normalizeRequest(payload) {
  if (payload && typeof payload === "object" && payload.request && typeof payload.request === "object") {
    return payload.request;
  }
  return payload && typeof payload === "object" ? payload : {};
}

const MAX_NOTIFICATION_TEXT_LENGTH = 2000;
const activeSystemNotifications = new Set();

function sanitizeNotificationText(value) {
  return typeof value === "string" ? value.trim().slice(0, MAX_NOTIFICATION_TEXT_LENGTH) : "";
}

function openSystemNotificationSettings() {
  if (process.platform === "darwin") {
    // The legacy pane identifier still selects the app-specific notification page on macOS 15+.
    shell.openExternal("x-apple.systempreferences:com.apple.preference.notifications?id=com.tron.desktop").catch(() => {});
    return true;
  }
  if (process.platform === "win32") {
    shell.openExternal("ms-settings:notifications").catch(() => {});
    return true;
  }
  return false;
}

function showSystemNotification(request) {
  if (!Notification.isSupported()) {
    return Promise.resolve({ shown: false, error: "notifications-unsupported" });
  }

  const title = sanitizeNotificationText(request.title);
  if (!title) {
    return Promise.resolve({ shown: false, error: "missing-notification-title" });
  }

  const id = sanitizeNotificationText(request.id) || `tron-notification-${crypto.randomUUID()}`;
  const notification = new Notification({
    id,
    title,
    body: sanitizeNotificationText(request.body),
    silent: request.silent === true
  });
  activeSystemNotifications.add(notification);
  while (activeSystemNotifications.size > 32) {
    activeSystemNotifications.delete(activeSystemNotifications.values().next().value);
  }

  return new Promise((resolve) => {
    let settled = false;
    let timeout;
    const finish = (result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      resolve(result);
    };

    notification.once("show", () => finish({ shown: true }));
    notification.once("failed", (_event, error) => {
      activeSystemNotifications.delete(notification);
      finish({ shown: false, error: sanitizeNotificationText(String(error || "notification-failed")) });
    });
    notification.once("close", () => activeSystemNotifications.delete(notification));
    notification.on("click", () => {
      const window = state.mainWindow;
      if (!window || window.isDestroyed()) return;
      if (window.isMinimized()) window.restore();
      window.show();
      window.focus();
      if (!window.webContents.isDestroyed()) {
        window.webContents.send("tron-desktop:notification-activated", { id });
      }
    });

    // Some Linux notification services do not report a show event; the request is
    // still accepted, so treat the absence of a failure as a successful send.
    timeout = setTimeout(() => finish({ shown: true }), 1500);
    notification.show();
  });
}

function nextPageId() {
  return `target-${Date.now()}`;
}

function sanitizeInjectionEvent(raw) {
  if (!raw || typeof raw !== "object") {
    throw new Error("invalid event payload format");
  }
  const eventType = typeof raw.eventType === "string" ? raw.eventType : "";
  if (!ALLOWED_INJECTION_EVENTS.has(eventType)) {
    throw new Error("event type is not allowed");
  }
  if (typeof raw.pageId !== "string" || !raw.pageId) {
    throw new Error("event payload missing pageId");
  }
  if (typeof raw.frameId !== "string" || !raw.frameId) {
    throw new Error("event payload missing frameId");
  }
  return raw;
}

function readAnnotationRect(payload) {
  const rect = payload.rect && typeof payload.rect === "object" ? payload.rect : null;
  if (!rect) {
    return null;
  }

  return {
    height: Math.max(1, Math.round(Number(rect.height) || 1)),
    width: Math.max(1, Math.round(Number(rect.width) || 1)),
    x: Math.max(0, Math.round(Number(rect.x) || 0)),
    y: Math.max(0, Math.round(Number(rect.y) || 0))
  };
}

function buildAnnotatedScreenshotDataUrl(screenshot, payload) {
  if (!screenshot || typeof screenshot.getSize !== "function" || typeof screenshot.toDataURL !== "function") {
    return null;
  }

  const sourceSize = screenshot.getSize();
  const maxPreviewEdge = 1800;
  const sourceWidth = Math.max(1, sourceSize.width);
  const sourceHeight = Math.max(1, sourceSize.height);
  const scale = Math.min(1, maxPreviewEdge / Math.max(sourceWidth, sourceHeight));
  const image = scale < 1
    ? screenshot.resize({
      height: Math.max(1, Math.round(sourceHeight * scale)),
      quality: "best",
      width: Math.max(1, Math.round(sourceWidth * scale))
    })
    : screenshot;
  const size = image.getSize();
  const width = Math.max(1, size.width);
  const height = Math.max(1, size.height);
  const markerNumber = Number(payload.markerNumber || 1);

  return {
    captureSource: "webcontents-view",
    dataUrl: image.toDataURL(),
    height,
    markerNumber,
    mimeType: "image/png",
    sourceHeight,
    sourceWidth,
    width
  };
}

function buildInstallAnnotationOverlayScript(payload) {
  const rect = readAnnotationRect(payload);
  if (!rect) {
    return null;
  }

  const options = {
    markerNumber: Number(payload.markerNumber || 1),
    rect
  };

  return `
(() => {
  const options = ${JSON.stringify(options)};
  const overlayId = "tron-native-annotation-overlay";
  document.getElementById(overlayId)?.remove();
  const root = document.createElement("div");
  root.id = overlayId;
  root.setAttribute("data-tron-inspector-ui", "true");
  Object.assign(root.style, {
    position: "fixed",
    inset: "0",
    pointerEvents: "none",
    zIndex: "2147483647",
    contain: "layout style paint"
  });

  const rect = options.rect;
  const box = document.createElement("div");
  Object.assign(box.style, {
    position: "fixed",
    left: Math.round(rect.x) + "px",
    top: Math.round(rect.y) + "px",
    width: Math.max(1, Math.round(rect.width)) + "px",
    height: Math.max(1, Math.round(rect.height)) + "px",
    background: "rgba(22, 119, 255, 0.12)",
    border: "3px solid #1677ff",
    boxSizing: "border-box"
  });

  const markerRadius = 18;
  const markerX = Math.max(markerRadius, Math.min(window.innerWidth - markerRadius, rect.x + markerRadius + 4));
  const markerY = Math.max(markerRadius, Math.min(window.innerHeight - markerRadius, rect.y - markerRadius - 8));
  const marker = document.createElement("div");
  marker.textContent = String(options.markerNumber || 1);
  Object.assign(marker.style, {
    position: "fixed",
    left: Math.round(markerX - markerRadius) + "px",
    top: Math.round(markerY - markerRadius) + "px",
    width: markerRadius * 2 + "px",
    height: markerRadius * 2 + "px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: "999px",
    background: "#1677ff",
    color: "#ffffff",
    font: "700 20px system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif",
    lineHeight: markerRadius * 2 + "px",
    boxSizing: "border-box",
    boxShadow: "0 8px 18px rgba(22, 119, 255, 0.32)"
  });

  root.append(box, marker);
  document.documentElement.appendChild(root);
  return true;
})()
`;
}

function removeAnnotationOverlayScript() {
  return `document.getElementById("tron-native-annotation-overlay")?.remove(); true;`;
}

async function waitForCaptureReadiness(sender) {
  await sender.executeJavaScript(`
new Promise((resolve) => {
  const fontsReady = document.fonts?.ready?.catch?.(() => undefined) || Promise.resolve();
  fontsReady.then(() => requestAnimationFrame(() => requestAnimationFrame(resolve)));
})
`, true);
}

async function withNativeAnnotationImage(event, sender) {
  if (event.eventType !== "tron:node-inspected") {
    return event;
  }
  const payload = event.payload && typeof event.payload === "object" ? { ...event.payload } : {};

  if (!sender || sender.isDestroyed()) {
    return { ...event, payload };
  }

  const installScript = buildInstallAnnotationOverlayScript(payload);
  if (installScript) {
    try {
      await sender.executeJavaScript(installScript, true);
      await waitForCaptureReadiness(sender);
    } catch (error) {
      console.error("native target annotation overlay failed", error);
    }
  }

  try {
    const image = await sender.capturePage();
    if (!image || typeof image.isEmpty !== "function" || image.isEmpty()) {
      return { ...event, payload };
    }
    const annotationImage = buildAnnotatedScreenshotDataUrl(image, payload);
    if (annotationImage) {
      payload.annotationImage = annotationImage;
    }
  } catch (error) {
    console.error("native target capture failed", error);
  } finally {
    try {
      await sender.executeJavaScript(removeAnnotationOverlayScript(), true);
    } catch (_error) {}
  }

  return { ...event, payload };
}

function forwardTargetEvent(event) {
  if (!state.mainWindow || state.mainWindow.isDestroyed()) {
    return;
  }
  state.mainWindow.webContents.send("desktop-target-event", event);
}

function artifactIdentifier(value, field) {
  if (typeof value !== "string" || !value.trim() || value.length > 512) {
    throw new Error(`invalid artifact ${field}`);
  }
  return value.trim();
}

function artifactFileName(value) {
  if (typeof value !== "string" || !value.trim()) return undefined;
  const fileName = value.split(/[\\/]/).pop()?.replace(/[\u0000-\u001f\u007f]/g, "").trim();
  if (!fileName || fileName === "." || fileName === "..") return undefined;
  const extension = path.extname(fileName);
  return fileName.length > 200 && extension
    ? `${fileName.slice(0, 200 - extension.length)}${extension}`
    : fileName.slice(0, 200);
}

function artifactFileNameFromDisposition(value) {
  if (typeof value !== "string") return undefined;
  const extended = /(?:^|;)\s*filename\*\s*=\s*([^;]+)/i.exec(value)?.[1]?.trim().replace(/^"|"$/g, "");
  if (extended) {
    const encoded = extended.replace(/^utf-8''/i, "");
    try {
      const fileName = artifactFileName(decodeURIComponent(encoded));
      if (fileName) return fileName;
    } catch (_error) {}
  }
  const plain = /(?:^|;)\s*filename\s*=\s*(?:"([^"]*)"|([^;]*))/i.exec(value);
  return artifactFileName(plain?.[1] ?? plain?.[2]);
}

function artifactSaveFilters(fileName) {
  const extension = path.extname(fileName).slice(1).toLowerCase();
  if (!/^[a-z0-9]{1,16}$/.test(extension)) return undefined;
  return [{ name: extension.toUpperCase(), extensions: [extension] }];
}

/**
 * Download one delivered file through the authenticated content endpoint and
 * persist it through the OS save dialog. The renderer supplies an access token
 * because the main process has no session of its own; the sender and origin
 * fence above still applies, and the backend re-authorizes every request.
 */
async function resolveArtifactPath(payload) {
  const sessionCode = artifactIdentifier(payload.sessionCode, "session");
  const runCode = artifactIdentifier(payload.runCode, "run");
  const itemId = artifactIdentifier(payload.itemId, "item");
  const workspaceCode = artifactIdentifier(payload.workspaceCode, "workspace");
  const token = artifactIdentifier(payload.token, "token");
  const url = new URL(
    `/api/v1/chats/sessions/${encodeURIComponent(sessionCode)}/artifacts/${encodeURIComponent(runCode)}/${encodeURIComponent(itemId)}/desktop-resolve`,
    state.backendUrl,
  );
  url.searchParams.set("workspaceCode", workspaceCode);
  const response = await net.fetch(url.toString(), {
    method: "POST",
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!response.ok) throw new Error(`artifact resolve unavailable (${response.status})`);
  const body = await response.json();
  const resolved = body && body.data && body.data.path;
  if (typeof resolved !== "string" || !path.isAbsolute(resolved)) {
    throw new Error("artifact resolve returned no local path");
  }
  return resolved;
}

async function handleArtifactOpen(command, payload) {
  try {
    const target = await resolveArtifactPath(payload);
    if (command === "artifact:reveal") {
      shell.showItemInFolder(target);
      return { status: "revealed" };
    }
    const failure = await shell.openPath(target);
    if (failure) return { status: "failed", message: failure };
    return { status: "opened" };
  } catch (error) {
    return {
      status: "failed",
      message: error instanceof Error ? error.message : "artifact operation failed",
    };
  }
}

async function handleArtifactSave(payload, browserWindow) {
  const sessionCode = artifactIdentifier(payload.sessionCode, "session");
  const runCode = artifactIdentifier(payload.runCode, "run");
  const itemId = artifactIdentifier(payload.itemId, "item");
  const workspaceCode = artifactIdentifier(payload.workspaceCode, "workspace");
  const token = artifactIdentifier(payload.token, "token");
  const url = new URL(
    `/api/v1/chats/sessions/${encodeURIComponent(sessionCode)}/artifacts/${encodeURIComponent(runCode)}/${encodeURIComponent(itemId)}/content`,
    state.backendUrl,
  );
  url.searchParams.set("workspaceCode", workspaceCode);
  url.searchParams.set("disposition", "attachment");
  const response = await net.fetch(url.toString(), {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!response.ok || !response.body) {
    return { status: "failed", message: `artifact content unavailable (${response.status})` };
  }
  const suggestedName = artifactFileNameFromDisposition(response.headers.get("content-disposition"))
    ?? artifactFileName(payload.suggestedName)
    ?? "artifact";
  const filters = artifactSaveFilters(suggestedName);
  const options = { defaultPath: suggestedName, ...(filters ? { filters } : {}) };
  const result = browserWindow
    ? await dialog.showSaveDialog(browserWindow, options)
    : await dialog.showSaveDialog(options);
  if (result.canceled || !result.filePath) {
    await response.body.cancel().catch(() => {});
    return { status: "cancelled" };
  }

  let backupPath;
  try {
    const candidate = path.join(path.dirname(result.filePath), `.tron-artifact-${crypto.randomUUID()}.backup`);
    try {
      await fs.promises.rename(result.filePath, candidate);
      backupPath = candidate;
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
    const { Readable } = require("node:stream");
    const { pipeline } = require("node:stream/promises");
    await pipeline(Readable.fromWeb(response.body), fs.createWriteStream(result.filePath, { flags: "wx" }));
    if (backupPath) await fs.promises.rm(backupPath, { force: true }).catch(() => {});
    return { status: "saved" };
  } catch (error) {
    await fs.promises.rm(result.filePath, { force: true }).catch(() => {});
    if (backupPath) await fs.promises.rename(backupPath, result.filePath).catch(() => {});
    throw error;
  }
}

function registerIpcHandlers() {
  ipcMain.handle("tron-desktop:invoke", async (_event, command, payload = {}) => {
    if (_event.sender !== state.mainWindow?.webContents || _event.senderFrame !== _event.sender.mainFrame
      || new URL(_event.senderFrame.url).origin !== new URL(state.backendUrl).origin) {
      throw new Error("untrusted-desktop-frame");
    }
    const request = normalizeRequest(payload);

    if (command === "artifact:save") {
      return handleArtifactSave(request, BrowserWindow.fromWebContents(_event.sender) || undefined);
    }
    if (command === "artifact:open" || command === "artifact:reveal") {
      return handleArtifactOpen(command, request);
    }

    if (typeof command === "string" && command.startsWith("fs:")) {
      const browserWindow = BrowserWindow.fromWebContents(_event.sender) || undefined;
      if (state.environment.mode === "wsl") return state.wsl.fileCommand(command, request, browserWindow);
      return handleFsCommand(command, request, browserWindow);
    }

    if (command === "getBackendEnvironment") {
      return { supported: process.platform === "win32", selection: state.environment, status: state.wsl?.status ?? { phase: "idle" } };
    }
    if (command === "listWslDistributions") {
      if (!state.wsl) throw new Error("wsl-windows-only");
      return state.wsl.listInstallations();
    }
    if (command === "uninstallWslBackend") {
      if (!state.wsl) throw new Error("wsl-windows-only");
      if (state.environment.mode !== "local") throw new Error("wsl-uninstall-local-only");
      if (state.switchingEnvironment) throw new Error("wsl-operation-in-progress");
      return state.wsl.uninstall(request.distribution, request.deleteUserData === true);
    }
    if (command === "selectBackendEnvironment") return selectBackendEnvironment(request);

    if (command === "desktopProcessMetrics") {
      return require("./process-metrics.cjs").collectProcessMetrics(app);
    }

    if (command === "desktopBackendStatus") {
      return {
        backendUrl: state.backendUrl,
        backendPort: state.backendPort,
        ready: state.backendReady,
        injectionEnabled: state.injectionEnabled,
        profileId: state.profileId,
        profileDataDir: state.profileDataDir,
        controlDatabasePath: state.controlDatabasePath,
        databasePath: state.databasePath,
        graphPath: state.graphPath,
        environment: state.environment
      };
    }

    if (command === "getAppVersion") {
      return app.getVersion();
    }

    if (command === "showSystemNotification") {
      return showSystemNotification(request);
    }

    if (command === "openSystemNotificationSettings") {
      return openSystemNotificationSettings();
    }

    if (command === "isReleaseNotesAcknowledged" || command === "acknowledgeReleaseNotes") {
      const version = app.getVersion();
      if (request.version !== version) throw new Error("release-notes-version-mismatch");
      const releaseNotes = require("./release-notes.cjs");
      if (command === "isReleaseNotesAcknowledged") {
        return releaseNotes.isAcknowledged(desktopDataDir, version);
      }
      releaseNotes.acknowledge(desktopDataDir, version);
      return true;
    }

    if (command === "checkUpdate") {
      return requireUpdater().check(request.accessToken);
    }

    if (command === "startDownloadUpdate") {
      const updater = requireUpdater();
      if (updater.getStatus().type !== "updateAvailable") {
        throw new Error("No downloadable update is available");
      }
      if (!String(request.accessToken || "").trim()) {
        throw new Error("An access token is required to download the update");
      }
      void updater.startDownload(request.accessToken).catch((error) => {
        console.warn(`update download failed: ${error instanceof Error ? error.message : String(error)}`);
      });
      return true;
    }

    if (command === "applyUpdate") {
      return requireUpdater().apply();
    }

    if (command === "getUpdateStatus") {
      return requireUpdater().getStatus();
    }

    if (command === "checkTargetPageEmbedding") {
      return checkTargetPageEmbeddingStatus(request.url);
    }

    if (command === "openTargetPage") {
      const pageId = String(request.pageId || nextPageId());
      const mode = String(request.mode || "iframe");
      const url = sanitizeUrl(request.url);
      if (!isHttpUrl(url)) {
        throw new Error("Only http/https URLs are allowed");
      }

      if (mode === "browser") {
        // fire-and-forget：立即返回，不等浏览器打开
        // 避免渲染进程后续处理抢回焦点导致浏览器窗口层级变低
        openUrlInSystemBrowser(url);
        return { pageId, opened: true, mode: "browser", backendUrl: state.backendUrl };
      }

      return {
        pageId,
        opened: false,
        mode,
        reason: "target-pages-use-webview",
        backendUrl: state.backendUrl
      };
    }

    if (command === "desktopInjectionEvent") {
      sanitizeInjectionEvent(request.event);
      return undefined;
    }

    throw new Error(`unknown desktop command: ${command}`);
  });

  ipcMain.handle("tron-desktop:target-event", async (ipcEvent, rawEvent) => {
    const sanitized = sanitizeInjectionEvent(rawEvent);
    if (process.env.TRON_ELECTRON_TARGET_DEBUG === "1" && sanitized.eventType !== "tron:dom-event") {
      console.log(`[target-debug] page event eventType=${sanitized.eventType} pageId=${sanitized.pageId}`);
    }
    const withImage = await withNativeAnnotationImage(sanitized, ipcEvent.sender);
    forwardTargetEvent(withImage);
    return true;
  });

  ipcMain.handle("tron-desktop:target-injection-script", (ipcEvent, payload = {}) => {
    if (!state.mainWindow || state.mainWindow.isDestroyed()) {
      return "";
    }

    const host = ipcEvent.sender.hostWebContents;
    if (!host || (host.id !== state.mainWindow.webContents.id
      && !Array.from(detachedBrowserWindows).some((child) => !child.isDestroyed() && child.webContents.id === host.id))) {
      return "";
    }

    const pageId = String(payload.pageId || "").trim().slice(0, 160);
    if (!pageId) {
      return "";
    }

    setCommentAvailability(ipcEvent.sender.id, pageId, payload.commentAvailable);

    return buildWebviewInjectionScript(pageId, Boolean(payload.enabled));
  });

  ipcMain.handle("tron-desktop:update-target-comment-availability", (ipcEvent, payload = {}) => {
    if (!state.mainWindow || state.mainWindow.isDestroyed()) {
      return false;
    }

    const host = ipcEvent.sender.hostWebContents;
    if (!host || (host.id !== state.mainWindow.webContents.id
      && !Array.from(detachedBrowserWindows).some((child) => !child.isDestroyed() && child.webContents.id === host.id))) {
      return false;
    }

    const pageId = String(payload.pageId || "").trim().slice(0, 160);
    if (!pageId) {
      return false;
    }

    setCommentAvailability(ipcEvent.sender.id, pageId, payload.commentAvailable);

    return true;
  });
}

registerIpcHandlers();

app.whenReady().then(() => {
  if (process.platform === "darwin") {
    app.setName("Tron");
    app.dock?.setIcon(ICON_PATH);
  }
  void bootstrap();
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin" && !residentDesktop?.isResident()) {
    app.quit();
  }
});

app.on("activate", () => {
  showMainWindow();
});

app.on("second-instance", (_event, argv) => {
  if (!argv.includes(STARTUP_ARG)) showMainWindow();
});

app.on("before-quit", () => {
  isQuitting = true;
  stopBackend();
  residentDesktop?.dispose();
});
}
}
