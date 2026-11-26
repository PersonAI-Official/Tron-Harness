const fs = require("node:fs");
const fsp = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");
const { spawn: defaultSpawn } = require("node:child_process");
const { finished } = require("node:stream/promises");
const { DEFAULT_PERSON_AI_SERVER_BASE_URL } = require("./helpers.cjs");

const PRODUCT_CODE = "tron";
const UPDATE_STATUS_EVENT = "update:status-changed";
const UPDATE_HELPER_READY = "TRON_UPDATE_HELPER_READY";

const OS_TYPES = Object.freeze({
  WINDOWS_X86: 110,
  WINDOWS_X64: 120,
  WINDOWS_ARM64: 130,
  MACOS_X64: 220,
  MACOS_ARM64: 230,
  LINUX_X86: 310,
  LINUX_X64: 320,
  LINUX_ARM64: 330,
  GENERIC: 0
});

const UPDATE_TYPES = new Set([
  "idle",
  "checking",
  "upToDate",
  "updateAvailable",
  "downloading",
  "downloaded",
  "installing",
  "error"
]);

function currentOsType(platformName = process.platform, archName = process.arch) {
  if (platformName === "win32") {
    if (archName === "x64") return OS_TYPES.WINDOWS_X64;
    if (archName === "arm64") return OS_TYPES.WINDOWS_ARM64;
    if (archName === "ia32") return OS_TYPES.WINDOWS_X86;
  }

  if (platformName === "darwin") {
    if (archName === "x64") return OS_TYPES.MACOS_X64;
    if (archName === "arm64") return OS_TYPES.MACOS_ARM64;
  }

  if (platformName === "linux") {
    if (archName === "x64") return OS_TYPES.LINUX_X64;
    if (archName === "arm64") return OS_TYPES.LINUX_ARM64;
    if (archName === "ia32") return OS_TYPES.LINUX_X86;
  }

  return OS_TYPES.GENERIC;
}

function versionSegments(value) {
  return String(value || "")
    .trim()
    .replace(/^[vV]/, "")
    .split(".")
    .map((segment) => {
      const digits = segment.match(/^\d+/)?.[0];
      return digits ? Number.parseInt(digits, 10) : 0;
    });
}

function compareVersions(left, right) {
  const leftSegments = versionSegments(left);
  const rightSegments = versionSegments(right);
  const length = Math.max(leftSegments.length, rightSegments.length);

  for (let index = 0; index < length; index += 1) {
    const leftValue = leftSegments[index] ?? 0;
    const rightValue = rightSegments[index] ?? 0;
    if (leftValue !== rightValue) {
      return leftValue > rightValue ? 1 : -1;
    }
  }

  return 0;
}

function normalizeVersion(value) {
  const version = String(value || "").trim().replace(/^[vV]/, "");
  if (!version || !/^\d+(?:\.\d+)*(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
    return "";
  }
  return version;
}

function normalizeBaseUrl(value) {
  const raw = String(value || "").trim();
  if (!raw) {
    throw new Error("TRON_VERSION_SERVER_URL is not configured");
  }

  const url = new URL(raw);
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error("TRON_VERSION_SERVER_URL must use http or https");
  }

  url.pathname = url.pathname.replace(/\/+$/, "");
  if (url.pathname.endsWith("/api/v1")) {
    url.pathname = url.pathname.slice(0, -"/api/v1".length);
  }
  return url;
}

function buildApiUrl(baseUrl, suffix) {
  const url = normalizeBaseUrl(baseUrl);
  const basePath = url.pathname.replace(/\/+$/, "");
  const [pathname, query] = String(suffix || "").split("?", 2);
  url.pathname = `${basePath}/api/v1${pathname}`;
  url.search = query ? `?${query}` : "";
  return url;
}

function readResponseError(response, body, operation) {
  const message = body && typeof body.message === "string" ? body.message : `${operation} failed with HTTP ${response.status}`;
  const error = new Error(message);
  error.status = response.status;
  error.code = body && (typeof body.code === "string" || typeof body.code === "number") ? String(body.code) : undefined;
  return error;
}

function unwrapApiData(body, response, operation) {
  if (!body || typeof body !== "object" || Array.isArray(body)) {
    return body;
  }

  if (!("code" in body) || !("data" in body)) {
    return body;
  }

  const code = body.code;
  const success = code === 0 || code === 200 || code === "0" || code === "200" || code === "SUCCESS";
  if (!success) {
    throw readResponseError(response, body, operation);
  }
  return body.data;
}

async function readJsonResponse(response, operation) {
  const body = await response.json().catch(() => null);
  if (!response.ok) {
    throw readResponseError(response, body, operation);
  }
  return unwrapApiData(body, response, operation);
}

function sanitizeFileName(value) {
  const fileName = path.basename(String(value || "")).replace(/[\u0000-\u001f]/g, "").trim();
  return fileName && fileName !== "." && fileName !== ".." ? fileName : "";
}

function installerExtension(platformName) {
  if (platformName === "win32") return ".exe";
  if (platformName === "darwin") return ".dmg";
  return ".AppImage";
}

function resolveInstallerFileName(packageInfo, version, platformName) {
  const expectedExtension = installerExtension(platformName);
  const configuredName = sanitizeFileName(packageInfo?.file_name ?? packageInfo?.fileName);
  const extensions = platformName === "linux" ? [".appimage", ".deb", ".rpm"] : [expectedExtension.toLowerCase()];
  if (configuredName && extensions.some((extension) => configuredName.toLowerCase().endsWith(extension))) {
    return configuredName;
  }
  return `Tron_${version}${expectedExtension}`;
}

function describeError(error) {
  if (error instanceof Error && error.message) return error.message;
  return String(error || "unknown error");
}

function describeErrorDetails(error) {
  if (!(error instanceof Error)) return describeError(error);
  const details = [error.name, error.message];
  for (const key of ["code", "errno", "syscall", "path"]) {
    if (error[key] !== undefined && error[key] !== null && error[key] !== "") {
      details.push(`${key}=${error[key]}`);
    }
  }
  if (error.stack) details.push(`stack=${error.stack.replace(/[\r\n]+/g, " | ")}`);
  return details.join("; ");
}

function cloneStatus(status) {
  return { ...status };
}

function defaultDataDir() {
  return path.join(os.homedir(), ".tron");
}

function defaultUpdaterLogPath(dataDir) {
  return path.join(dataDir, "logs", "tron-updater.log");
}

async function writeLog(logPath, message) {
  try {
    await fsp.mkdir(path.dirname(logPath), { recursive: true });
    await fsp.appendFile(logPath, `${new Date().toISOString()} ${message}\n`, "utf8");
  } catch (_error) {
    // Logging must never prevent the update helper from completing.
  }
}

async function waitForLauncherExit(input = process.stdin, parentPid, onProgress = null) {
  if (parentPid !== undefined) {
    if (!Number.isSafeInteger(parentPid) || parentPid <= 0 || parentPid === process.pid) throw new Error("invalid update launcher PID");
    const startedAt = Date.now();
    const deadline = Date.now() + 120000;
    let nextProgressAt = startedAt + 10000;
    await onProgress?.(`waiting for launcher process ${parentPid} to exit`);
    while (true) {
      try { process.kill(parentPid, 0); }
      catch (error) {
        if (error.code === "ESRCH") {
          await onProgress?.(`launcher process ${parentPid} exited after ${Date.now() - startedAt}ms`);
          return;
        }
        throw error;
      }
      if (Date.now() >= deadline) {
        await onProgress?.(`launcher process ${parentPid} remained active for ${Date.now() - startedAt}ms; giving up`);
        throw new Error("timed out waiting for the update launcher to exit");
      }
      if (Date.now() >= nextProgressAt) {
        await onProgress?.(`launcher process ${parentPid} is still active after ${Date.now() - startedAt}ms`);
        nextProgressAt += 10000;
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
  if (!input || typeof input.once !== "function" || input.readableEnded || input.destroyed) return;
  await onProgress?.("waiting for update launcher input stream to close");
  await new Promise((resolve) => {
    const done = () => {
      input.removeListener("end", done);
      input.removeListener("close", done);
      resolve();
    };
    input.once("end", done);
    input.once("close", done);
    if (typeof input.resume === "function") input.resume();
  });
  await onProgress?.("update launcher input stream closed");
}

function waitForSpawn(child) {
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      child.removeListener("spawn", spawned);
      child.removeListener("error", failed);
    };
    const spawned = () => { cleanup(); resolve(); };
    const failed = (error) => { cleanup(); reject(error); };
    child.once("spawn", spawned);
    child.once("error", failed);
  });
}

function waitForChildExit(child) {
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      child.removeListener("error", failed);
      child.removeListener("close", closed);
    };
    const closed = (code, signal) => {
      cleanup();
      if (code === 0) resolve({ code, signal });
      else reject(new Error(`update installer exited with code ${code}${signal ? ` (${signal})` : ""}`));
    };
    const failed = (error) => { cleanup(); reject(error); };
    child.once("error", failed);
    child.once("close", closed);
  });
}

function waitForHelperReady(child) {
  return new Promise((resolve, reject) => {
    let output = "", errors = "";
    const timer = setTimeout(() => done(new Error("update helper readiness timed out")), 15000);
    const stdout = (chunk) => {
      output += chunk.toString();
      if (output.includes(`${UPDATE_HELPER_READY}\n`)) done();
      else if (output.length > 4096) done(new Error("invalid update helper response"));
    };
    const stderr = (chunk) => { if (errors.length < 4096) errors += chunk.toString(); };
    const exited = (code) => done(new Error(errors.trim() || `update helper exited before ready (${code})`));
    const failed = (error) => done(error);
    const done = (error) => {
      clearTimeout(timer);
      child.stdout?.removeListener("data", stdout);
      child.stderr?.removeListener("data", stderr);
      child.removeListener("error", failed);
      child.removeListener("exit", exited);
      error ? reject(error) : resolve();
    };
    child.stdout?.on("data", stdout);
    child.stderr?.on("data", stderr);
    child.once("error", failed);
    child.once("exit", exited);
  });
}

function waitForWriterDrain(writer) {
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      writer.removeListener("drain", onDrain);
      writer.removeListener("error", onError);
    };
    const onDrain = () => {
      cleanup();
      resolve();
    };
    const onError = (error) => {
      cleanup();
      reject(error);
    };

    writer.once("drain", onDrain);
    writer.once("error", onError);
  });
}

async function runApplyUpdate(installerPath, options = {}) {
  const platformName = options.platformName ?? process.platform;
  const spawnImpl = options.spawnImpl ?? defaultSpawn;
  const input = options.input ?? process.stdin;
  const logPath = options.logPath ?? defaultUpdaterLogPath(defaultDataDir());
  const filePath = String(installerPath || "");

  if (!filePath) {
    await writeLog(logPath, "update helper: installer path is missing");
    return 1;
  }

  try {
    await writeLog(logPath, `update helper started: pid=${process.pid}; launcherPid=${options.parentPid ?? "none"}; installer=${filePath}`);
    options.onReady?.();
    await writeLog(logPath, "update helper reported ready to launcher");
    await waitForLauncherExit(input, options.parentPid, (message) => writeLog(logPath, `update helper: ${message}`));

    if (platformName !== "win32") {
      await writeLog(logPath, `update helper does not support platform ${platformName}`);
      return 1;
    }

    const args = ["--updated", "/S"];
    if (options.installDir) {
      if (!path.win32.isAbsolute(options.installDir)) throw new Error("invalid update installation directory");
      // NSIS consumes everything after /D= as the directory; keep it last.
      args.push(`/D=${options.installDir}`);
    }
    await writeLog(logPath, `update helper starting NSIS installer: ${filePath}; args=${JSON.stringify(args)}; cwd=${os.tmpdir()}`);
    const child = spawnImpl(filePath, args, {
      detached: false,
      stdio: "ignore",
      windowsHide: true,
      cwd: os.tmpdir()
    });
    const installerExit = waitForChildExit(child);
    await waitForSpawn(child);
    await writeLog(logPath, `update helper spawned NSIS installer: pid=${child.pid ?? "unknown"}`);
    const installerResult = await installerExit;
    await writeLog(logPath, `update helper NSIS installer exited successfully: code=${installerResult.code}; signal=${installerResult.signal ?? "none"}`);
    const executablePath = path.join(options.installDir || "", "Tron.exe");
    if (!options.installDir || !path.win32.isAbsolute(options.installDir)) throw new Error("invalid update installation directory");
    await writeLog(logPath, `update helper launching updated app: ${executablePath}`);
    const app = spawnImpl(executablePath, ["--updated"], {
      detached: true,
      stdio: "ignore",
      windowsHide: false,
      cwd: options.installDir
    });
    await waitForSpawn(app);
    app.unref?.();
    await writeLog(logPath, `update helper installed and launched app: pid=${app.pid ?? "unknown"}; executable=${executablePath}`);
    return 0;
  } catch (error) {
    await writeLog(logPath, `update helper failed: ${describeErrorDetails(error)}`);
    return 1;
  }
}

class Updater {
  constructor(options = {}) {
    this.dataDir = options.dataDir ?? defaultDataDir();
    this.getVersion = options.getVersion ?? (() => "0.0.0");
    this.getVersionServerUrl = options.getVersionServerUrl ?? (() => process.env.TRON_VERSION_SERVER_URL || DEFAULT_PERSON_AI_SERVER_BASE_URL);
    this.platformName = options.platformName ?? process.platform;
    this.archName = options.archName ?? process.arch;
    this.execPath = options.execPath ?? process.execPath;
    this.fetchImpl = options.fetchImpl ?? globalThis.fetch?.bind(globalThis);
    this.spawnImpl = options.spawnImpl ?? defaultSpawn;
    this.openInstaller = options.openInstaller ?? (async () => "desktop-open-not-configured");
    this.exit = options.exit ?? (() => {});
    this.onStatus = options.onStatus ?? (() => {});
    this.onLog = options.onLog ?? (() => {});
    this.status = { type: "idle" };
    this.update = null;
    this.downloadPromise = null;
    this.logPath = options.logPath ?? defaultUpdaterLogPath(this.dataDir);
  }

  getStatus() {
    return cloneStatus(this.status);
  }

  setStatus(status) {
    if (!UPDATE_TYPES.has(status.type)) {
      throw new Error(`unknown update status: ${status.type}`);
    }
    if (this.status.type !== status.type) {
      this.onLog(status.type === "error" ? "ERROR" : "INFO",
        status.type === "error" ? status.message : `Update status: ${status.type}`,
        { operation: "desktop.update", state: status.type, platform: this.platformName, architecture: this.archName });
    }
    this.status = { installationMode: this.platformName === "win32" ? "automatic" : "manual", ...status };
    this.onStatus(this.getStatus());
    return this.getStatus();
  }

  requireFetch() {
    if (typeof this.fetchImpl !== "function") {
      throw new Error("fetch is not available in the Electron runtime");
    }
    return this.fetchImpl;
  }

  requireToken(accessToken) {
    const token = String(accessToken || "").trim();
    if (!token) throw new Error("access token is required for update checks");
    return token;
  }

  async requestJson(suffix, token, operation) {
    const fetchImpl = this.requireFetch();
    const url = buildApiUrl(this.getVersionServerUrl(), suffix);
    const response = await fetchImpl(url, {
      headers: {
        Authorization: `Bearer ${token}`,
        "X-Product-Code": PRODUCT_CODE,
        Accept: "application/json"
      }
    });
    return readJsonResponse(response, operation);
  }

  async check(accessToken) {
    if (this.downloadPromise || ["checking", "downloaded", "installing"].includes(this.status.type)) return this.getStatus();
    const token = this.requireToken(accessToken);
    this.setStatus({ type: "checking" });

    try {
      const fetchImpl = this.requireFetch();
      const url = buildApiUrl(this.getVersionServerUrl(), `/versions/latest?product_code=${encodeURIComponent(PRODUCT_CODE)}`);
      const response = await fetchImpl(url, {
        headers: {
          Authorization: `Bearer ${token}`,
          "X-Product-Code": PRODUCT_CODE,
          Accept: "application/json"
        }
      });

      if (response.status === 404) {
        this.update = null;
        return this.setStatus({ type: "upToDate" });
      }

      const latest = await readJsonResponse(response, "checking for updates");
      const version = normalizeVersion(latest?.version_number ?? latest?.versionNumber);
      if (!version) throw new Error("version service returned no valid version number");

      if (compareVersions(version, this.getVersion()) <= 0) {
        this.update = null;
        return this.setStatus({ type: "upToDate" });
      }

      const osType = currentOsType(this.platformName, this.archName);
      const packages = Array.isArray(latest?.packages) ? latest.packages : [];
      const packageInfo = packages.find((item) => Number(item?.os_type ?? item?.osType) === osType);
      if (!packageInfo) {
        this.update = null;
        this.onLog("INFO", "No update package published for the current platform", {
          operation: "desktop.update.check", version, platform: this.platformName, architecture: this.archName
        });
        return this.setStatus({ type: "upToDate" });
      }

      const versionId = String(latest?.id || "").trim();
      if (!versionId) throw new Error("version service returned no version id");

      this.update = {
        version,
        versionId,
        packageInfo,
        fileName: resolveInstallerFileName(packageInfo, version, this.platformName)
      };
      return this.setStatus({
        type: "updateAvailable",
        version,
        versionId,
        releaseDate: String(latest?.release_date ?? latest?.releaseDate ?? ""),
        fileName: this.update.fileName
      });
    } catch (error) {
      this.update = null;
      this.setStatus({ type: "error", message: describeError(error) });
      throw error;
    }
  }

  async startDownload(accessToken) {
    if (this.downloadPromise) return true;
    if (this.status.type !== "updateAvailable" || !this.update) {
      throw new Error("no downloadable update is available");
    }

    const token = this.requireToken(accessToken);
    this.downloadPromise = this.download(token).then(() => this.apply()).finally(() => {
      this.downloadPromise = null;
    });
    await this.downloadPromise;
    return true;
  }

  async download(token) {
    const update = this.update;
    this.setStatus({ type: "downloading", version: update.version, progress: 0 });

    const updatesDir = path.join(this.dataDir, "updates");
    const finalPath = path.join(updatesDir, update.fileName);
    const partialPath = `${finalPath}.part`;

    try {
      const downloadPayload = await this.requestJson(
        `/versions/${encodeURIComponent(update.versionId)}/download?os_type=${currentOsType(this.platformName, this.archName)}`,
        token,
        "requesting update download"
      );
      const downloadUrl = String(downloadPayload?.download_url ?? downloadPayload?.downloadUrl ?? "").trim();
      if (!downloadUrl) throw new Error("version service returned no download URL");

      const downloadResponse = await this.requireFetch()(downloadUrl, { headers: { Accept: "application/octet-stream" } });
      if (!downloadResponse.ok) {
        throw readResponseError(downloadResponse, null, "downloading update");
      }
      if (!downloadResponse.body || typeof downloadResponse.body.getReader !== "function") {
        throw new Error("update download response has no readable body");
      }

      await fsp.mkdir(updatesDir, { recursive: true });
      await fsp.rm(partialPath, { force: true });
      const writer = fs.createWriteStream(partialPath, { flags: "wx" });
      let writerError = null;
      writer.on("error", (error) => {
        writerError = error;
      });
      const reader = downloadResponse.body.getReader();
      const total = Number(downloadResponse.headers?.get?.("content-length") || 0);
      let downloaded = 0;

      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          if (!value || value.length === 0) continue;
          downloaded += value.length;
          if (!writer.write(Buffer.from(value))) await waitForWriterDrain(writer);
          if (total > 0) {
            this.setStatus({
              type: "downloading",
              version: update.version,
              progress: Math.min(100, (downloaded / total) * 100)
            });
          }
        }
        if (writerError) throw writerError;
        const completed = finished(writer, { cleanup: true });
        writer.end();
        await completed;
        if (!downloaded || (total > 0 && downloaded !== total)) throw new Error("update download is incomplete; retry the download");
      } catch (error) {
        await new Promise((resolve) => {
          if (writer.closed) {
            resolve();
            return;
          }
          writer.once("close", resolve);
          writer.once("error", resolve);
          writer.destroy();
        });
        throw error;
      }

      await fsp.rm(finalPath, { force: true });
      await fsp.rename(partialPath, finalPath);
      this.setStatus({
        type: "downloaded",
        version: update.version,
        filePath: finalPath,
        fileName: update.fileName
      });
      return this.getStatus();
    } catch (error) {
      await fsp.rm(partialPath, { force: true }).catch(() => {});
      this.setStatus({ type: "error", message: describeError(error) });
      throw error;
    }
  }

  async apply() {
    if (this.status.type === "installing") return true;
    if (this.status.type !== "downloaded") throw new Error("no downloaded update is available");
    const version = this.status.version;
    try {
      const filePath = String(this.status.filePath || "");
      if (!filePath || !fs.existsSync(filePath)) throw new Error("downloaded update file is missing");

      if (this.platformName === "darwin" || this.platformName === "linux") {
        if (this.platformName === "linux" && filePath.toLowerCase().endsWith(".appimage")) await fsp.chmod(filePath, 0o755);
        const openError = await this.openInstaller(filePath);
        if (openError) throw new Error(String(openError));
        if (this.platformName === "darwin") {
          // The DMG is user-driven, but the running copy must exit before its
          // replacement can be copied into Applications.
          this.setStatus({ type: "installing", version });
          setTimeout(() => this.exit(0), 600).unref?.();
        }
        return true;
      }

      if (this.platformName !== "win32") throw new Error(`automatic installation is not supported on ${this.platformName}`);

      this.setStatus({ type: "installing", version });
      this.onLog("INFO", "Starting Windows update helper", {
        operation: "desktop.update.apply",
        version,
        installerPath: filePath,
        helperExecutable: this.execPath,
        launcherPid: process.pid,
        installDir: path.dirname(this.execPath),
        helperLogPath: this.logPath
      });
      const child = this.spawnImpl(this.execPath, ["--_tron-apply-update", filePath, this.logPath, path.dirname(this.execPath), String(process.pid)], {
        detached: true,
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
        cwd: os.tmpdir()
      });
      child.stdin?.on("error", () => {});
      this.onLog("INFO", "Windows update helper process created", {
        operation: "desktop.update.apply", version, helperPid: child.pid ?? null
      });
      try { await waitForHelperReady(child); }
      catch (error) { child.stdin?.end(); child.kill?.(); throw error; }
      this.onLog("INFO", "Windows update helper reported ready; scheduling forced main-process exit", {
        operation: "desktop.update.apply", version, helperPid: child.pid ?? null, exitDelayMs: 600
      });
      child.unref?.();
      child.stdin?.unref?.();
      child.stdout?.unref?.();
      child.stderr?.unref?.();
      setTimeout(() => {
        this.onLog("INFO", "Forcing main process exit for update installation", {
          operation: "desktop.update.apply", version, helperPid: child.pid ?? null
        });
        this.exit(0);
      }, 600).unref?.();
      return true;
    } catch (error) {
      this.setStatus({ type: "error", version, message: describeError(error) });
      throw error;
    }
  }
}

function createUpdater(options) {
  return new Updater(options);
}

module.exports = {
  PRODUCT_CODE,
  UPDATE_STATUS_EVENT,
  UPDATE_HELPER_READY,
  OS_TYPES,
  Updater,
  buildApiUrl,
  compareVersions,
  createUpdater,
  currentOsType,
  normalizeVersion,
  readJsonResponse,
  resolveInstallerFileName,
  runApplyUpdate,
  waitForLauncherExit
};
