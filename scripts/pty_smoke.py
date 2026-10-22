import fcntl
import os
import pty
import select
import struct
import sys
import tempfile
import termios
import time

BIN = sys.argv[1]
CTRL_S, CTRL_Q = b"\x13", b"\x11"
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


def drain(fd, idle=0.4):
    out = b""
    while select.select([fd], [], [], idle)[0]:
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

print("\n%d failure(s)" % len(failures))
sys.exit(1 if failures else 0)
