#!/usr/bin/env python3
"""Compare release builds against the bundled echo server on Linux."""

import argparse
import json
import os
from pathlib import Path
import platform
import resource
import select
import signal
import subprocess
import tempfile
import time


MODES = {
    "silent": (),
    "ping-pong": ("--interval", "100ms"),
    "flood": ("--rate", "20"),
}


def run_sample(client, url, mode, connections, seconds, report_path):
    command = [
        client, mode, "--url", url, "-c", str(connections), "-d", f"{seconds}s",
        "--json", report_path, *MODES[mode],
    ]
    started = time.monotonic()
    deadline = started + seconds + 20
    peak_rss = 0
    with tempfile.TemporaryFile() as output:
        with subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT) as process:
            status_path = Path("/proc") / str(process.pid) / "status"
            while True:
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    break
                if time.monotonic() >= deadline:
                    process.kill()
                    process.wait()
                    raise RuntimeError(f"{client} {mode} timed out")
                try:
                    for line in status_path.read_text().splitlines():
                        if line.startswith("VmHWM:"):
                            peak_rss = max(peak_rss, int(line.split()[1]))
                            break
                except (FileNotFoundError, ProcessLookupError):
                    pass
                time.sleep(0.01)
        if process.returncode:
            output.seek(0)
            raise RuntimeError(output.read().decode(errors="replace"))
    if not peak_rss:
        raise RuntimeError(f"could not sample RSS for {client} {mode}")
    report = json.loads(report_path.read_text())
    cpu = usage.ru_utime + usage.ru_stime
    return {
        "version": report["version"],
        "mode": mode,
        "peak_active": report["peak_active"],
        "load_seconds": report["load_duration_seconds"],
        "cpu_seconds": cpu,
        "sampled_peak_rss_kib": peak_rss,
        "messages_sent_per_second": report["send_messages_per_second"],
        "pings_sent_per_second": report["pings_sent"] / report["load_duration_seconds"],
        "missed_send_slots": report["missed_send_slots"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--client", type=Path, action="append",
                        help="Release wsgator binary. Repeat to compare builds.")
    parser.add_argument("--server", type=Path, default=Path("target/release/ws-mini-server"))
    parser.add_argument("--connections", type=int, default=100)
    parser.add_argument("--seconds", type=int, default=4)
    parser.add_argument("--repeats", type=int, default=3)
    args = parser.parse_args()
    if platform.system() != "Linux":
        parser.error("this script uses Linux /proc and wait4")
    if not 1 <= args.connections <= 1_000_000 or min(args.seconds, args.repeats) < 1:
        parser.error("connections, seconds and repeats must be positive, with at most 1000000 connections")
    clients = [p.resolve() for p in (args.client or [Path("target/release/wsgator")])]
    server_path = args.server.resolve()
    samples = []
    with tempfile.TemporaryDirectory() as work, tempfile.TemporaryFile() as server_errors:
        with subprocess.Popen([server_path, "--bind", "127.0.0.1:0"],
                              stdout=subprocess.PIPE, stderr=server_errors, text=True) as server:
            try:
                if not select.select([server.stdout], [], [], 5)[0]:
                    raise RuntimeError("local echo server did not start")
                line = server.stdout.readline().strip()
                if not line.startswith("Echo server listening on ws://127.0.0.1:"):
                    raise RuntimeError(f"unexpected echo server startup message: {line!r}")
                url = line.split()[-1]
                for trial in range(1, args.repeats + 1):
                    for mode in MODES:
                        for client in (clients if trial % 2 else reversed(clients)):
                            sample = run_sample(client, url, mode, args.connections,
                                                args.seconds, Path(work) / "report.json")
                            samples.append(dict(sample, binary=client.name, trial=trial))
            finally:
                if server.poll() is None:
                    server.send_signal(signal.SIGINT)
                    try:
                        server.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
    cpu_model = next((line.split(":", 1)[1].strip()
                      for line in Path("/proc/cpuinfo").read_text().splitlines()
                      if line.startswith("model name")), platform.machine())
    print(json.dumps({
        "kernel": platform.release(),
        "cpu_model": cpu_model,
        "available_cpus": len(os.sched_getaffinity(0)),
        "file_descriptor_limit": resource.getrlimit(resource.RLIMIT_NOFILE)[0],
        "echo_server": subprocess.check_output([server_path, "--version"], text=True).strip(),
        "connections": args.connections,
        "seconds": args.seconds,
        "repeats": args.repeats,
        "behavior_options": MODES,
        "samples": samples,
    }, indent=2))


if __name__ == "__main__":
    main()
