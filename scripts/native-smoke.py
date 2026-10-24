#!/usr/bin/env python3
"""Run the opt-in native smoke binary against an isolated Xvfb display.

Build first: cargo build -p netease-cloud-music-desktop --features native-smoke,custom-protocol
Requires Xvfb and the regular application runtime libraries. No account is used.
"""
import json
import math
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import time
import wave

workspace = Path(__file__).resolve().parents[1]
binary = workspace / "target/debug/netease-cloud-music-desktop"
output = Path(os.environ.get("NCM_TEST_OUTPUT") or tempfile.mkdtemp(prefix="ncm-native-"))
output.mkdir(parents=True, exist_ok=True)
audio = output / "tone.wav"
with wave.open(str(audio), "wb") as file:
    file.setnchannels(1)
    file.setsampwidth(2)
    file.setframerate(8000)
    file.writeframes(b"".join(struct.pack("<h", int(4000 * math.sin(i * 440 * math.tau / 8000))) for i in range(48000)))

env = os.environ.copy()
env.update(NCM_DATA_DIR=str(output / "data"), NCM_AUDIO_SINK="fakesink",
           NCM_NATIVE_SMOKE="1", NCM_NATIVE_SMOKE_AUDIO=str(audio),
           NCM_NATIVE_SMOKE_REPORT=str(output / "result.txt"),
           GDK_BACKEND="x11", WEBKIT_DISABLE_DMABUF_RENDERER="1", RUST_LOG="warn")
read_fd, write_fd = os.pipe()
xlog = (output / "xvfb.log").open("w")
xvfb = subprocess.Popen([shutil.which("Xvfb") or "Xvfb", "-displayfd", str(write_fd), "-screen", "0", "1280x840x24", "-nolisten", "tcp", "-ac"],
                        pass_fds=(write_fd,), stdout=xlog, stderr=xlog, env=env)
os.close(write_fd)
try:
    import select
    if not select.select([read_fd], [], [], 15)[0]:
        raise RuntimeError("Xvfb did not initialize; inspect " + str(output / "xvfb.log"))
    display = os.read(read_fd, 100).decode().strip()
    if not display:
        raise RuntimeError("Xvfb exited without a display: " + (output / "xvfb.log").read_text())
    env["DISPLAY"] = ":" + display
    samples = []
    with (output / "app.log").open("w") as log:
        app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=log)
        started = time.monotonic()
        while app.poll() is None:
            if time.monotonic() - started > 180 + int(env.get('NCM_NATIVE_SOAK_SECONDS', '0')):
                app.terminate()
                raise RuntimeError("Native smoke timed out")
            # Include all descendants, not only the main process, in PSS totals.
            parents = {}
            cpu_ticks = {}
            for proc in Path('/proc').iterdir():
                if not proc.name.isdigit():
                    continue
                try:
                    stat = (proc / 'stat').read_text().rsplit(')', 1)[1].split()
                    parents[int(proc.name)] = int(stat[1])
                    cpu_ticks[int(proc.name)] = int(stat[11]) + int(stat[12])
                except (OSError, ValueError, IndexError):
                    pass
            descendants = {app.pid}
            while True:
                extra = {pid for pid, parent in parents.items() if parent in descendants}
                if extra <= descendants:
                    break
                descendants |= extra
            pss = 0
            for pid in descendants:
                try:
                    for line in Path(f'/proc/{pid}/smaps_rollup').read_text().splitlines():
                        if line.startswith('Pss:'):
                            pss += int(line.split()[1])
                except OSError:
                    pass
            samples.append({"seconds": round(time.monotonic() - started, 2), "processes": len(descendants), "pss_kb": pss,
                            "cpu_seconds": sum(cpu_ticks.get(pid, 0) for pid in descendants) / os.sysconf('SC_CLK_TCK')})
            if len(samples) % 20 == 0: (output / "memory.json").write_text(json.dumps(samples, indent=2))
            time.sleep(5 if time.monotonic() - started > 120 else .5)
        code = app.wait()
    (output / "memory.json").write_text(json.dumps(samples, indent=2))
    print((output / "app.log").read_text())
    print("Artifacts:", output)
    result = output / "result.txt"
    if code != 0 or not result.exists() or result.read_text() != "PASS":
        raise SystemExit(1)
finally:
    if 'app' in locals() and app.poll() is None:
        app.terminate()
        app.wait(timeout=10)
    os.close(read_fd)
    xvfb.terminate()
    xvfb.wait(timeout=10)
