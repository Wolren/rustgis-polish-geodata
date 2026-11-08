"""Fuzz the download paths against the live services: realistic areas, hostile arguments.

Each case runs `rustgis-cli run --sample` with one or two commands and records the wall time, the
exit code and the last status line. A case is OK, REFUSED (a message the plugin meant to give),
TIMEOUT, or CRASH (a non-zero exit with no plugin message). The report names the cases worth a look
and the improvements they suggest.

Run: python tools/fuzz.py --cases 12 --seed 1 --timeout 90 --report D:/Scratch/fuzz-report.json
"""

import base64
import json
import os
import random
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLI = os.path.join(
    os.environ.get("LOCALAPPDATA", ""), "Programs", "RustGIS", "rustgis-cli.exe"
)
PLUGIN = os.path.join(ROOT, "dist", "dane-pl")

# Real Polish places, so a case lands where the services have data.
PLACES = [
    ("Poznań", 16.90, 52.39),
    ("Kraków", 19.93, 50.05),
    ("Warszawa", 21.00, 52.22),
    ("Gdańsk", 18.64, 54.35),
    ("Wrocław", 17.03, 51.10),
    ("Białystok", 23.15, 53.13),
    ("Szczecin", 14.55, 53.42),
    ("Rzeszów", 21.99, 50.03),
]

UNITS = [
    ("Poznań", "gmina"),
    ("Wrocław", "miasto"),
    ("poznański", "powiat"),
    ("wielkopolskie", "wojewodztwo"),
    ("Łasków", "obreb"),
    ("nie ma takiej jednostki", "gmina"),
]


def box(lon: float, lat: float, span: float) -> list:
    return [round(lon - span / 2, 6), round(lat - span / 2, 6), round(lon + span / 2, 6), round(lat + span / 2, 6)]


def dataset_ids() -> dict:
    """The real dataset ids per tool, read from the catalogue, so a fuzz case runs what the app runs."""
    with open(os.path.join(ROOT, "data", "wfs.json"), encoding="utf-8") as handle:
        datasets = json.load(handle)
    out = {}
    for entry in datasets:
        if entry.get("index"):
            continue
        out.setdefault(entry["tool"], []).append(entry["id"])
    return out


def plan(rng: random.Random, count: int) -> list:
    """The cases: one area command, sometimes followed by a download."""
    cases = []
    catalogued = dataset_ids()
    kinds = ["small", "tiny", "big", "outside", "degenerate", "county", "unit", "wody", "csv"]
    for index in range(count):
        kind = kinds[index % len(kinds)] if count > len(kinds) else kinds[index]
        name, lon, lat = rng.choice(PLACES)
        if kind == "small":
            cases.append(("small area and municipalities", [
                {"id": "obszar-json", "params": {"bbox": box(lon, lat, 0.02)}},
                {"id": "pobierz-granice", "tool": True, "params": {"dane": "A03_Granice_gmin"}},
            ]))
        elif kind == "tiny":
            cases.append(("a 20 m box", [
                {"id": "obszar-json", "params": {"bbox": box(lon, lat, 0.0002)}},
                {"id": "pobierz-adresy", "tool": True,
                 "params": {"dane": rng.choice(catalogued.get("pobierz-adresy", ["A03_Granice_gmin"]))}},
            ]))
        elif kind == "big":
            cases.append(("a 2 degree box", [
                {"id": "obszar-json", "params": {"bbox": box(lon, lat, 2.0)}},
                {"id": "pobierz-granice", "tool": True, "params": {"dane": "A03_Granice_gmin"}},
            ]))
        elif kind == "outside":
            cases.append(("a box outside Poland", [
                {"id": "obszar-json", "params": {"bbox": [10.0, 45.0, 10.1, 45.1]}},
                {"id": "pobierz-granice", "tool": True, "params": {"dane": "A03_Granice_gmin"}},
            ]))
        elif kind == "degenerate":
            cases.append(("an inverted box", [
                {"id": "obszar-json", "params": {"bbox": [17.0, 52.5, 16.9, 52.4]}},
            ]))
        elif kind == "county":
            unit = rng.choice(["Częstochowa", "Poznań", "Kraków"])
            cases.append((f"county EGiB for {unit}", [
                {"id": "obszar-jednostka", "params": {"nazwa": unit, "poziom": "gmina"}},
                {"id": "dzialki-powiat", "params": {"limit": 200}},
            ]))
        elif kind == "unit":
            unit, level = rng.choice(UNITS)
            cases.append((f"unit area {unit} / {level}", [
                {"id": "obszar-jednostka", "params": {"nazwa": unit, "poziom": level}},
            ]))
        elif kind == "wody":
            cases.append(("a Wody service layer list", [
                {"id": "wody-warstwy", "params": {"usluga": "wody-apgw"}},
            ]))
        else:
            rows = ["adres;nazwa"] + [f"{p[0]}, ulica {i};x" for i, p in enumerate(PLACES[:4])] + ["bzdura;z"]
            body = "\n".join(rows) + "\n"
            carried = base64.b64encode(body.encode("utf-8")).decode("ascii")
            cases.append((f"geocode a CSV at {name}", [
                {"id": "geokoduj-csv", "tool": True, "params": {
                    "file": {"name": "fuzz.csv", "data": carried}, "limit": 6}},
            ]))
    return cases


def run_case(commands: list, timeout: int) -> dict:
    args = [CLI, "run", "--plugin", PLUGIN, "--sample"]
    for command in commands:
        params = dict(command.get("params", {}))
        if command.get("tool"):
            # A tool runs through the geoprocessing command, and needs the area layer by name.
            params.setdefault("obszar", "Area to download")
            call = {"tool": f"plugin:dane-pl/{command['id']}", "params": params}
            args += ["--cmd", f"gp.run={json.dumps(call)}"]
        else:
            args += ["--cmd", f"plugin:dane-pl/{command['id']}={json.dumps(params)}"]
    started = time.time()
    try:
        done = subprocess.run(args, capture_output=True, text=True, timeout=timeout, encoding="utf-8", errors="replace")
        elapsed = time.time() - started
        stderr = [line for line in done.stderr.splitlines() if line.startswith("rustgis-cli:")]
        status = stderr[-1] if stderr else ""
        if done.returncode == 0:
            verdict = "OK"
        elif status:
            verdict = "REFUSED"
        else:
            verdict = "CRASH"
        return {
            "verdict": verdict,
            "seconds": round(elapsed, 1),
            "status": status[:300],
            "code": done.returncode,
        }
    except subprocess.TimeoutExpired:
        return {"verdict": "TIMEOUT", "seconds": float(timeout), "status": "", "code": None}


def suggestions(results: list) -> list:
    """What the observed verdicts say about the download paths."""
    out = []
    timeouts = [r for r in results if r["verdict"] == "TIMEOUT"]
    crashes = [r for r in results if r["verdict"] == "CRASH"]
    refusals = [r for r in results if r["verdict"] == "REFUSED"]
    slow = [r for r in results if r["verdict"] == "OK" and r["seconds"] > 20]
    if timeouts:
        out.append(f"{len(timeouts)} cases timed out: the gate should measure the area before the request, not after it")
    if crashes:
        out.append(f"{len(crashes)} cases crashed without a plugin message: a panic or a host call with bad arguments")
    if slow:
        out.append(f"{len(slow)} cases took over 20 s: a tool would keep the window free, a command does not")
    if any("limit" in r["status"].lower() for r in refusals):
        out.append("a refusal names a limit: the message should carry the measured size and the next step")
    if not crashes and not timeouts:
        out.append("no crash and no timeout in this run: the gates hold")
    return out


def main() -> int:
    cases = 12
    seed = 1
    timeout = 90
    report = None
    args = sys.argv[1:]
    for i, arg in enumerate(args):
        if arg == "--cases" and i + 1 < len(args):
            cases = int(args[i + 1])
        elif arg == "--seed" and i + 1 < len(args):
            seed = int(args[i + 1])
        elif arg == "--timeout" and i + 1 < len(args):
            timeout = int(args[i + 1])
        elif arg == "--report" and i + 1 < len(args):
            report = args[i + 1]
    rng = random.Random(seed)
    results = []
    for name, commands in plan(rng, cases):
        outcome = run_case(commands, timeout)
        outcome["case"] = name
        results.append(outcome)
        print(f"{outcome['verdict']:8} {outcome['seconds']:6.1f}s  {name}")
        if outcome["status"]:
            print(f"         {outcome['status'][:120]}")
    print("\nfuzz: " + "; ".join(suggestions(results)))
    if report:
        with open(report, "w", encoding="utf-8") as handle:
            json.dump({"seed": seed, "results": results, "suggestions": suggestions(results)}, handle, ensure_ascii=False, indent=1)
        print(f"fuzz: report written to {report}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
