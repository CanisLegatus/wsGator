#!/usr/bin/env python3
"""Exercise the real wsGator terminal UI against the bundled echo server.

Unix only. Requires pyte; Pillow is optional for --screenshots.
Build first: cargo build --workspace --locked
Run: python3 scripts/tui_smoke.py [--screenshots /tmp/wsgator-screens]
"""
import argparse
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time

import pyte

ROOT = Path(__file__).resolve().parent.parent


class Terminal:
    def __init__(self, directory, url, binaries):
        self.master, self.slave = pty.openpty()
        self.original = termios.tcgetattr(self.slave)
        self.screen = pyte.Screen(128, 42)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.raw = bytearray()
        self.resize(128, 42)
        env = dict(os.environ, TERM="xterm-256color", COLORTERM="truecolor",
                   XDG_CONFIG_HOME=str(directory / "profile"))
        env.pop("NO_COLOR", None)
        self.process = subprocess.Popen(
            [str(binaries / "wsgator")],
            cwd=directory, env=env, stdin=self.slave, stdout=self.slave,
            stderr=self.slave, start_new_session=True,
            preexec_fn=lambda: fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0),
        )
        self.wait("MISSION PARAMETERS")
        self.send("\r\x15" + url + "\r")
        self.send("\t\r\x1512\r\t\r\x153s\r")

    @property
    def text(self):
        return "\n".join(self.screen.display)

    def resize(self, width, height):
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
        self.screen.resize(height, width)

    def pump(self, seconds=0.1):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([self.master], [], [], max(0, deadline - time.monotonic()))[0]:
                data = os.read(self.master, 65536)
                if not data:
                    break
                self.raw.extend(data)
                self.stream.feed(self.decoder.decode(data))

    def send(self, text):
        os.write(self.master, text.encode())
        self.pump(0.12)

    def wait(self, text, timeout=7):
        deadline = time.monotonic() + timeout
        while text not in self.text:
            if self.process.poll() is not None:
                raise AssertionError(f"TUI exited early ({self.process.returncode}):\n{self.text}")
            if time.monotonic() > deadline:
                raise AssertionError(f"Missing {text!r}:\n{self.text}")
            self.pump()

    def screenshot(self, directory, name):
        if directory is None:
            return
        from PIL import Image, ImageDraw, ImageFont
        directory.mkdir(parents=True, exist_ok=True)
        # Render the captured terminal cells, including the actual application colors.
        font_file = subprocess.check_output(["fc-match", "-f", "%{file}", "IosevkaTerm NFM"], text=True)
        bold_file = subprocess.check_output(["fc-match", "-f", "%{file}", "IosevkaTerm NFM:style=Bold"], text=True)
        font, bold = ImageFont.truetype(font_file, 16), ImageFont.truetype(bold_file, 16)
        cell_w, cell_h, pad = 10, 22, 24
        image = Image.new("RGB", (self.screen.columns*cell_w+2*pad, self.screen.lines*cell_h+2*pad), "#080d16")
        draw = ImageDraw.Draw(image)
        colors = dict(default="d5e5f0", black="000000", red="cd0000", green="00cd00", brown="cdcd00", blue="0000ee", magenta="cd00cd", cyan="00cdcd", white="e5e5e5", brightblack="7f7f7f", brightred="ff0000", brightgreen="00ff00", brightbrown="ffff00", brightblue="5c5cff", brightmagenta="ff00ff", brightcyan="00ffff", brightwhite="ffffff")
        for row in range(self.screen.lines):
            for col in range(self.screen.columns):
                cell = self.screen.buffer[row][col]
                fg = colors.get(cell.fg, cell.fg)
                bg = "080d16" if cell.bg == "default" else colors.get(cell.bg, cell.bg)
                if cell.reverse:
                    fg, bg = bg, fg
                x, y = pad+col*cell_w, pad+row*cell_h
                draw.rectangle((x,y,x+cell_w,y+cell_h), fill="#"+bg)
                if cell.data:
                    draw.text((x,y+1), cell.data, font=bold if cell.bold else font, fill="#"+fg)
        image.save(directory / f"{name}.png")
        (directory / f"{name}.txt").write_text(self.text)

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=5)
        os.close(self.master)
        os.close(self.slave)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--screenshots", type=Path)
    parser.add_argument("--release", action="store_true", help="Use release binaries")
    args = parser.parse_args()
    binaries = ROOT / "target" / ("release" if args.release else "debug")
    server = subprocess.Popen([str(binaries / "ws-mini-server"), "--bind", "127.0.0.1:0"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    terminal = None
    try:
        assert select.select([server.stdout], [], [], 5)[0], "echo server did not start"
        url = server.stdout.readline().strip().split()[-1]
        with tempfile.TemporaryDirectory(prefix="wsgator-tui-") as temporary:
            directory = Path(temporary)
            terminal = Terminal(directory, url, binaries)
            terminal.send("\x1b[C")  # Ping-pong -> Flood, reset focus to target.
            terminal.send("\t\t\t\r\x1540\r")
            terminal.send("\t\t\t\t\r\x15ramp:750ms\r")
            terminal.wait("ramp:750ms")
            terminal.screenshot(args.screenshots, "configure")
            terminal.send("\x1b[15~")  # F5
            terminal.wait("12 established")
            terminal.wait("hold · 12 target")
            terminal.pump(1.0)
            terminal.screenshot(args.screenshots, "telemetry")
            terminal.wait("Results retained")
            assert terminal.process.poll() is None, "TUI should stay open after completion"
            terminal.screenshot(args.screenshots, "results")
            terminal.send("e")
            terminal.wait("Saved wsgator-report-")
            report = json.loads(next(directory.glob("wsgator-report-*.json")).read_text())
            assert report["behavior"] == "flood-text", report
            assert report["connected"] == 12 and report["active_at_end"] == 0, report
            assert report["messages_sent"] > 0 and not report["errors"], report
            assert [p["kind"] for p in report["phases"]] == ["ramp", "hold", "drain"], report
            assert report["phases"][1]["active_at_start"] == 12, report
            terminal.send("3")
            terminal.wait("SESSION LOGBOOK")
            terminal.screenshot(args.screenshots, "sessions")
            terminal.send("\r")
            terminal.wait("ARCHIVED")
            terminal.resize(80, 24)
            terminal.pump(0.4)
            assert "wsGator" in terminal.text
            terminal.resize(128, 42)
            terminal.pump(0.4)
            # Rerun the archived mission, stop it, and keep the application open.
            terminal.send("\x1b[15~")
            terminal.wait("12 established")
            terminal.send("x")
            terminal.wait("Results retained")
            assert "STOPPED" in terminal.text
            terminal.send("?")
            terminal.wait("MISSION CONTROL / KEYMAP")
            terminal.send("\x1b")
            terminal.send("d")
            terminal.wait("32 established")
            terminal.wait("LOCAL DEMO")
            terminal.pump(1.0)
            terminal.screenshot(args.screenshots, "demo")
            # A real OS signal (not just a raw-mode key) must also drain safely.
            terminal.process.send_signal(signal.SIGINT)
            assert terminal.process.wait(timeout=5) == 0
            terminal.pump()
            restored = termios.tcgetattr(terminal.slave)
            for flag in (termios.ECHO, termios.ICANON):
                assert restored[3] & flag == terminal.original[3] & flag, "terminal mode was not restored"
            assert b"\x1b[?1049l" in terminal.raw, "alternate screen was not restored"
            profile = directory / "profile/wsgator/profile.json"
            assert profile.exists(), "settings profile was not saved"
            settings = json.loads(profile.read_text())
            assert settings["connections"] == "12", "demo overwrote the user's profile"
            assert settings["load_profile"] == "ramp:750ms", settings
            print("TUI smoke passed: default launch, editing, live telemetry, completion, JSON export, history, rerun, stop, resize, help, local demo, OS interrupt, terminal restoration.")
    finally:
        if terminal is not None:
            terminal.close()
        server.send_signal(signal.SIGINT)
        _, errors = server.communicate(timeout=5)
        assert server.returncode == 0, errors


if __name__ == "__main__":
    main()
