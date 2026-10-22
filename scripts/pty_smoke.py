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
pid = fd = None
try:
    os.mkdir(os.path.join(tree, "sub"))
    for name, text in (("sub/inner.txt", "inner\n"), ("top.txt", "hello top\n"), (".hidden", "dot\n")):
        with open(os.path.join(tree, name), "w") as f:
            f.write(text)
    pid, fd = spawn([tree])
    drain(fd)
    send(fd, CTRL_T)
    out = drain(fd, 0.5, 2.0)
    check("tree shows status row", b"Files:" in out, out[-300:])
    check("tree lists top.txt", b"top.txt" in out, out[-300:])
    check("tree lists collapsed sub/", b"> sub/" in out, out[-300:])
    check("tree shows dotfile", b".hidden" in out, out[-300:])
    check("tree hides collapsed contents", b"top.txt" in out and b"inner.txt" not in out, out[-300:])
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
    if fd is not None:
        try:
            os.close(fd)
        except OSError:
            pass
    shutil.rmtree(tree, ignore_errors=True)

# 11. syntax highlighting: a .rs file is drawn with ANSI colors right before
# the keyword, string and comment; a .txt file with identical text has none.
# crossterm emits the 256-color form: magenta \x1b[38;5;13m, green
# \x1b[38;5;10m; comments are dim (\x1b[2m) in the normal foreground.
hl = tempfile.mkdtemp(prefix="ted-smoke-hl-")
SRC = 'fn main() { let s = "hi"; } // note\n'


def open_and_quit(path):
    pid, fd = spawn([path])
    try:
        out = drain(fd)
        send(fd, CTRL_Q)
        drain(fd)
        return out, wait_exit(pid) == 0
    finally:
        try:
            os.kill(pid, 9)
        except OSError:
            pass
        try:
            os.waitpid(pid, 0)
        except OSError:
            pass
        os.close(fd)


try:
    rs = os.path.join(hl, "sample.rs")
    txt = os.path.join(hl, "sample.txt")
    for p in (rs, txt):
        with open(p, "w") as f:
            f.write(SRC)
    out, ok = open_and_quit(rs)
    check("rs: keyword is magenta", b"\x1b[38;5;13mfn" in out, out[:300])
    check("rs: string is green", b'\x1b[38;5;10m"hi"' in out, out[:300])
    check("rs: comment is dim", b"\x1b[2m// note" in out, out[:300])
    check("rs session exits 0", ok)
    out, ok = open_and_quit(txt)
    check("txt: text is shown", b'fn main() { let s = "hi"; } // note' in out, out[:300])
    check("txt: no syntax colors", b"\x1b[38;5;" not in out, out[:300])
    check("txt: nothing dim", b"\x1b[2m" not in out, out[:300])
    check("txt session exits 0", ok)
finally:
    shutil.rmtree(hl, ignore_errors=True)

# 12. escape sequences in the file are shown as text, never sent to the
# terminal: ESC becomes `?`. A .txt file gets no styling of its own, so any
# \x1b[31m or \x1b[2J in the output came from the file.
esc = tempfile.mkdtemp(prefix="ted-smoke-esc-")
try:
    p = os.path.join(esc, "esc.txt")
    with open(p, "wb") as f:
        f.write(b"\x1b[31mRED \x1b[2J END\n")
    out, ok = open_and_quit(p)
    # ted clears the alternate screen once on entry; any other \x1b[2J is the file's.
    after_entry = out.replace(b"\x1b[?1049h\x1b[2J", b"", 1)
    check("esc: ted's own entry clear seen once", out.count(b"\x1b[?1049h\x1b[2J") == 1, out[:300])
    check("esc: clear-screen from file not sent", b"\x1b[2J" not in after_entry, out[:300])
    check("esc: color from file not sent", b"\x1b[31m" not in out, out[:300])
    check("esc: shown as ? text", b"?[31mRED" in out and b"END" in out, out[:300])
    check("esc session exits 0", ok)
finally:
    shutil.rmtree(esc, ignore_errors=True)

# 13. several buffers: open a.txt and b.txt through the tree, edit a.txt, list
# the buffers with Ctrl-O, close warning (Ctrl-W), quit warning (Ctrl-Q).
CTRL_O, CTRL_P, CTRL_W = b"\x0f", b"\x10", b"\x17"
bufs = tempfile.mkdtemp(prefix="ted-smoke-bufs-")
pid = fd = None
try:
    for name in ("a.txt", "b.txt"):
        with open(os.path.join(bufs, name), "w") as f:
            f.write(name + " body\n")
    pid, fd = spawn([bufs])
    drain(fd)
    send(fd, CTRL_T)
    drain(fd)
    send(fd, b"\r")  # a.txt (first row)
    drain(fd)
    send(fd, CTRL_T)
    drain(fd)
    send(fd, b"\x1b[B")
    send(fd, b"\r")  # b.txt
    out = drain(fd, 0.5, 2.0)
    check("buffers: second file is open", b"b.txt body" in out, out[-300:])
    check("buffers: status shows [2/2]", b"[2/2]" in out, out[-300:])
    send(fd, CTRL_P)
    drain(fd)
    send(fd, b"X")
    out = drain(fd, 0.5, 2.0)
    check("buffers: edit in first shows [1/2] and [+]", b"[1/2]" in out and b"[+]" in out, out[-300:])
    send(fd, CTRL_O)
    out = drain(fd, 0.5, 2.0)
    check("buffers: Ctrl-O shows the list", b"Buffers: 2 buffers" in out, out[-300:])
    check("buffers: list has both names", b"a.txt [+]" in out and b"b.txt" in out, out[-300:])
    check("buffers: current is marked", b"> 1  a.txt" in out, out[-300:])
    send(fd, b"\r")
    out = drain(fd, 0.5, 2.0)
    check("buffers: Enter returns to the editor", b"Buffers:" not in out and b"Xa.txt body" in out, out[-300:])
    send(fd, CTRL_W)
    out = drain(fd, 0.5, 2.0)
    check("buffers: Ctrl-W warns on the modified buffer", b"Press Ctrl-W again" in out, out[-300:])
    send(fd, CTRL_Q)
    out = drain(fd, 0.5, 2.0)
    check("buffers: Ctrl-Q warns with 1 buffer", b"Unsaved changes in 1 buffer!" in out, out[-300:])
    send(fd, CTRL_Q)
    drain(fd)
    code = wait_exit(pid)
    check("buffers: second Ctrl-Q exits 0", code == 0, code)
    if code is not None:
        pid = None
finally:
    if pid is not None:
        try:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        except OSError:
            pass
    if fd is not None:
        try:
            os.close(fd)
        except OSError:
            pass
    shutil.rmtree(bufs, ignore_errors=True)

# 14. PageDown / PageUp move the editor cursor a screenful at a time.
pg = tempfile.mkdtemp(prefix="ted-smoke-page-")
pid = fd = None
try:
    p = os.path.join(pg, "long.txt")
    with open(p, "w") as f:
        f.write("".join("line %d\n" % i for i in range(200)))
    pid, fd = spawn([p])
    drain(fd)
    send(fd, b"\x1b[6~")
    send(fd, b"\x1b[6~")
    out = drain(fd, 0.5, 2.0)
    check("page: two PageDowns scroll to line 40", b"line 40" in out, out[-300:])
    for _ in range(4):
        send(fd, b"\x1b[5~")
    out = drain(fd, 0.5, 2.0)
    check("page: PageUp goes back to the top", b"line 0" in out, out[-300:])
    send(fd, CTRL_Q)
    drain(fd)
    code = wait_exit(pid)
    check("page: session exits 0", code == 0, code)
    if code is not None:
        pid = None
finally:
    if pid is not None:
        try:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        except OSError:
            pass
    if fd is not None:
        try:
            os.close(fd)
        except OSError:
            pass
    shutil.rmtree(pg, ignore_errors=True)

# 15. F1 opens the help screen, `q` returns to the editor text.
F1 = b"\x1bOP"
hp = tempfile.mkdtemp(prefix="ted-smoke-help-")
pid = fd = None
try:
    p = os.path.join(hp, "h.txt")
    with open(p, "w") as f:
        f.write("hello help body\n")
    pid, fd = spawn([p])
    out = drain(fd)
    check("help: initial hint shown", b"F1: help" in out, out[-300:])
    send(fd, F1)
    out = drain(fd, 0.5, 2.0)
    check("help: F1 shows the Help screen", b"Help" in out and b"(q to close)" in out, out[-300:])
    check("help: lists Ctrl-S and Ctrl-Q", b"Ctrl-S" in out and b"Ctrl-Q" in out, out[-300:])
    send(fd, b"q")
    out = drain(fd, 0.5, 2.0)
    check("help: q returns to the editor text", b"hello help body" in out and b"(q to close)" not in out, out[-300:])
    send(fd, CTRL_Q)
    drain(fd)
    code = wait_exit(pid)
    check("help: session exits 0", code == 0, code)
    if code is not None:
        pid = None
finally:
    if pid is not None:
        try:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        except OSError:
            pass
    if fd is not None:
        try:
            os.close(fd)
        except OSError:
            pass
    shutil.rmtree(hp, ignore_errors=True)

# 16. find and go to line: Ctrl-F, Ctrl-L, F3 (ESC O R), prompt in the message row.
CTRL_F, CTRL_L, F3, ESC = b"\x06", b"\x0c", b"\x1bOR", b"\x1b"
sp = tempfile.mkdtemp(prefix="ted-smoke-search-")
pid = fd = None
try:
    p = os.path.join(sp, "s.txt")
    with open(p, "w") as f:
        f.write("".join(f"line {n}\n" for n in range(1, 201)))
    pid, fd = spawn([p])
    out = drain(fd)
    check("search: line 150 is not on the first screen", b"line 150" not in out, out[-200:])
    send(fd, CTRL_F)
    out = drain(fd)
    check("search: Ctrl-F opens the Find prompt", b"Find:" in out, out[-200:])
    send(fd, b"line 150")
    drain(fd)
    send(fd, b"\r")
    out = drain(fd)
    check("search: Enter shows line 150 (status 150:1)", b"line 150" in out and b" 150:1" in out and b"Find:" not in out and b"[+]" not in out, out[-300:])
    send(fd, CTRL_L)
    out = drain(fd)
    check("search: Ctrl-L opens the Go to line prompt", b"Go to line:" in out, out[-200:])
    send(fd, b"10")
    drain(fd)
    send(fd, b"\r")
    out = drain(fd)
    check("search: go to 10 scrolls back to line 10 (status 10:1)", b"line 10" in out and b" 10:1" in out and b"line 150" not in out and b"[+]" not in out, out[-300:])
    # The remembered term ("line 150") is prefilled and typed text is appended
    # to it, so clear it with Backspace first.
    BS = b"\x7f"
    send(fd, CTRL_F)
    out = drain(fd)
    check("search: second Ctrl-F prefills the last term", b"Find: line 150" in out, out[-200:])
    send(fd, BS * len("line 150"))
    out = drain(fd)
    check("search: Backspace clears the prefill", b"Find: " in out and b"line 150" not in out.split(b"Find: ")[-1], out[-200:])
    send(fd, b"zzz")
    out = drain(fd)
    check("search: no match says not found", b"Find: zzz" in out and b"not found" in out, out[-200:])
    send(fd, ESC)
    out = drain(fd, 0.6, 2.0)
    check("search: Esc closes the prompt and the cursor stays at 10:1", b"Find:" not in out and b"not found" not in out and b" 10:1" in out, out[-200:])
    send(fd, CTRL_F)
    drain(fd)
    send(fd, BS * len("line 150"))
    drain(fd)
    send(fd, b"line 15")
    drain(fd)
    send(fd, b"\r")
    out = drain(fd)
    check("search: Enter accepts line 15 (status 15:1)", b" 15:1" in out and b"Find:" not in out, out[-200:])
    send(fd, F3)
    out = drain(fd)
    check("search: F3 jumps to the next match (status 150:1)", b" 150:1" in out and b"[+]" not in out, out[-200:])
    send(fd, CTRL_Q)
    drain(fd)
    code = wait_exit(pid)
    check("search: session exits 0", code == 0, code)
    if code is not None:
        pid = None
finally:
    if pid is not None:
        try:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        except OSError:
            pass
    if fd is not None:
        try:
            os.close(fd)
        except OSError:
            pass
    shutil.rmtree(sp, ignore_errors=True)

shutil.rmtree(tmp, ignore_errors=True)

print("\n%d failure(s)" % len(failures))
sys.exit(1 if failures else 0)
