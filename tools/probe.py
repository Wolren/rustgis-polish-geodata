"""Probes every WMS template in `data/services.json` and every built-in overlay in `src/catalog.rs`.

A catalogue entry is only as good as the address behind it: `PZGIK/ORTO/WMS/StandardResolution`
answered a real GetMap when it was added and returns 404 everywhere today. `tools/services.py`
never removes an entry (a dropped connection must not lose a service), so nothing noticed.

    python tools/probe.py            audit, exit 1 when any entry is dead
    python tools/probe.py --prune    also drop the dead entries from data/services.json
    python tools/probe.py --overlays only the built-in overlays from src/catalog.rs

Three boxes per place, widest first: the country, 20 km, and a 1.2 km square. A service may be
scale-dependent (addresses and cadastral parcels draw only when zoomed in) or cover one region
(a river, a park, a coast), so one box cannot judge it. "Dead" means no image at any of the
fifteen requests: an HTTP error, an XML exception, or a PNG so small it carries no drawing.
"""

import argparse
import io
import json
import math
import os
import re
import ssl
import sys
import threading
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SERVICES = os.path.join(ROOT, "data", "services.json")
CATALOG = os.path.join(ROOT, "src", "catalog.rs")

PLACES = [
    ("Poznan", "1883782,6873571,1884382,6874171"),
    ("Gdansk", "2075809,7236423,2076409,7237023"),
    ("Zakopane", "2220524,6325619,2221124,6326219"),
    ("Wroclaw", "1895471,6640475",),
    ("Warszawa", "2338123,6841120,2339523,6842520"),
]
PLACES[3] = ("Wroclaw", "1895471,6640475,1896071,6641075")
# Half-widths in metres around each place: the country, then 20 km, then 600 m.
SPANS = [("country", 1_500_000), ("20 km", 20_000), ("1 km", 600)]

AGENT = "Mozilla/5.0 (compatible; RustGIS-polish-geodata probe)"
TIMEOUT = 90
# An empty transparent 256 px tile is a few hundred bytes; anything under this draws nothing.
MIN_BYTES = 1200


def ssl_context():
    """Trust the OS bundle as well as Python's: this machine sits behind a TLS-inspecting proxy."""
    try:
        import certifi

        return ssl.create_default_context(cafile=certifi.where())
    except Exception:
        return ssl.create_default_context()


CONTEXT = ssl_context()


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": AGENT})
    with urllib.request.urlopen(request, timeout=TIMEOUT, context=CONTEXT) as answer:
        return answer.read(), answer.headers.get("Content-Type", "")


def box_around(box, half):
    """A square `half` metres either side of the centre of a `west,south,east,north` box."""
    parts = [float(part) for part in box.split(",")]
    cx, cy = (parts[0] + parts[2]) / 2.0, (parts[1] + parts[3]) / 2.0
    return f"{cx - half:.0f},{cy - half:.0f},{cx + half:.0f},{cy + half:.0f}"


def tile_url(template, box, half):
    """The request for one place: a bbox template, or a z/x/y one turned into a tile."""
    if "{z}" in template:
        z = 16 if half <= 600 else 14 if half <= 20_000 else 6
        parts = [float(part) for part in box.split(",")]
        lon = (parts[0] + parts[2]) / 2.0 / 20037508.34 * 180.0
        lat = math.degrees(
            2 * math.atan(math.exp((parts[1] + parts[3]) / 2.0 / 6378137.0)) - math.pi / 2
        )
        n = 2 ** z
        x = int((lon + 180.0) / 360.0 * n)
        y = int(
            (1.0 - math.log(math.tan(math.radians(lat)) + 1 / math.cos(math.radians(lat))) / math.pi)
            / 2.0
            * n
        )
        return template.replace("{z}", str(z)).replace("{x}", str(x)).replace("{y}", str(y))
    return (
        template.replace("{bbox-epsg-3857}", box_around(box, half))
        .replace("{width}", "256")
        .replace("{height}", "256")
    )


def probe(template):
    """(drawn?, detail): the first request that answers with a drawn tile decides."""
    seen = []
    for place, box in PLACES:
        for label, half in SPANS:
            try:
                data, kind = fetch(tile_url(template, box, half))
            except Exception as error:
                seen.append(f"{place}/{label}: {type(error).__name__}: {error}")
                continue
            if not kind.lower().startswith("image"):
                head = data[:60].decode("utf-8", "replace").replace("\n", " ")
                seen.append(f"{place}/{label}: not an image ({kind}) {head[:50]}")
                continue
            if len(data) < MIN_BYTES:
                seen.append(f"{place}/{label}: blank ({len(data)} B)")
                continue
            return True, f"{kind} {len(data)} B over {place}/{label}"
    return False, "; ".join(seen[:3]) or "no answer"


def overlays():
    """The built-in overlays, read out of the catalogue's Rust source."""
    text = io.open(CATALOG, encoding="utf-8").read()
    # Addresses written as constants (KIEG_URL, BDOT_URL) are resolved from their definitions.
    constants = {
        name: re.sub(r"\\\s*\n\s*", "", body)
        for name, body in re.findall(
            r'const (\w+): &str = "((?:[^"\\]|\\.)*)";', text, re.S
        )
    }
    block = text.split("pub const OVERLAYS", 1)[1].split("];", 1)[0]
    found = []
    for entry in block.split("Overlay {")[1:]:
        ident = re.search(r'id:\s*"([^"]+)"', entry)
        url = re.search(r'url:\s*"((?:[^"\\]|\\.)*)"', entry, re.S)
        named = re.search(r"url:\s*(\w+)", entry)
        if not ident:
            continue
        if url:
            template = re.sub(r"\\\s*\n\s*", "", url.group(1))
        elif named and named.group(1) in constants:
            template = constants[named.group(1)]
        else:
            continue
        found.append((ident.group(1), template))
    return found


def audit(entries, prune_path=None):
    results = {}
    lock = threading.Lock()
    queue = list(entries)

    def worker():
        while True:
            with lock:
                if not queue:
                    return
                ident, template = queue.pop()
            drawn, detail = probe(template)
            with lock:
                results[ident] = (drawn, detail)
                print(f"  {'ok  ' if drawn else 'DEAD'} {ident[:52]:54} {detail[:64]}")

    threads = [threading.Thread(target=worker) for _ in range(6)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()

    dead = [ident for ident, _ in entries if not results[ident][0]]
    print(f"\n{len(entries) - len(dead)}/{len(entries)} draw a tile; {len(dead)} dead")
    for ident in dead:
        template = dict(entries)[ident]
        print(f"  {ident}\n    {template[:120]}\n    {results[ident][1]}")

    if dead and prune_path:
        with io.open(prune_path, encoding="utf-8") as handle:
            rows = json.load(handle)
        kept = [row for row in rows if row.get("id") not in dead]
        with io.open(prune_path, "w", encoding="utf-8", newline="\n") as handle:
            json.dump(kept, handle, indent=2, ensure_ascii=False)
            handle.write("\n")
        print(f"dropped {len(dead)}, kept {len(kept)} -> {os.path.normpath(prune_path)}")
        return 0
    return 1 if dead else 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--prune", action="store_true", help="drop the dead services entries")
    parser.add_argument("--overlays", action="store_true", help="probe only src/catalog.rs")
    args = parser.parse_args()

    if args.overlays:
        entries = [(ident, template) for ident, template in overlays() if "PLACEHOLDER" not in template]
        print(f"{len(entries)} built-in overlays")
        return audit(entries)

    with io.open(SERVICES, encoding="utf-8") as handle:
        services = json.load(handle)
    entries = [(service["id"], service["url"]) for service in services]
    return audit(entries, SERVICES if args.prune else None)


if __name__ == "__main__":
    sys.exit(main())
