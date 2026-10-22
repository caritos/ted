import fcntl
import os
import pty
import select
import shutil
import struct
import subprocess
import sys
import tempfile
import termios
import time

BIN = sys.argv[1]
CTRL_S, CTRL_Q, CTRL_G = b"\x13", b"\x11", b"\x07"
failures = []


def check(name, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else f"  -> {detail}"))
    if not ok:
        failures.append(name)


def spawn(args):
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(BIN, [BIN] + args)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    return pid, fd


def drain(fd, idle=0.4, limit=3.0):
    # Stops after `idle` seconds of quiet, or after `limit` seconds in total:
    # the git view redraws every second (ages tick), so it may never be quiet.
    out = b""
    end = time.time() + limit
    while time.time() < end and select.select([fd], [], [], idle)[0]:
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            break
        if not chunk:
            break
        out += chunk
    return out


def wait_exit(pid, timeout=3):
    end = time.time() + timeout
    while time.time() < end:
        done, status = os.waitpid(pid, os.WNOHANG)
        if done:
            return os.waitstatus_to_exitcode(status)
        time.sleep(0.05)
    return None


def send(fd, data):
    os.write(fd, data)
    time.sleep(0.1)


tmp = tempfile.mkdtemp(prefix="ted-smoke-")

# 1. type, save, quit
path = os.path.join(tmp, "new.txt")
pid, fd = spawn([path])
drain(fd)
send(fd, b"hello")
send(fd, CTRL_S)
out = drain(fd)
check("save shows message", b"Saved" in out, out[-200:])
send(fd, CTRL_Q)
tail = drain(fd)
code = wait_exit(pid)
check("clean quit exits 0", code == 0, code)
check("file saved to disk", open(path).read() == "hello" if os.path.exists(path) else False)
check("screen restored on exit", b"\x1b[?1049l" in tail and b"\x1b[?25h" in tail, tail[-120:])

# 2. unsaved quit needs confirmation
path2 = os.path.join(tmp, "unsaved.txt")
pid, fd = spawn([path2])
drain(fd)
send(fd, b"x")
send(fd, CTRL_Q)
out = drain(fd)
check("unsaved quit warns", b"Unsaved" in out, out[-200:])
check("still running after warning", wait_exit(pid, 0.3) is None)
send(fd, CTRL_Q)
drain(fd)
check("second Ctrl-Q exits 0", wait_exit(pid) == 0)
check("discarded file never created", not os.path.exists(path2))

# 3. existing file is displayed
path3 = os.path.join(tmp, "existing.txt")
open(path3, "w").write("first line\nsecond line\n")
pid, fd = spawn([path3])
out = drain(fd)
check("shows file contents", b"first line" in out and b"second line" in out, out[-300:])
check("shows status bar name", b"existing.txt" in out)
send(fd, CTRL_Q)
drain(fd)
check("quits cleanly", wait_exit(pid) == 0)

# 4. editing an existing file preserves the rest and the trailing newline
pid, fd = spawn([path3])
drain(fd)
send(fd, b"X")
send(fd, CTRL_S)
drain(fd)
send(fd, CTRL_Q)
drain(fd)
wait_exit(pid)
check("edit round-trips", open(path3).read() == "Xfirst line\nsecond line\n", open(path3).read())

# 5. non-UTF-8 file is refused before touching the terminal
path5 = os.path.join(tmp, "binary.bin")
open(path5, "wb").write(b"\xff\xfe\x00\xc0")
pid, fd = spawn([path5])
out = drain(fd)
code = wait_exit(pid)
check("non-UTF-8 exits 1", code == 1, code)
check("non-UTF-8 never enters alt screen", b"\x1b[?1049h" not in out, out)
check("non-UTF-8 explains why", b"ted:" in out, out)
check("binary file untouched", open(path5, "rb").read() == b"\xff\xfe\x00\xc0")

# 6. bad usage
pid, fd = spawn(["a", "b"])
out = drain(fd)
check("usage error exits 2", wait_exit(pid) == 2 and b"usage" in out, out)

# 7. git view in a repo: changed + untracked files, diff lines, back to editor
# Hermetic: ignore the user's and system git config (git helper and ted child inherit this).
os.environ["GIT_CONFIG_GLOBAL"] = "/dev/null"
os.environ["GIT_CONFIG_NOSYSTEM"] = "1"
repo = tempfile.mkdtemp(prefix="ted-smoke-repo-")


def git(*args):
    subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True)


git("init", "-q")
git("config", "user.name", "Smoke Test")
git("config", "user.email", "smoke@example.com")
git("config", "commit.gpgsign", "false")
open(os.path.join(repo, "tracked.txt"), "w").write("old line\n")
git("add", "tracked.txt")
git("commit", "-q", "-m", "initial")
open(os.path.join(repo, "tracked.txt"), "w").write("old line\nadded line\n")
open(os.path.join(repo, "fresh.txt"), "w").write("brand new\n")
pid, fd = spawn([repo])
drain(fd)
send(fd, CTRL_G)
out = drain(fd, 1.0)
check("git view lists tracked file", b"tracked.txt" in out, out[-300:])
check("git view lists untracked file", b"fresh.txt" in out, out[-300:])
check("git view shows a + diff line", b"+added line" in out or b"+brand new" in out, out[-300:])
send(fd, b"q")
drain(fd)
send(fd, b"z")
out = drain(fd)
check("q returns to editor", b"[+]" in out, out[-200:])
send(fd, CTRL_Q)
drain(fd)
check("unsaved quit still warns after git view", wait_exit(pid, 0.3) is None)
send(fd, CTRL_Q)
drain(fd)
check("git view session exits 0", wait_exit(pid) == 0)
shutil.rmtree(repo, ignore_errors=True)

# 8. git view outside a repo reports it and recovers
plain = tempfile.mkdtemp(prefix="ted-smoke-norepo-")
pid, fd = spawn([plain])
drain(fd)
send(fd, CTRL_G)
out = drain(fd, 1.0)
check("non-repo says not a git repository", b"not a git repository" in out, out[-300:])
send(fd, b"q")
drain(fd)
send(fd, b"z")
out = drain(fd)
check("non-repo q returns to editor", b"[+]" in out, out[-200:])
send(fd, CTRL_Q)
drain(fd)
send(fd, CTRL_Q)
drain(fd)
check("non-repo session exits 0", wait_exit(pid) == 0)
shutil.rmtree(plain, ignore_errors=True)

# 9. a refresh slower than the tick never blocks keys: `git` first on PATH
# sleeps 1.1 s per call, longer than the 1 s tick (outside a repo a refresh
# is one git call).
slow = tempfile.mkdtemp(prefix="ted-smoke-slowgit-")
slow_bin = os.path.join(slow, "bin")
os.mkdir(slow_bin)
wrapper = os.path.join(slow_bin, "git")
open(wrapper, "w").write(f'#!/bin/sh\nsleep 1.1\nexec "{shutil.which("git")}" "$@"\n')
os.chmod(wrapper, 0o755)
saved_path = os.environ["PATH"]
os.environ["PATH"] = slow_bin + os.pathsep + saved_path
pid, fd = spawn([slow])
os.environ["PATH"] = saved_path
drain(fd)
send(fd, CTRL_G)
drain(fd, 0.4, 2.5)
start = time.time()
send(fd, b"q")
send(fd, CTRL_Q)
code = None
while code is None and time.time() - start < 5:
    drain(fd, 0.1, 0.3)
    code = wait_exit(pid, 0.1)
check("slow git: q then Ctrl-Q exits promptly", code == 0, f"code {code} after {time.time() - start:.1f}s")
if code is None:
    os.kill(pid, 9)
    os.waitpid(pid, 0)
shutil.rmtree(slow, ignore_errors=True)
# 10. file tree: Ctrl-T lists the start directory, Right expands, Enter opens a
# file, Ctrl-T toggles back to the editor.
CTRL_T = b"\x14"
tree = tempfile.mkdtemp(prefix="ted-smoke-tree-")
pid = None
try:
    os.mkdir(os.path.join(tree, "sub"))
    open(os.path.join(tree, "sub", "inner.txt"), "w").write("inner\n")
    open(os.path.join(tree, "top.txt"), "w").write("hello top\n")
    open(os.path.join(tree, ".hidden"), "w").write("dot\n")
    pid, fd = spawn([tree])
    drain(fd)
    send(fd, CTRL_T)
    out = drain(fd, 0.5, 2.0)
    check("tree shows status row", b"Files:" in out, out[-300:])
    check("tree lists top.txt", b"top.txt" in out, out[-300:])
    check("tree lists collapsed sub/", b"> sub/" in out, out[-300:])
    check("tree shows dotfile", b".hidden" in out, out[-300:])
    check("tree hides collapsed contents", b"inner.txt" not in out, out[-300:])
    send(fd, b"\x1b[B")  # Down, then Up: selection starts on sub/ either way
    send(fd, b"\x1b[A")
    send(fd, b"\x1b[C")  # Right expands sub/
    out = drain(fd, 0.5, 2.0)
    check("Right expands sub/", b"v sub/" in out, out[-300:])
    check("expanded child is indented", b"    inner.txt" in out, out[-300:])
    for _ in range(3):  # inner.txt, .hidden, top.txt
        send(fd, b"\x1b[B")
    drain(fd)
    send(fd, b"\r")
    out = drain(fd, 0.5, 2.0)
    check("Enter opens file in editor", b"hello top" in out and b"Files:" not in out, out[-300:])
    check("status shows opened file name", b"top.txt" in out, out[-300:])
    send(fd, CTRL_T)
    out = drain(fd, 0.5, 2.0)
    check("Ctrl-T shows tree again", b"Files:" in out, out[-300:])
    send(fd, CTRL_T)
    out = drain(fd, 0.5, 2.0)
    check("Ctrl-T in tree returns to editor", b"hello top" in out and b"Files:" not in out, out[-300:])
    send(fd, CTRL_Q)
    drain(fd)
    code = wait_exit(pid)
    check("tree session exits 0", code == 0, code)
    if code is not None:
        pid = None  # already reaped
finally:
    if pid is not None:
        try:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        except OSError:
            pass
    shutil.rmtree(tree, ignore_errors=True)

shutil.rmtree(tmp, ignore_errors=True)

print("\n%d failure(s)" % len(failures))
sys.exit(1 if failures else 0)
