const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
function fail(message) { console.error('错误：' + message); process.exit(1); }
const root = process.cwd();
const requestedArch = process.env.TRON_WSL_ARCH;
const targets = {
    x86_64: ['linux/amd64', 'x86_64-unknown-linux-gnu'],
    aarch64: ['linux/arm64', 'aarch64-unknown-linux-gnu']
};
if (requestedArch !== 'all' && !Object.hasOwn(targets, requestedArch)) fail('WSL_ARCH 只支持 x86_64、aarch64 或 all。');
if (!['true', 'false'].includes(process.env.TRON_WSL_SKIP_TESTS)) fail('SKIP_TESTS 只支持 true 或 false。');
const revision = fs.readFileSync('version.properties', 'utf8').match(/^revision\s*=\s*(\S+)\s*$/m)?.[1];
if (!revision || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(revision)) fail('version.properties 缺少有效的 revision。');
const manifestVersion = fs.readFileSync('tron-backend/Cargo.toml', 'utf8').match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (manifestVersion !== revision) fail('后端版本与 revision 不一致，请先执行 make version。');
const requestedVersion = process.env.TRON_WSL_VERSION;
if (requestedVersion && requestedVersion !== revision) fail('VERSION 必须等于 ' + revision + '；修改版本请先执行 make version VERSION=<版本>。');
function docker(args, options = {}) {
    const result = spawnSync('docker', args, { stdio: 'inherit', ...options });
    if (result.error) fail('无法执行 Docker：' + result.error.message);
    if (result.status !== 0) fail('Docker 命令失败，请查看以上日志，检查镜像访问、构建依赖及目标架构支持。');
    return result;
}
const info = docker(['info', '--format', '{{.OSType}}'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] });
if (info.stdout.trim() !== 'linux') fail('请将 Docker 切换为 Linux 容器。');
if (root.includes(',')) fail('Docker 挂载不支持包含逗号的仓库路径。');
const cacheId = crypto.createHash('sha256').update(root).digest('hex').slice(0, 12);
const uid = process.getuid ? process.getuid() : 0;
const gid = process.getgid ? process.getgid() : 0;
const image = process.env.TRON_WSL_IMAGE;
// Forward caller-supplied proxies to APT, Cargo and Git inside the container.
// A loopback proxy on the host is not reachable as 127.0.0.1 from a Docker
// container (especially with Colima), so forwarding it makes an otherwise
// healthy build fail with a misleading APT connection error. Opt in only when
// the proxy is intentionally reachable from the container network.
const allowLoopbackProxy = process.env.TRON_WSL_USE_LOOPBACK_PROXY === 'true';
const loopbackProxy = /^(?:https?|socks(?:4|5)?:\/\/)?(?:localhost|127\.0\.0\.1|\[::1\])(?::\d+)?(?:\/|$)/i;
const proxyArgs = ['http_proxy', 'https_proxy', 'all_proxy', 'no_proxy'].flatMap((name) => {
    const value = process.env[name] ?? process.env[name.toUpperCase()];
    if (!value) return [];
    if (name !== 'no_proxy' && loopbackProxy.test(value) && !allowLoopbackProxy) {
        console.warn('忽略容器不可达的本机代理：' + name + '=' + value);
        return [];
    }
    return ['--env', name + '=' + value];
});
for (const arch of requestedArch === 'all' ? Object.keys(targets) : [requestedArch]) {
    const [platform, target] = targets[arch];
    const output = path.join(root, 'output', 'wsl2', revision, target);
    fs.mkdirSync(output, { recursive: true });
    console.log('构建 ' + target + '，版本 ' + revision + '，镜像 ' + image);
    docker([
        'run', '--rm', '--init', '--interactive', '--platform', platform,
        '--mount', 'type=bind,source=' + root + ',target=/workspace,readonly',
        '--mount', 'type=bind,source=' + output + ',target=/output',
        '--mount', 'type=volume,source=tron-wsl-cargo-' + cacheId + ',target=/cargo-cache',
        '--mount', 'type=volume,source=tron-wsl-build-' + cacheId + '-' + arch + ',target=/build',
        '--workdir', '/workspace',
        '--env', 'TRON_WSL_TARGET=' + target,
        '--env', 'TRON_WSL_VERSION=' + revision,
        '--env', 'TRON_WSL_IMAGE=' + image,
        '--env', 'TRON_WSL_UID=' + uid,
        '--env', 'TRON_WSL_GID=' + gid,
        ...proxyArgs,
        image, 'sh', '-s'
    ], { input: fs.readFileSync(path.join(__dirname, 'build-wsl.sh'), 'utf8').replace(/\r\n/g, '\n'), stdio: ['pipe', 'inherit', 'inherit'] });
    for (const file of ['tron-backend', 'SHA256SUMS', 'runtime-dependencies.txt']) {
        if (!fs.statSync(path.join(output, file)).isFile()) fail('缺少构建产物：' + file);
    }
    console.log('构建完成：' + output);
}
