"""Builds data/services.json from the national service catalogue.

Reads the QGIS GIS Support service list (its own copy of the geoportal catalogue), fetches
GetCapabilities for every WMS entry, keeps the root layer name that a GetMap can draw, and
verifies each template with one real tile request. Entries that fail are listed, never guessed.

    python tools/services.py "path/to/services.json"

The output is committed, so the plugin build needs no network and no Python.
"""

import hashlib
import json
import os
import re
import sys
import threading
import urllib.request

import xml.etree.ElementTree as ET

CACHE = os.path.join(os.path.dirname(__file__), ".cache")
OUT = os.path.join(os.path.dirname(__file__), "..", "data", "services.json")

# Where a tile is asked for, in EPSG:3857 metres: two boxes, 1.2 km and 0.6 km across. A service
# that draws only part of the country (a coast, a river, a mountain park) answers at none of the
# boxes over Poznań, so the probes walk four places: inland, the coast, the mountains, the
# south-west. The first answer that is an image of some size wins.
BOXES = [
    ("Poznań", "1883782,6873571,1884382,6874171"),
    ("Poznań, close", "1883900,6873700,1884200,6874000"),
    ("Gdańsk", "2075809,7236423,2076409,7237023"),
    ("Zakopane", "2220524,6325619,2221124,6326219"),
    ("Wrocław", "1895471,6640475,1896071,6641075"),
]
AGENT = "Mozilla/5.0 (compatible; RustGIS-dane-pl/0.1)"


def fetch(url, timeout=30):
    request = urllib.request.Request(url, headers={"User-Agent": AGENT})
    with urllib.request.urlopen(request, timeout=timeout) as answer:
        return answer.read(), answer.headers.get("Content-Type", "")


def cached(url, suffix=".xml", timeout=30):
    os.makedirs(CACHE, exist_ok=True)
    key = hashlib.sha256(url.encode("utf-8")).hexdigest()[:20]
    path = os.path.join(CACHE, key + suffix)
    if os.path.exists(path):
        with open(path, "rb") as f:
            return f.read(), ""
    data, kind = fetch(url, timeout)
    with open(path, "wb") as f:
        f.write(data)
    return data, kind


def capabilities_url(address):
    base = address.split("?")[0].rstrip("&")
    return base + "?SERVICE=WMS&REQUEST=GetCapabilities&VERSION=1.3.0"


def local(tag):
    return tag.split("}")[-1]


def root_layer(xml_bytes):
    """The layer names a GetMap draws with, and the capabilities version.

    A group layer usually draws nothing on its own, so the leaves are what a request needs.
    """
    tree = ET.fromstring(xml_bytes)
    version = None
    for node in tree.iter():
        if local(node.tag) in ("WMS_Capabilities", "WMT_MS_Capabilities"):
            version = node.get("version")
            break
    top = None
    for node in tree.iter():
        if local(node.tag) == "Capability":
            for child in node:
                if local(child.tag) == "Layer":
                    top = child
                    break
            break
    if top is None:
        return None, version
    own, leaves, pending = None, [], [top]
    while pending and len(leaves) < 40:
        node = pending.pop(0)
        children = [child for child in node if local(child.tag) == "Layer"]
        for child in node:
            if local(child.tag) == "Name" and (child.text or "").strip():
                if node is top:
                    own = child.text.strip()
                else:
                    leaves.append(child.text.strip())
        pending.extend(children)
    if leaves:
        return ",".join(leaves[:40]), version
    return own, version
def template(address, layer, version):
    base = address.split("?")[0].rstrip("&")
    v = "1.3.0" if (version or "1.3.0").startswith("1.3") else "1.1.1"
    crs = "CRS" if v == "1.3.0" else "SRS"
    return (
        f"{base}?SERVICE=WMS&VERSION={v}&REQUEST=GetMap&LAYERS={layer}&STYLES=&"
        f"{crs}=EPSG:3857&BBOX={{bbox-epsg-3857}}&WIDTH={{width}}&HEIGHT={{height}}&"
        "FORMAT=image/png&TRANSPARENT=TRUE"
    )


def probe(address, layer, version):
    url = (
        template(address, layer, version)
        .replace("{bbox-epsg-3857}", BOXES[0][1])
        .replace("{width}", "256")
        .replace("{height}", "256")
    )
    reason = "no answer"
    for place, box in BOXES:
        try:
            data, kind = fetch(url.replace(BOXES[0][1], box), 40)
        except Exception as error:
            reason = f"{type(error).__name__}: {error}"
            continue
        if not kind.lower().startswith("image"):
            head = data[:80].decode("utf-8", "replace").replace(chr(10), " ")
            reason = f"not an image ({kind}): {head}"
            continue
        if len(data) < 500:
            reason = f"image too small over {place} ({len(data)} bytes)"
            continue
        return True, f"{kind} {len(data)} bytes over {place}"
    return False, reason
POLISH = str.maketrans("ąćęłńóśźżĄĆĘŁŃÓŚŹŻ", "acelnoszzACELNOSZZ")


def slug(name):
    """A stable id: Polish letters folded to ascii, so the id reads in a log."""
    flat = name.translate(POLISH).lower()
    return re.sub(r"[^a-z0-9]+", "-", flat).strip("-")[:60]


def https_first(address):
    if address.startswith("http://"):
        return ["https://" + address[len("http://"):], address]
    return [address]


CREDIT = "GUGiK · geoportal.gov.pl"
# Each marker is matched inside the breadcrumb or credit; the value is the publisher name.
PUBLISHERS = (
    ("cartomatic", "Cartomatic"),
    ("główny urząd geodezji i kartografii", CREDIT),
    ("państwowy instytut geologiczny", "Państwowy Instytut Geologiczny (PIG)"),
    ("lasy państwowe", "Lasy Państwowe (LP) - Bank Danych o Lasach"),
    ("generalna dyrekcja ochrony środowiska", "Generalna Dyrekcja Ochrony Środowiska (GDOŚ)"),
    ("narodowy instytut dziedzictwa", "Narodowy Instytut Dziedzictwa (NID)"),
    ("pgw wody polskie", "PGW Wody Polskie"),
    ("główny urząd statystyczny", "Główny Urząd Statystyczny"),
    ("polskie linie kolejowe", "Polskie Linie Kolejowe S.A."),
    ("zakład ubezpieczeń społecznych", "Zakład Ubezpieczeń Społecznych"),
    ("polska agencja kosmiczna", "Polska Agencja Kosmiczna"),
    ("instytut meteorologii i gospodarki wodnej", "Instytut Meteorologii i Gospodarki Wodnej"),
    ("główny urząd nadzoru budowlanego", "Główny Urząd Nadzoru Budowlanego"),
    ("krajowy ośrodek wsparcia rolnictwa", "Krajowy Ośrodek Wsparcia Rolnictwa"),
)


def credit(entry):
    """The map credit of a service: GUGiK's portal for its own, the publisher for the rest.

    A breadcrumb under "DANE INNYCH INSTYTUCJI" names the publisher; every other national
    section is GUGiK's own, and a bare publisher name is kept as it is.
    """
    path = entry.get("source") or entry.get("category") or ""
    folded = path.lower()
    for marker, name in PUBLISHERS:
        if marker in folded:
            return name
    if "usługi" in folded:
        return CREDIT
    return path.strip() or CREDIT


def one(entry):
    address = entry["url"]
    last = "no address"
    for candidate in https_first(address):
        try:
            xml_bytes, _ = cached(capabilities_url(candidate))
        except Exception as error:
            last = f"capabilities {type(error).__name__}: {error}"
            continue
        try:
            layer, version = root_layer(xml_bytes)
        except Exception as error:
            last = f"capabilities unreadable: {error}"
            continue
        if not layer:
            last = "no requestable root layer"
            continue
        ok, detail = probe(candidate, layer, version)
        if not ok:
            last = detail
            continue
        return {
            "id": slug(entry["name"]),
            "name": entry["name"],
            "source": credit(entry),
            "url": template(candidate, layer, version),
            "layer": layer,
            "zoom": 18,
        }, None
    return None, last


def main():
    if len(sys.argv) < 2:
        raise SystemExit("give the path to the GIS Support services.json")
    with open(sys.argv[1], encoding="utf-8") as f:
        catalogue = json.load(f)
    wms = [entry for entry in catalogue if entry.get("type", "WMS").upper() == "WMS"]
    print(f"{len(wms)} WMS entries to check")

    good, bad = [], []
    lock = threading.Lock()
    queue = list(wms)

    def worker():
        while True:
            with lock:
                if not queue:
                    return
                entry = queue.pop()
            kept, reason = one(entry)
            with lock:
                if kept:
                    good.append(kept)
                    print(f"  ok   {kept['name'][:60]:62} {kept['layer'][:28]}")
                else:
                    bad.append((entry["name"], reason))
                    print(f"  FAIL {entry['name'][:60]:62} {reason[:70]}")

    threads = [threading.Thread(target=worker) for _ in range(4)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()

    # A server that drops one connection must not lose a service, so the file only grows: every
    # entry here answered a real GetMap in some run.
    if os.path.exists(OUT):
        with open(OUT, encoding="utf-8") as f:
            for item in json.load(f):
                good.append(item)
    # One entry per name: two catalogue rows for the same service would fight over a layer.
    unique, names = [], set()
    for item in good:
        key = item["name"].lower()
        if key in names:
            continue
        names.add(key)
        unique.append(item)
    good = unique
    used = {}
    for item in good:
        slug = item["id"] or "service"
        used[slug] = used.get(slug, 0) + 1
        if used[slug] > 1:
            item["id"] = f"{slug}-{used[slug]}"
    # Every entry names the command a ribbon button runs and the short label it shows, so the
    # manifest can be built from this file alone.
    for item in good:
        item.setdefault("label", item["name"])
        if not item.get("cmd"):
            item["cmd"] = "usluga-" + item["id"]
    good.sort(key=lambda item: item["name"].lower())
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        json.dump(good, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"\nkept {len(good)}, failed {len(bad)} -> {os.path.normpath(OUT)}")
    with open(os.path.join(CACHE, "failures.txt"), "w", encoding="utf-8") as f:
        for name, reason in bad:
            f.write(f"{name}\t{reason}\n")


if __name__ == "__main__":
    main()
