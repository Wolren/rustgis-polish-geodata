"""Build data/eziudp.json: the static register of Polish WMS and WFS services EZiUDP reports.

The plugin refuses the unfiltered register at run time (9 MB, 12 000 rows), so this builder takes
it once, groups the addresses, proves every service with a real request and writes the survivors.

Run: python tools/eziudp.py            # harvest, verify, merge with the file, write it
     python tools/eziudp.py --check    # exit 1 when a service listed in the file stops answering
     python tools/eziudp.py --limit 40 # a trial run over the first 40 addresses
"""

import io
import json
import os
import re
import sys
import threading
import urllib.parse
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "data", "eziudp.json")
API = "https://integracja.gugik.gov.pl/eziudp/api.php?format=json"
AGENT = "Mozilla/5.0 (compatible; polish-geodata/0.1; tools/eziudp.py)"
TIMEOUT = 25
# WMS tiles are asked for in EPSG:3857 over Poznań, the coast and the mountains, so a partial service answers.
BOXES = [
    ("Poznań", "1883782,6873571,1884382,6874171"),
    ("Gdańsk", "2075809,7236423,2076409,7237023"),
    ("Zakopane", "2220524,6325619,2221124,6326219"),
]
THREADS = 10


def fetch(url, timeout=TIMEOUT):
    request = urllib.request.Request(url, headers={"User-Agent": AGENT})
    with urllib.request.urlopen(request, timeout=timeout) as answer:
        return answer.read()


def register():
    """Every row the register holds, read once."""
    body = fetch(API, 300)
    data = json.loads(body.decode("utf-8", "replace"))
    return data.get("data") or []


def clean(raw):
    """The address itself: the register writes some as `WMS=http://…`."""
    text = str(raw).strip()
    if "=" in text.split("//")[0]:
        text = text.split("=", 1)[1]
    return text.strip()


def kind_of(url):
    low = url.lower()
    if "wfs" in low:
        return "wfs"
    if "csw" in low or "geonetwork" in low or "catalogue" in low:
        return "csw"
    if "wcs" in low:
        return "wcs"
    if "wms" in low or "mapserver" in low or "geoserver" in low:
        return "wms"
    return ""


def wms_layer(caps):
    """The first layer a GetMap can ask for: a named layer whose own Name is not empty."""
    for match in re.finditer(r"<Layer[^>]*>(.*?)</Layer>", caps, re.S):
        block = match.group(1)
        name = re.search(r"<Name>(?:<!\[CDATA\[)?([^\]<]+)", block)
        if name and name.group(1).strip():
            return name.group(1).strip()
    return ""


def normalize(url):
    """One address per service: case and a trailing separator do not make a second one."""
    parts = urllib.parse.urlsplit(url.strip())
    host = (parts.hostname or "").lower()
    path = parts.path.rstrip("/")
    query = parts.query.rstrip("&")
    return urllib.parse.urlunsplit((parts.scheme.lower(), host, path, query, ""))


def probe_wms(url):
    """A layer name and a drawn tile, or the reason it failed."""
    sep = "&" if "?" in url else "?"
    caps = fetch(f"{url}{sep}SERVICE=WMS&VERSION=1.3.0&REQUEST=GetCapabilities").decode(
        "utf-8", "replace"
    )
    if "WMS_Capabilities" not in caps and "WMT_MS_Capabilities" not in caps:
        lower = caps.lower()
        if "<html" in lower or "exception" in lower:
            raise RuntimeError("not a WMS")
    layer = wms_layer(caps)
    if not layer:
        raise RuntimeError("no layer")
    for _, box in BOXES:
        tile = (
            f"{url}{sep}SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS={urllib.parse.quote(layer)}"
            f"&STYLES=&CRS=EPSG:3857&BBOX={box}&WIDTH=256&HEIGHT=256&FORMAT=image/png"
            "&TRANSPARENT=TRUE"
        )
        try:
            body = fetch(tile)
        except Exception:  # noqa: BLE001 - the next box may answer
            continue
        if body[:4] in (b"\x89PNG", b"GIF8") or body[:2] == b"\xff\xd8":
            return layer
    # A server that predates Web Mercator still answers a lon/lat box.
    for box in ("16.85,52.30,16.95,52.40", "18.55,54.30,18.65,54.40"):
        tile = (
            f"{url}{sep}SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS={urllib.parse.quote(layer)}"
            f"&STYLES=&CRS=EPSG:4326&BBOX={box}&WIDTH=256&HEIGHT=256&FORMAT=image/png"
            "&TRANSPARENT=TRUE"
        )
        try:
            body = fetch(tile)
        except Exception:  # noqa: BLE001 - the next box may answer
            continue
        if body[:4] in (b"\x89PNG", b"GIF8") or body[:2] == b"\xff\xd8":
            return layer
    # Older servers want WMS 1.1.1: `SRS`, and the box in lon/lat.
    for box in ("16.85,52.30,16.95,52.40", "18.55,54.30,18.65,54.40"):
        tile = (
            f"{url}{sep}SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS={urllib.parse.quote(layer)}"
            f"&STYLES=&SRS=EPSG:4326&BBOX={box}&WIDTH=256&HEIGHT=256&FORMAT=image/png"
        )
        try:
            body = fetch(tile)
        except Exception:  # noqa: BLE001 - the next box may answer
            continue
        if body[:4] in (b"\x89PNG", b"GIF8") or body[:2] == b"\xff\xd8":
            return layer
    raise RuntimeError("no tile")


def probe_wfs(url):
    sep = "&" if "?" in url else "?"
    caps = fetch(f"{url}{sep}SERVICE=WFS&VERSION=2.0.0&REQUEST=GetCapabilities").decode(
        "utf-8", "replace"
    )
    if "WFS_Capabilities" not in caps:
        raise RuntimeError("not a WFS")
    return True


def harvest(rows):
    """One entry per address: the first dataset that reports it, with how many share it."""
    by_url = {}
    for row in rows:
        services = row.get("uslugi") or {}
        for key in ("przegladania", "pobierania", "wyszukiwania"):
            for raw in services.get(key) or []:
                url = clean(raw)
                if not url.lower().startswith("http"):
                    continue
                url = normalize(url)
                kind = kind_of(url)
                if kind not in ("wms", "wfs"):
                    continue
                entry = by_url.get(url)
                if entry is None:
                    by_url[url] = {
                        "url": url,
                        "kind": kind,
                        "name": str(row.get("zbior") or "").strip(),
                        "institution": str(row.get("organ") or "").strip(),
                        "unit": str(row.get("jednostka") or "").strip(),
                        "teryt": str(row.get("teryt") or "").strip(),
                        "area": str(row.get("obszar") or "").strip(),
                        "datasets": 1,
                        "layer": "",
                    }
                else:
                    entry["datasets"] += 1
    return by_url


def verify(entries, previous):
    """Checks every address, in a few threads, and keeps the reasons the failures failed."""
    done = {}
    lock = threading.Lock()
    queue = list(entries)
    failures = []

    def work():
        while True:
            with lock:
                if not queue:
                    return
                entry = queue.pop()
            try:
                if entry["kind"] == "wms":
                    entry["layer"] = probe_wms(entry["url"])
                else:
                    probe_wfs(entry["url"])
                with lock:
                    done[entry["url"]] = entry
            except Exception as error:  # noqa: BLE001 - reported, never fatal
                with lock:
                    failures.append((entry["url"], str(error)[:120]))

    threads = [threading.Thread(target=work) for _ in range(THREADS)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    return done, failures


def load_previous():
    if not os.path.exists(OUT):
        return []
    with io.open(OUT, encoding="utf-8") as handle:
        return json.load(handle)


def retry():
    """Re-probes what the file lacks and keeps the addresses that now answer."""
    known = {entry["url"] for entry in load_previous()}
    entries = [entry for url, entry in harvest(register()).items() if url not in known]
    entries = [entry for entry in entries if entry["kind"] in ("wms", "wfs")]
    print(f"eziudp: retrying {len(entries)} addresses")
    done, failures = verify(entries, [])
    kept = load_previous() + list(done.values())
    kept.sort(key=lambda e: (e["institution"].lower(), e["name"].lower(), e["url"]))
    for entry in kept:
        entry["id"] = "eziudp-" + re.sub(r"[^a-z0-9]+", "-", entry["url"].lower()).strip("-")[-48:]
    text = json.dumps(kept, ensure_ascii=False, indent=1)
    with io.open(OUT, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text + "\n")
    print(f"eziudp: {len(done)} more answered, {len(failures)} still do not; {len(kept)} entries")
    return 0


def main():
    if "--retry" in sys.argv[1:]:
        return retry()
    check = "--check" in sys.argv[1:]
    limit = 0
    for i, arg in enumerate(sys.argv):
        if arg == "--limit" and i + 1 < len(sys.argv):
            limit = int(sys.argv[i + 1])
    previous = load_previous()
    rows = register()
    print(f"eziudp: {len(rows)} rows")

    if check:
        entries = {entry["url"]: entry for entry in previous}
        done, failures = verify(list(entries.values()), previous)
        print(f"eziudp: {len(done)} still answer, {len(failures)} do not")
        return 1 if failures else 0

    by_url = harvest(rows)
    print(f"eziudp: {len(by_url)} addresses, {sum(1 for e in by_url.values() if e['kind'] == 'wms')} WMS")
    targets = list(by_url.values())
    if limit:
        targets = targets[:limit]
    done, failures = verify(targets, previous)
    print(f"eziudp: {len(done)} answered, {len(failures)} did not")

    # An address that failed today stays in the file when it answered once before.
    kept = sorted(done.values(), key=lambda e: (e["institution"].lower(), e["name"].lower(), e["url"]))
    seen = {entry["url"] for entry in kept}
    for entry in previous:
        if entry["url"] not in seen and not limit:
            kept.append(entry)
    for entry in kept:
        entry["id"] = "eziudp-" + re.sub(r"[^a-z0-9]+", "-", entry["url"].lower()).strip("-")[-48:]
    text = json.dumps(kept, ensure_ascii=False, indent=1)
    with io.open(OUT, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text + "\n")
    print(f"eziudp: wrote {len(kept)} entries to data/eziudp.json")
    if failures:
        print(f"eziudp: {len(failures)} addresses did not answer, kept out")
        for url, reason in failures[:12]:
            print(f"  {reason}: {url}")
        os.makedirs(os.path.join(ROOT, "tools", ".cache"), exist_ok=True)
        with io.open(os.path.join(ROOT, "tools", ".cache", "eziudp_failures.txt"), "w", encoding="utf-8") as handle:
            for url, reason in failures:
                handle.write(f"{url}\t{reason}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
