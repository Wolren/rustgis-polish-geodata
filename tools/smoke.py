"""Runs every command of the installed plugin in a private session and reports what fails.

`tools/probe.py` checks the addresses; this checks the commands: the manifest, the dispatch, the
host calls. It drives `rustgis-cli mcp` over stdio, so it needs the app and the plugin installed,
and it changes nothing outside its own session.

    python tools/smoke.py                  every command that answers without a download
    python tools/smoke.py --slow           also the three elevation models (a few MB each)
    python tools/smoke.py --only wody      only commands whose id contains "wody"

A command that reports an error, or that the host does not know, is a failure: the exit code is 1.
Tile layers are cleared between commands, because a map holds at most 32 of them and the
catalogue has 139.
"""

import argparse
import io
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIXTURES = os.path.join(ROOT, "tools", "fixtures")
AREA = os.path.join(FIXTURES, "area.geojson")
PARCELS = os.path.join(FIXTURES, "parcels.geojson")

# The commands that cannot run without an argument, and the argument that proves them.
PARAMS = {
    "eziudp": {"teryt": "1465"},
    # The geocoder wants the city first: "Marszałkowska 1, Warszawa" finds nothing.
    "geokoduj": {"adres": "Warszawa, Marszałkowska 1"},
    "dzialka-id": {"id": "146510_8.0303.50"},
    "katalog": {"szukaj": "ortofoto"},
    "warstwy": {"szukaj": "powódź"},
    "wody-mzp-glebokosc-1": {"warstwa": "obszar szczególnego zagrożenia powodzią"},
    "przytnij": {"obszar": "area", "layer": "parcels"},
    "dem": {"area": "area"},
    "dem-evr": {"area": "area"},
    "dem-nmpt": {"area": "area"},
}
# What a command needs on the map before it runs: a street-level view for the county WFS.
SETUP = {
    "dzialki-z-widoku": [("map.setScale", {"scale": 5000})],
}
# The commands that download a model: minutes and megabytes, so they are opt-in.
SLOW = {"dem", "dem-evr", "dem-nmpt", "dem-style"}


class Session:
    def __init__(self, data):
        command = ["rustgis-cli", "mcp", "--yes"]
        for path in data:
            command += ["--data", path]
        self.proc = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            encoding="utf-8",
            errors="replace",
            bufsize=1,
        )
        self.counter = 0
        self.send({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                   "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                              "clientInfo": {"name": "polish-geodata smoke", "version": "0"}}})
        self.read()
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def send(self, message):
        self.proc.stdin.write(json.dumps(message) + "\n")
        self.proc.stdin.flush()

    def read(self):
        while True:
            line = self.proc.stdout.readline()
            if not line:
                return None
            line = line.strip()
            if line.startswith("{"):
                return json.loads(line)

    def call(self, name, arguments):
        self.counter += 1
        ident = 1000 + self.counter
        self.send({"jsonrpc": "2.0", "id": ident, "method": "tools/call",
                   "params": {"name": name, "arguments": arguments}})
        while True:
            answer = self.read()
            if answer and answer.get("id") == ident:
                return answer.get("result", {"error": answer.get("error")})

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        self.proc.terminate()


def text_of(result):
    for item in result.get("content") or []:
        if item.get("type") == "text":
            return item["text"]
    return ""


def run(session, command, params=None):
    arguments = {"command": "plugin:polish-geodata/" + command}
    if params is not None:
        arguments["params"] = params
    return text_of(session.call("run_command", arguments))


def clear_tiles(session):
    """A map holds 32 tile layers; the catalogue has more, so each command starts clean."""
    for _ in range(64):
        answer = text_of(session.call("run_command", {"command": "map.tiles"}))
        try:
            tiles = json.loads(answer)
        except Exception:
            return
        if not isinstance(tiles, list) or not tiles:
            return
        # Removing index 0 removes the topmost layer and renumbers the rest.
        session.call("run_command", {"command": "map.removeTiles",
                                     "params": {"index": 0}})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--slow", action="store_true", help="also the elevation models")
    parser.add_argument("--only", default="", help="run only ids containing this")
    args = parser.parse_args()

    manifest = json.load(io.open(os.path.join(ROOT, "plugin.json"), encoding="utf-8"))
    commands = [command["id"] for command in manifest["commands"]]

    session = Session([AREA, PARCELS])

    failed, skipped, ran = [], [], 0
    for command in commands:
        if args.only and args.only not in command:
            continue
        if command in SLOW and not args.slow:
            skipped.append(command)
            continue
        for setup, setup_params in SETUP.get(command, []):
            session.call("run_command", {"command": setup, "params": setup_params})
        answer = run(session, command, PARAMS.get(command))
        ran += 1
        # A plugin answer is JSON; the host reports a refusal as a plain sentence.
        bad = not answer.startswith("{") or '"error"' in answer[:40]
        print(f"  {'FAIL' if bad else 'ok  '} {command[:58]:60} {answer[:70].replace(chr(10), ' ')}")
        if bad:
            failed.append((command, answer[:200]))
        clear_tiles(session)
    session.close()

    print(f"\n{ran} commands run, {len(failed)} failed, {len(skipped)} skipped (--slow)")
    for command, answer in failed:
        print(f"  {command}\n    {answer}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
