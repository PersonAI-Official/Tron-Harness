"""Executed inside the selected WSL distribution, as its default Linux user."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
import urllib.request
import zipfile

MAX_PACKAGE = 512 * 1024 * 1024


def linux_path(value):
    resolved = Path(value).resolve()
    # Never turn Windows-mounted storage into the WSL data/work directory.
    filesystem = subprocess.check_output(
        ["stat", "-f", "-c", "%T", str(resolved)], text=True
    ).strip()
    if re.match(r"^/mnt/[a-z](?:/|$)", str(resolved), re.I) or filesystem in ("9p", "drvfs", "smb2"):
        raise ValueError("wsl-linux-path-required")
    return resolved


def directories(version):
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?", version):
        raise ValueError("invalid-version")
    root = linux_path(Path.home()) / ".tron"
    root.mkdir(mode=0o700, exist_ok=True)
    root = linux_path(root)
    desktop = root / "desktop"
    desktop.mkdir(mode=0o700, exist_ok=True)
    desktop = linux_path(desktop)
    releases = desktop / "releases"
    releases.mkdir(exist_ok=True)
    return root, desktop, linux_path(releases) / version


def emit(value):
    print(json.dumps(value), flush=True)


def unpack(package, stage):
    with package.open("rb") as source:
        magic = source.read(4)
    if magic == b"\x7fELF":
        shutil.copyfile(package, stage / "tron-backend")
        return
    allowed = {"tron-backend", "SHA256SUMS", "runtime-dependencies.txt"}
    seen = set()
    total = 0

    def copy(name, size, source):
        nonlocal total
        name = name.removeprefix("./")
        if name not in allowed or name in seen or size < 0:
            raise ValueError("invalid-wsl-package")
        total += size
        if total > MAX_PACKAGE:
            raise ValueError("package-too-large")
        seen.add(name)
        with (stage / name).open("xb") as output:
            shutil.copyfileobj(source, output, 1024 * 1024)

    if magic[:2] == b"PK":
        with zipfile.ZipFile(package) as archive:
            if len(archive.infolist()) > 8:
                raise ValueError("invalid-wsl-package")
            for entry in archive.infolist():
                if entry.is_dir() and entry.filename in ("./", "."):
                    continue
                if (entry.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError("invalid-wsl-package")
                with archive.open(entry) as source:
                    copy(entry.filename, entry.file_size, source)
    else:
        with tarfile.open(package, "r:*") as archive:
            for index, entry in enumerate(archive):
                if index > 8:
                    raise ValueError("invalid-wsl-package")
                if entry.isdir() and entry.name in (".", "./"):
                    continue
                if not entry.isfile():
                    raise ValueError("invalid-wsl-package")
                with archive.extractfile(entry) as source:
                    copy(entry.name, entry.size, source)


def install(version, frontend):
    root, desktop, release = directories(version)
    with (desktop / "install.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        stage = Path(tempfile.mkdtemp(prefix="install-", dir=desktop))
        try:
            package = stage / "package"
            size = 0
            with package.open("xb") as output:
                while chunk := sys.stdin.buffer.read(1024 * 1024):
                    size += len(chunk)
                    if size > MAX_PACKAGE + 256:
                        raise ValueError("package-too-large")
                    output.write(chunk)
            # Only the desktop writes this trailer, after the HTTP body finished.
            # A closed pipe during a crash/cancel must not publish a partial ELF.
            with package.open("r+b") as output:
                output.seek(max(0, size - 256))
                tail = output.read()
                trailer = re.search(rb"\nTRON-DOWNLOAD-COMPLETE ([0-9a-f]{64}) ([0-9]+)\n$", tail)
                if not trailer:
                    raise ValueError("wsl-package-incomplete")
                payload_size = size - (len(tail) - trailer.start())
                if payload_size != int(trailer[2]) or not 0 < payload_size <= MAX_PACKAGE:
                    raise ValueError("wsl-package-incomplete")
                output.truncate(payload_size)
                output.seek(0)
                if hashlib.file_digest(output, "sha256").hexdigest().encode() != trailer[1]:
                    raise ValueError("wsl-package-checksum-mismatch")
            unpack(package, stage)
            package.unlink()
            binary = stage / "tron-backend"
            with binary.open("rb") as source:
                header = source.read(20)
            machine = {"x86_64": 62, "aarch64": 183}.get(os.uname().machine)
            if len(header) < 20 or header[:6] != b"\x7fELF\x02\x01" or struct.unpack("<H", header[18:20])[0] != machine:
                raise ValueError("wsl-package-architecture-mismatch")
            with binary.open("rb") as source:
                digest = hashlib.file_digest(source, "sha256").hexdigest()
            sums = stage / "SHA256SUMS"
            if sums.exists():
                expected = re.search(r"^([0-9a-fA-F]{64})\s+\*?(?:\./)?tron-backend\s*$", sums.read_text(), re.M)
                if not expected or expected[1].lower() != digest:
                    raise ValueError("wsl-package-checksum-mismatch")
            binary.chmod(0o700)
            libraries = subprocess.run(["ldd", str(binary)], capture_output=True, text=True, check=True)
            if "not found" in libraries.stdout + libraries.stderr:
                raise ValueError("wsl-runtime-libraries-missing")
            source = Path(frontend)
            if not (source / "index.html").is_file():
                raise ValueError("desktop-frontend-missing")
            shutil.copytree(source, stage / "frontend")
            (stage / "installation.json").write_text(json.dumps({"version": version, "sha256": digest, "architecture": os.uname().machine}))
            # A completed version is immutable. Never replace a running binary.
            if release.exists():
                raise ValueError("wsl-release-already-exists")
            stage.rename(release)
            emit({"version": version, "dataDir": str(root), "releaseDir": str(release), "sha256": digest})
        finally:
            if stage.exists():
                shutil.rmtree(stage)


def inspect(version):
    root, _, release = directories(version)
    installed = (release / "installation.json").is_file() and (release / "tron-backend").is_file() and (release / "frontend/index.html").is_file()
    emit({"installed": installed, "dataDir": str(root), "releaseDir": str(release)})


def uninstall(delete_user_data=False):
    home = linux_path(Path.home())
    root = home / ".tron"
    desktop = root / "desktop"
    releases = desktop / "releases"
    # Refuse redirected installation roots before touching any files.
    for directory in (root, desktop, releases):
        if directory.is_symlink():
            raise ValueError("wsl-uninstall-unsafe-path")
        if directory == root and not directory.exists():
            return
        if directory.exists():
            linux_path(directory)
    desktop.mkdir(mode=0o700, exist_ok=True)
    with (desktop / "install.lock").open("a") as install_lock, (desktop / "backend.lock").open("a") as backend_lock:
        try:
            fcntl.flock(install_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            fcntl.flock(backend_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError("wsl-backend-already-running")
        # Keep the lock files in place so other desktop instances share the
        # same locks. Business data and managed workspaces live outside releases.
        if releases.exists():
            shutil.rmtree(releases)
        if delete_user_data:
            # Leave only the shared lock files. Removing them would let a
            # concurrent desktop instance acquire a different inode's lock.
            for parent, retained in ((root, {"desktop"}), (desktop, {"install.lock", "backend.lock"})):
                for entry in parent.iterdir():
                    if entry.name in retained:
                        continue
                    if entry.is_symlink() or not entry.is_dir():
                        entry.unlink()
                    else:
                        shutil.rmtree(entry)


def serve(version, port, nonce, startup_timeout_seconds):
    root, desktop, release = directories(version)
    if not re.fullmatch(r"[0-9a-f]{32}", nonce) or not 1024 <= port <= 65535:
        raise ValueError("invalid-launch-parameters")
    lock = (desktop / "backend.lock").open("a")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        raise ValueError("wsl-backend-already-running")
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", port))
    for name in ("logs", "control", "workspaces", "cache/sqlite-extensions"):
        (root / name).mkdir(parents=True, exist_ok=True)
        linux_path(root / name)
    config = root / "application.yaml"
    if not config.exists():
        with config.open("x") as output:
            # The backend fills this key in place on first launch. A flow-style
            # empty map followed by its appended block would break the next run.
            output.write('variableCrypto:\n  keyBase64: ""\n')
        config.chmod(0o600)
    config = linux_path(config)
    marker = release / "frontend" / ("tron-runtime-" + nonce + ".txt")
    marker.write_text(nonce)
    environment = {key: value for key, value in os.environ.items() if not key.startswith("TRON_")}
    # WSL appends Windows PATH entries by default; backend tools stay in Linux.
    environment["PATH"] = ":".join(item for item in environment.get("PATH", "").split(":") if item.startswith("/") and not re.match(r"^/mnt/[a-z](?:/|$)", item, re.I))
    environment.update({
        "TRON_BACKEND_DATABASE_URL": str(root / "tron.sqlite3"),
        "TRON_BACKEND_CONTROL_DATABASE_URL": str(root / "control/profiles.sqlite"),
        "TRON_BACKEND_GRAPH_STORAGE_PATH": str(root / "tron.graph.grafeo"),
        "TRON_BACKEND_APPLICATION_CONFIG_PATH": str(config),
        "TRON_BACKEND_FRONTEND_DIST_DIR": str(release / "frontend"),
        "TRON_TERMINAL_WORKING_DIRECTORY": str(root / "workspaces"),
        "TRON_SQLITE_EXTENSION_MODE": "required",
        "TRON_SQLITE_EXTENSION_RUNTIME_DIR": str(root / "cache/sqlite-extensions"),
        "TRON_BACKEND_BIND_ADDRESS": "127.0.0.1",
        "TRON_BACKEND_PORT": str(port),
    })
    powershell = shutil.which("powershell.exe")
    if powershell:
        environment["TRON_COMPUTER_USE_POWERSHELL"] = powershell
    stopping = threading.Event()
    for signum in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(signum, lambda *_: stopping.set())

    def watch_owner():
        # EOF also fires after a Windows Electron crash; no orphaned backend.
        # Use the fd directly: a blocked daemon must not hold Python's buffered
        # stdin lock when the backend fails and the interpreter shuts down.
        while os.read(sys.stdin.fileno(), 1):
            pass
        stopping.set()

    threading.Thread(target=watch_owner, daemon=True).start()
    child = None
    try:
        with (root / "logs/tron-backend.log").open("ab", buffering=0) as log:
            child = subprocess.Popen(["setpriv", "--pdeathsig", "TERM", "--", str(release / "tron-backend")], cwd=root, env=environment, stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
            ready = False
            deadline = time.monotonic() + startup_timeout_seconds
            while child.poll() is None and not stopping.is_set():
                if not ready:
                    try:
                        with opener.open(f"http://127.0.0.1:{port}/api/v1/readiness", timeout=1) as response:
                            payload = json.load(response)["data"]
                        if payload["status"] == "ready" and payload["version"] == version:
                            emit({"port": port, "dataDir": str(root), "releaseDir": str(release), "version": version})
                            ready = True
                    except (OSError, ValueError, KeyError):
                        pass
                    if time.monotonic() > deadline:
                        raise ValueError("wsl-backend-readiness-timeout")
                stopping.wait(0.2)
            if not stopping.is_set():
                raise ValueError("wsl-backend-exited: " + str(child.returncode))
    finally:
        if child:
            try:
                os.killpg(child.pid, signal.SIGTERM)
                child.wait(timeout=8)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            except ProcessLookupError:
                pass
        marker.unlink(missing_ok=True)
        lock.close()


try:
    os.umask(0o077)
    command = sys.argv[1]
    if command == "uninstall":
        option = sys.argv[2] if len(sys.argv) > 2 else "keep-data"
        if option not in ("keep-data", "delete-data"):
            raise ValueError("invalid-uninstall-option")
        uninstall(option == "delete-data")
    elif command == "inspect":
        inspect(sys.argv[2])
    elif command == "install":
        install(sys.argv[2], sys.argv[3])
    elif command == "serve":
        serve(sys.argv[2], int(sys.argv[3]), sys.argv[4], int(sys.argv[5]))
    else:
        raise ValueError("unknown-runtime-command")
except Exception as error:
    print(str(error), file=sys.stderr, flush=True)
    sys.exit(1)
