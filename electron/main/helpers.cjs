const fs = require("node:fs");
const http = require("node:http");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");

const HOST = "127.0.0.1";
const LAN_DEV_HOST = "0.0.0.0";
const SIDE_CAR_BASELINE_PORT = 8080;
const MAX_PORT_PROBE = 80;
const BACKEND_HEALTH_PATH = "/api/v1/health";
const DEFAULT_PERSON_AI_SERVER_BASE_URL = "https://personai.chat/person-ai-index";

function isHttpUrl(input) {
  try {
    const url = new URL(input);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch (_error) {
    return false;
  }
}

function normalizeTargetUrl(input) {
  if (isHttpUrl(input)) {
    return input;
  }

  if (!input) {
    return "";
  }

  return `https://${input}`;
}

function sanitizeUrl(raw) {
  return normalizeTargetUrl(String(raw || "").trim());
}

function loadPort() {
  const parsed = Number.parseInt(process.env.TRON_BACKEND_PORT || "", 10);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : SIDE_CAR_BASELINE_PORT;
}

function resolveBackendHost({ isPackaged = false } = {}) {
  return !isPackaged && process.env.TRON_ALLOW_LAN_DEV === "true" ? LAN_DEV_HOST : HOST;
}

function canBindPort(port, host = resolveBackendHost()) {
  return new Promise((resolve) => {
    const server = net.createServer();
    server.once("error", () => resolve(false));
    server.once("listening", () => {
      server.close(() => resolve(true));
    });
    server.listen(port, host);
  });
}

async function findFreePort(start, host = resolveBackendHost()) {
  const base = start > 0 ? start : SIDE_CAR_BASELINE_PORT;
  for (let offset = 0; offset < MAX_PORT_PROBE; offset += 1) {
    const candidate = base + offset;
    if (await canBindPort(candidate, host)) {
      return candidate;
    }
  }

  throw new Error(`no free port found in ${base}..${base + MAX_PORT_PROBE}`);
}

function requestHealth(url) {
  return new Promise((resolve) => {
    const request = http.get(`${url}${BACKEND_HEALTH_PATH}`, (response) => {
      response.resume();
      resolve(response.statusCode >= 200 && response.statusCode < 300);
    });
    request.setTimeout(1200, () => {
      request.destroy();
      resolve(false);
    });
    request.once("error", () => resolve(false));
  });
}

const BACKEND_STARTUP_TIMEOUT_MS = 10 * 60 * 1000;

async function waitBackendReady(url) {
  const deadline = performance.now() + BACKEND_STARTUP_TIMEOUT_MS;
  while (performance.now() < deadline) {
    if (await requestHealth(url)) {
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, Math.min(150, deadline - performance.now())));
  }

  return false;
}

function resolveDesktopDataDir({ isPackaged = false, platformName = process.platform, execPath = process.execPath, homeDir = os.homedir() } = {}) {
  if (platformName === "win32" && isPackaged) {
    return path.win32.join(path.win32.dirname(execPath), "data");
  }
  return path.join(homeDir, ".tron");
}

function resolveControlDataDir(appDataDir) {
  return path.join(appDataDir, "control");
}

function resolveControlDatabasePath(appDataDir) {
  return path.join(resolveControlDataDir(appDataDir), "profiles.sqlite");
}

function resolveSharedDatabasePath(appDataDir) {
  return path.join(appDataDir, "tron.sqlite3");
}

function resolveSharedGraphStoragePath(appDataDir) {
  return path.join(appDataDir, "tron.graph.grafeo");
}

function resolveBackendLogPath(appDataDir) {
  return path.join(appDataDir, "logs", "tron-backend.log");
}

function resolveUpdateServerUrl(resourcesDir) {
  const environmentUrl = String(process.env.TRON_VERSION_SERVER_URL || "").trim();
  if (environmentUrl) return environmentUrl;

  const configPath = path.join(resourcesDir, "update-config.json");
  try {
    const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
    if (typeof config.versionServerUrl === "string" && config.versionServerUrl.trim()) {
      return config.versionServerUrl.trim();
    }
  } catch (_error) {
    // Fall through to the Person-AI SDK default.
  }

  return DEFAULT_PERSON_AI_SERVER_BASE_URL;
}

function resolveBackendConfigPath(appDataDir, resourcesDir, options = {}) {
  const userConfigPath = path.join(appDataDir, "application.yaml");
  if (options.backendInitialize) return userConfigPath;
  if (fs.existsSync(userConfigPath)) {
    return userConfigPath;
  }

  const bundledConfigPath = path.join(resourcesDir, "application.yaml");
  if (!fs.existsSync(bundledConfigPath)) {
    throw new Error(`backend application config is missing: ${bundledConfigPath}`);
  }

  try {
    let config = fs.readFileSync(bundledConfigPath, "utf8");
    if (options.relocateStorage) {
      config = config.replace(
        /^([ \t]*root:)[ \t]*~\/\.tron\/resources[ \t]*$/m,
        (_match, key) => `${key} ${JSON.stringify(path.join(appDataDir, "resources"))}`,
      );
      if (!/^skillsRegistryRoot:/m.test(config)) {
        config = `skillsRegistryRoot: ${JSON.stringify(path.join(appDataDir, "skills-registry"))}\n${config}`;
      }
    }
    if (options.profileConfig) {
      config = config.replace(
        /^([ \t]*keyBase64:)[ \t]*.*$/m,
        "$1 \"\"",
      );
    }
    fs.writeFileSync(userConfigPath, config, { encoding: "utf8", flag: "wx" });
    if (process.platform !== "win32") {
      fs.chmodSync(userConfigPath, 0o600);
    }
  } catch (error) {
    if (error?.code !== "EEXIST") {
      throw error;
    }
  }

  return userConfigPath;
}

function resolveResourcesDir(app) {
  if (app?.isPackaged) {
    return process.resourcesPath;
  }

  return path.resolve(__dirname, "..", "resources");
}

function resolveFrontendDistDir(resourcesDir) {
  const candidate = path.join(resourcesDir, "frontend");
  const nested = path.join(candidate, "frontend");

  if (fs.existsSync(path.join(candidate, "index.html"))) {
    return candidate;
  }

  if (fs.existsSync(path.join(nested, "index.html"))) {
    return nested;
  }

  return candidate;
}

function resolveSidecarPath(resourcesDir) {
  const binaryName = process.platform === "win32" ? "tron-backend.exe" : "tron-backend";
  const candidate = path.join(resourcesDir, "binaries", binaryName);
  if (fs.existsSync(candidate)) {
    return candidate;
  }

  throw new Error("tron-backend sidecar binary was not found in app resources");
}

function iframeBlockReasonFromHeaders(headers) {
  const xFrameOptions = readHeaderValues(headers, "x-frame-options")
    .map((value) => value.trim().toLowerCase());
  for (const value of xFrameOptions) {
    if (value === "deny") {
      return "x-frame-options-deny";
    }
    if (value.startsWith("sameorigin")) {
      return "x-frame-options-sameorigin";
    }
  }

  const cspValues = readHeaderValues(headers, "content-security-policy");
  for (const value of cspValues) {
    const directive = value
      .split(";")
      .map((item) => item.trim())
      .find((item) => item.toLowerCase().startsWith("frame-ancestors"));
    const normalized = String(directive || "").toLowerCase();
    if (normalized.includes("'none'")) {
      return "csp-frame-ancestors-none";
    }
    if (normalized.includes("'self'")) {
      return "csp-frame-ancestors-self";
    }
  }

  return null;
}

function readHeaderValues(headers, name) {
  const raw = headers?.[name] ?? headers?.[name.toLowerCase()];
  if (Array.isArray(raw)) {
    return raw.map(String);
  }
  if (typeof raw === "string") {
    return [raw];
  }
  return [];
}

async function checkTargetPageEmbeddingStatus(rawUrl) {
  const targetUrl = sanitizeUrl(rawUrl);
  if (!isHttpUrl(targetUrl)) {
    throw new Error("Only http/https URLs are allowed");
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 4500);
  try {
    const response = await fetch(targetUrl, {
      redirect: "follow",
      signal: controller.signal,
      headers: {
        "user-agent": "Mozilla/5.0 Tron Desktop"
      }
    });
    const headers = {};
    response.headers.forEach((value, key) => {
      headers[key.toLowerCase()] = value;
    });
    const reason = iframeBlockReasonFromHeaders(headers);

    return {
      url: response.url || targetUrl,
      embeddable: !reason,
      blocked: Boolean(reason),
      reason: reason || undefined
    };
  } finally {
    clearTimeout(timeout);
  }
}

module.exports = {
  BACKEND_HEALTH_PATH,
  DEFAULT_PERSON_AI_SERVER_BASE_URL,
  HOST,
  LAN_DEV_HOST,
  SIDE_CAR_BASELINE_PORT,
  checkTargetPageEmbeddingStatus,
  findFreePort,
  iframeBlockReasonFromHeaders,
  isHttpUrl,
  loadPort,
  resolveDesktopDataDir,
  resolveBackendLogPath,
  resolveControlDataDir,
  resolveControlDatabasePath,
  resolveBackendConfigPath,
  resolveUpdateServerUrl,
  resolveFrontendDistDir,
  resolveSharedDatabasePath,
  resolveSharedGraphStoragePath,
  resolveBackendHost,
  resolveResourcesDir,
  resolveSidecarPath,
  sanitizeUrl,
  BACKEND_STARTUP_TIMEOUT_MS,
  waitBackendReady
};
