const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const http = require("node:http");
const { execFile, spawn } = require("node:child_process");
const { once } = require("node:events");
const { promisify } = require("node:util");
const { buildApiUrl, normalizeVersion, readJsonResponse } = require("./updater.cjs");
const { BACKEND_STARTUP_TIMEOUT_MS, findFreePort } = require("./helpers.cjs");
const { secureDownloadUrl } = require("./wsl-download.cjs");

const execFileAsync = promisify(execFile);
const runtime = fs.readFileSync(path.join(__dirname, "wsl/runtime.py"), "utf8");
const dependencies = fs.readFileSync(path.join(__dirname, "wsl/dependencies.sh"), "utf8").replace(/\r\n/g, "\n");
const fileRuntime = () => fs.readFileSync(path.join(__dirname, "wsl/files.py"), "utf8");
const MAX_DOWNLOAD = 512 * 1024 * 1024;

function decodeWslOutput(buffer) {
  return buffer.toString(buffer.includes(0) ? "utf16le" : "utf8").replace(/^\uFEFF/, "").trim();
}

function distributionName(value) {
  if (typeof value !== "string" || !value.trim() || /[\x00-\x1f]/.test(value) || value.length > 256) throw new Error("invalid-wsl-distribution");
  return value;
}

function run(args, { input, timeout = 30000, binary = false } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn("wsl.exe", args, { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
    const stdout = [], stderr = [];
    let size = 0;
    let failure;
    const timer = setTimeout(() => { failure = new Error("wsl-command-timeout"); child.kill(); }, timeout);
    child.stdout.on("data", (chunk) => {
      size += chunk.length;
      if (size > 8 * 1024 * 1024) { failure = new Error("wsl-output-too-large"); child.kill(); }
      else stdout.push(chunk);
    });
    child.stderr.on("data", (chunk) => { if (Buffer.concat(stderr).length < 16000) stderr.push(chunk); });
    child.on("error", (error) => { clearTimeout(timer); reject(error.code === "ENOENT" ? new Error("wsl-not-installed") : error); });
    child.stdin.on("error", () => {});
    child.once("close", (code) => {
      clearTimeout(timer);
      if (failure || code !== 0) reject(failure || new Error(decodeWslOutput(Buffer.concat(stderr)) || decodeWslOutput(Buffer.concat(stdout)) || "wsl-command-failed"));
      else resolve(binary ? Buffer.concat(stdout) : decodeWslOutput(Buffer.concat(stdout)));
    });
    child.stdin.end(input);
  });
}

function argsFor(distro, command, root = false) {
  return ["--distribution", distributionName(distro), ...(root ? ["--user", "root"] : []), "--exec", ...command];
}

function requestText(url) {
  return new Promise((resolve) => {
    const request = http.get(url, (response) => {
      let body = "";
      response.on("data", (chunk) => { body += chunk; if (body.length > 4096) request.destroy(); });
      response.on("end", () => resolve(response.statusCode === 200 ? body : ""));
      response.on("error", () => resolve(""));
    });
    request.setTimeout(1500, () => request.destroy());
    request.on("error", () => resolve(""));
  });
}

class WslBackend {
  constructor({ dataDir, getVersion, getVersionServerUrl, frontendDir, sidecarPath, fetchImpl = globalThis.fetch, fetchDownloadImpl = fetchImpl, onStatus = () => {} }) {
    this.preferencePath = path.join(dataDir, "backend-environment.json");
    this.getVersion = getVersion;
    this.getVersionServerUrl = getVersionServerUrl;
    this.frontendDir = frontendDir;
    this.sidecarPath = sidecarPath;
    this.platforms = null;
    this.fetch = fetchImpl;
    this.fetchDownload = fetchDownloadImpl;
    this.onStatus = onStatus;
    this.status = { phase: "idle" };
    this.busy = false;
    this.session = null;
    this.roots = new Map();
  }

  selection() {
    let value;
    try { value = JSON.parse(fs.readFileSync(this.preferencePath, "utf8")); }
    catch (error) { if (error.code === "ENOENT") return { mode: "local" }; throw new Error("invalid-backend-preference"); }
    if (value.mode === "local") return { mode: "local" };
    if (value.mode === "wsl") return { mode: "wsl", distribution: distributionName(value.distribution) };
    throw new Error("invalid-backend-preference");
  }

  saveSelection(value) {
    const selection = value.mode === "local" ? { mode: "local" } : { mode: "wsl", distribution: distributionName(value.distribution) };
    fs.mkdirSync(path.dirname(this.preferencePath), { recursive: true });
    fs.writeFileSync(this.preferencePath + ".tmp", JSON.stringify(selection), { mode: 0o600 });
    fs.renameSync(this.preferencePath + ".tmp", this.preferencePath);
  }

  setStatus(phase, details = {}) {
    this.status = { phase, ...details };
    this.onStatus(this.status);
    return this.status;
  }

  async list() {
    const names = (await run(["--list", "--quiet"])).split(/\r?\n/).map((name) => name.trim()).filter(Boolean);
    const verbose = (await run(["--list", "--verbose"])).split(/\r?\n/);
    const ordered = [...names].sort((left, right) => right.length - left.length);
    return verbose.flatMap((raw) => {
      const line = raw.replace(/^\s*\*?\s*/, "");
      const name = ordered.find((candidate) => line.startsWith(candidate + " "));
      const wslVersion = Number(line.match(/\s([12])\s*$/)?.[1] || 0);
      return name && wslVersion ? [{ name, wslVersion }] : [];
    });
  }

  async probe(distro) {
    const installed = (await this.list()).find((entry) => entry.name === distro);
    if (!installed || installed.wslVersion !== 2) throw new Error("wsl2-distribution-required");
    const result = await run(argsFor(distro, ["sh", "-c", 'set -eu; . /etc/os-release; printf "%s\\n" "$ID" "$VERSION_ID" "$(uname -m)" "$HOME"']));
    const [os, version, architecture, home] = result.split(/\r?\n/);
    const osType = (await this.wslPlatforms())[os]?.[architecture];
    if (!osType || !/^\d+(?:\.\d+)*$/.test(version) || (os === "ubuntu" ? Number(version.split(".").slice(0, 2).join(".")) < 24.04 : Number(version.split(".")[0]) < 13)) {
      throw new Error("wsl-unsupported-distribution: Ubuntu 24.04+ / Debian 13+, x86_64 / ARM64");
    }
    if (!home?.startsWith("/") || /^\/mnt\/[a-z](?:\/|$)/i.test(home)) throw new Error("wsl-linux-path-required");
    return { distribution: distro, os, version, architecture, home, osType };
  }

  async listInstallations() {
    const distributions = await this.list();
    return Promise.all(distributions.map(async (item) => ({
      ...item,
      tronInstalled: await run(argsFor(item.name, ["sh", "-c", 'for release in "$HOME"/.tron/desktop/releases/*; do if [ -f "$release/tron-backend" ]; then printf installed; break; fi; done'])) === "installed",
    })));
  }

  async uninstall(distro, deleteUserData = false) {
    if (this.busy || this.session) throw new Error("wsl-operation-in-progress");
    this.busy = true;
    try {
      distributionName(distro);
      if (!(await this.list()).some((item) => item.name === distro)) throw new Error("invalid-wsl-distribution");
      this.setStatus("uninstalling", { distribution: distro });
      await run(argsFor(distro, ["python3", "-c", runtime, "uninstall", deleteUserData === true ? "delete-data" : "keep-data"]), { timeout: 120000 });
      this.setStatus("idle");
    } catch (error) { this.setStatus("error", { distribution: distro, message: error.message }); throw error; }
    finally { this.busy = false; }
  }

  async wslPlatforms() {
    if (!this.platforms) {
      const { stdout } = await execFileAsync(this.sidecarPath, ["--wsl-platforms"], {
        windowsHide: true, timeout: 10000, maxBuffer: 16 * 1024, encoding: "utf8"
      });
      const platforms = JSON.parse(stdout);
      for (const os of ["ubuntu", "debian"]) {
        for (const architecture of ["x86_64", "aarch64"]) {
          if (!Number.isInteger(platforms?.[os]?.[architecture]) || platforms[os][architecture] <= 0) {
            throw new Error("wsl-platforms-unavailable");
          }
        }
      }
      this.platforms = platforms;
    }
    return this.platforms;
  }

  async api(suffix) {
    const response = await this.fetch(buildApiUrl(this.getVersionServerUrl(), suffix).href, {
      headers: { Accept: "application/json", "X-Product-Code": "tron" }, signal: AbortSignal.timeout(30000)
    });
    return readJsonResponse(response, "WSL package lookup");
  }

  async ensureInstalled(distro) {
    const version = normalizeVersion(this.getVersion());
    this.setStatus("checking", { distribution: distro });
    const info = await this.probe(distro);
    let installation;
    try { installation = JSON.parse(await run(argsFor(distro, ["python3", "-c", runtime, "inspect", version]))); } catch { /* Bootstrap Python and the runtime libraries on first use. */ }
    if (installation?.installed) return { ...info, ...installation, backendVersion: version };

    // The current API exposes the latest release only. Do not mix desktop assets
    // with a different backend version or silently choose a generic Linux package.
    const latest = await this.api("/versions/latest?product_code=tron");
    if (normalizeVersion(latest?.version_number ?? latest?.versionNumber) !== version) throw new Error("wsl-version-mismatch");
    const packageInfo = latest?.packages?.find((entry) => Number(entry.os_type ?? entry.osType) === info.osType);
    if (!packageInfo || !latest.id) throw new Error("wsl-package-not-published");

    this.setStatus("dependencies", { distribution: distro });
    await run(argsFor(distro, ["sh", "-s"], true), { input: dependencies, timeout: 600000 });
    const frontend = await run(argsFor(distro, ["wslpath", "-u", this.frontendDir]));
    const link = await this.api(`/versions/${encodeURIComponent(String(latest.id))}/download?os_type=${info.osType}`);
    const url = secureDownloadUrl(link?.download_url ?? link?.downloadUrl);
    this.setStatus("downloading", { distribution: distro, progress: 0 });
    const response = await this.fetchDownload(url.href, { signal: AbortSignal.timeout(600000) });
    if (!response.ok || !response.body) {
      await response.body?.cancel().catch(() => {});
      throw new Error("wsl-package-download-failed");
    }
    try { secureDownloadUrl(response.url, url); }
    catch (error) { await response.body.cancel().catch(() => {}); throw error; }
    const total = Number(response.headers.get("content-length")) || 0;
    if (total > MAX_DOWNLOAD) throw new Error("package-too-large");
    const child = spawn("wsl.exe", argsFor(distro, ["python3", "-c", runtime, "install", version, frontend]), { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
    let output = "", errors = "";
    child.stdout.on("data", (chunk) => { if (output.length < 16000) output += chunk; });
    child.stderr.on("data", (chunk) => { if (errors.length < 16000) errors += chunk; });
    child.stdin.on("error", () => {});
    const completed = new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("close", (code) => code === 0 ? resolve() : reject(new Error(errors.trim() || "wsl-package-install-failed")));
    });
    // Observe an early installer failure even while the download is in flight.
    completed.catch(() => {});
    const timer = setTimeout(() => child.kill(), 660000);
    const reader = response.body.getReader();
    try {
      let received = 0;
      const digest = crypto.createHash("sha256");
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        received += value.length;
        digest.update(value);
        if (received > MAX_DOWNLOAD) throw new Error("package-too-large");
        if (!child.stdin.write(Buffer.from(value))) await once(child.stdin, "drain");
        const progress = total ? Math.min(99, Math.floor(received * 100 / total)) : undefined;
        if (progress !== this.status.progress) this.setStatus("downloading", { distribution: distro, progress });
      }
      if (!received || (total && received !== total)) throw new Error("wsl-package-incomplete");
      this.setStatus("installing", { distribution: distro });
      child.stdin.end(`\nTRON-DOWNLOAD-COMPLETE ${digest.digest("hex")} ${received}\n`);
      await completed;
      return { ...info, ...JSON.parse(output), backendVersion: version };
    } catch (error) {
      await reader.cancel().catch(() => {});
      child.stdin.destroy();
      child.kill();
      throw error;
    } finally { clearTimeout(timer); }
  }

  async prepare(distro) {
    if (this.busy) throw new Error("wsl-operation-in-progress");
    this.busy = true;
    try {
      const info = await this.ensureInstalled(distributionName(distro));
      this.setStatus("installed", info);
      return info;
    } catch (error) { this.setStatus("error", { distribution: distro, message: error.message }); throw error; }
    finally { this.busy = false; }
  }

  async start(distro, initialPort = 8080) {
    const info = await this.prepare(distro);
    this.setStatus("starting", info);
    let port = Math.max(initialPort, 1024);
    for (let attempt = 0; attempt < 80; attempt += 1) {
      port = await findFreePort(port, "127.0.0.1");
      const linuxPort = Number(await run(argsFor(distro, ["python3", "-c", "import socket,sys\nfor port in range(int(sys.argv[1]), min(int(sys.argv[1])+80,65536)):\n try:\n  with socket.socket() as probe: probe.bind(('127.0.0.1',port))\n  print(port); break\n except OSError: pass\nelse: sys.exit('wsl-no-free-port')", String(port)])));
      if (!Number.isInteger(linuxPort) || linuxPort < port || linuxPort > 65535) throw new Error("wsl-no-free-port");
      if (linuxPort === port) break;
      port = linuxPort;
      if (attempt === 79) throw new Error("wsl-no-free-port");
    }
    const nonce = crypto.randomBytes(16).toString("hex");
    const serveArgs = ["python3", "-u", "-c", runtime, "serve", info.backendVersion, String(port), nonce, String(BACKEND_STARTUP_TIMEOUT_MS / 1000)];
    const child = spawn("wsl.exe", argsFor(distro, serveArgs), { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
    let output = "", errors = "";
    child.stdin.on("error", () => {});
    child.stderr.on("data", (chunk) => { if (errors.length < 16000) errors += chunk; });
    const session = { ...info, port, backendUrl: `http://127.0.0.1:${port}`, process: child };
    this.session = session;
    child.once("exit", () => {
      if (this.session === session) {
        this.session = null;
        this.setStatus("error", { distribution: distro, backendExited: true, message: errors.trim() || "wsl-backend-exited" });
      }
    });
    try {
      await new Promise((resolve, reject) => {
        const timer = setTimeout(() => done(new Error("wsl-backend-readiness-timeout")), BACKEND_STARTUP_TIMEOUT_MS + 30_000);
        const closed = () => done(new Error(errors.trim() || "wsl-backend-exited"));
        const failed = (error) => done(error);
        const data = (chunk) => { output += chunk; if (output.includes("\n")) done(); };
        const done = (error) => {
          clearTimeout(timer);
          child.stdout.removeListener("data", data);
          child.removeListener("close", closed);
          child.removeListener("error", failed);
          error ? reject(error) : resolve();
        };
        child.stdout.on("data", data);
        child.once("close", closed);
        child.once("error", failed);
      });
      const started = JSON.parse(output.trim());
      if (started.port !== port || started.version !== info.backendVersion) throw new Error("wsl-backend-handshake-failed");
      // Validate the exact WSL instance, not any other process on the same port.
      let connected = false;
      for (let attempt = 0; attempt < 40 && this.session === session; attempt += 1) {
        if (await requestText(`${session.backendUrl}/tron-runtime-${nonce}.txt`) === nonce) { connected = true; break; }
        await new Promise((resolve) => setTimeout(resolve, 250));
      }
      if (!connected) throw new Error("wsl-localhost-forwarding-unavailable");
      this.setStatus("ready", { ...info, backendUrl: session.backendUrl });
      return session;
    } catch (error) { this.stop(); this.setStatus("error", { ...info, message: error.message }); throw error; }
  }

  stop() {
    const session = this.session;
    this.session = null;
    this.roots.clear();
    if (!session) return;
    session.process.stdin.end();
    // Normally the Linux supervisor exits after graceful shutdown. EOF still
    // reaches it if Electron exits before this fallback timer can run.
    const timer = setTimeout(() => session.process.kill(), 12000);
    timer.unref();
    session.process.once("close", () => clearTimeout(timer));
  }

  async fileCommand(command, payload, browserWindow) {
    const session = this.session;
    if (!session) throw new Error("wsl-backend-not-connected");
    if (command === "fs:pickRoot") {
      const { dialog } = require("electron");
      const prefix = `\\\\wsl.localhost\\${session.distribution}`;
      const result = await dialog.showOpenDialog(browserWindow, { defaultPath: prefix + session.home.replaceAll("/", "\\"), properties: ["openDirectory", "createDirectory"] });
      if (result.canceled || !result.filePaths?.length) return null;
      const match = result.filePaths[0].match(/^\\\\(?:wsl\.localhost|wsl\$)\\([^\\]+)(\\.*)$/i);
      if (!match || match[1].toLowerCase() !== session.distribution.toLowerCase()) throw new Error("wsl-linux-path-required");
      const selected = match[2].replaceAll("\\", "/");
      const root = JSON.parse(await run(argsFor(session.distribution, ["python3", "-c", fileRuntime()]), { input: JSON.stringify({ command: "root", root: selected }) }));
      const rootId = crypto.randomUUID();
      this.roots.set(rootId, root.path);
      return { rootId, ...root };
    }
    const root = this.roots.get(payload.rootId);
    if (!root) throw new Error("root-not-found");
    if (command === "fs:closeRoot") { this.roots.delete(payload.rootId); return {}; }
    if (command === "fs:openInEditor") {
      if (payload.editor !== "vscode") throw new Error("editor-not-found");
      await run(argsFor(session.distribution, ["sh", "-c", 'DONT_PROMPT_WSL_INSTALL=1 code "$1"', "tron-editor", root]));
      return {};
    }
    return JSON.parse(await run(argsFor(session.distribution, ["python3", "-c", fileRuntime()]), { input: JSON.stringify({ ...payload, command, root }) }));
  }
}

module.exports = { WslBackend, decodeWslOutput };
