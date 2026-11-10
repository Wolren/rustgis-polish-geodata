"""Builds data/wcs.json from the GUGiK WCS services.

Reads GetCapabilities and DescribeCoverage for every GUGiK WCS endpoint, keeps the coverages the
publisher names, and proves each one with a real GetCoverage over a small box in Poznań. The
output is committed, so the plugin build needs no network and no Python.

    python tools/wcs.py

The file only grows: a coverage that answered in an earlier run stays, because a service that
fails once must not drop a catalogue entry. A coverage this tool does not describe is reported,
never written.
"""

import io
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request

import xml.etree.ElementTree as ET

CACHE = os.path.join(os.path.dirname(__file__), ".cache")
OUT = os.path.join(os.path.dirname(__file__), "..", "data", "wcs.json")

AGENT = "Mozilla/5.0 (compatible; RustGIS-polish-geodata/0.1)"
SOURCE = "GUGiK \u00b7 geoportal.gov.pl"

# The publisher's WCS endpoints: the five the EZiUDP register lists, and the GeoTIFF terrain one.
SERVICES = (
    "https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WCS/StandardResolution",
    "https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WCS/HighResolution",
    "https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WCS/TrueOrto",
    "https://mapy.geoportal.gov.pl/wss/service/PZGIK/NMT/GRID1/WCS/DigitalTerrainModel",
    "https://mapy.geoportal.gov.pl/wss/service/PZGIK/NMT/GRID1/WCS/DigitalTerrainModelFormatTIFF",
    "https://mapy.geoportal.gov.pl/wss/service/PZGIK/NMPT/GRID1/WCS/DigitalSurfaceModel",
)

# Each coverage, in file order: the id, ribbon label, theme and product kind; labels stay Polish.
COVERAGES = {
    "Orthoimagery_StandardResolution": ("orto-standard", "Ortofoto standard (WCS)", "ortho", "imagery"),
    "Orthoimagery_High_Resolution": ("orto-hi-res", "Ortofoto hi-res (WCS)", "ortho", "imagery"),
    "True_Orthoimagery": ("orto-true", "Ortofoto prawdziwe (WCS)", "ortho", "imagery"),
    "DTM_PL-KRON86-NH": ("nmt-kron86", "NMT KRON86 (WCS)", "models", "terrain"),
    "DTM_PL-EVRF2007-NH": ("nmt-evrf2007", "NMT EVRF2007 (WCS)", "models", "terrain"),
    "DTM_PL-KRON86-NH_TIFF": ("nmt-kron86-tiff", "NMT KRON86 TIFF (WCS)", "models", "terrain"),
    "DSM_PL-KRON86-NH": ("nmpt-kron86", "NMPT KRON86 (WCS)", "models", "surface"),
    "DSM_PL-EVRF2007-NH": ("nmpt-evrf2007", "NMPT EVRF2007 (WCS)", "models", "surface"),
}

# A small box in Poznań, in each projection the services answer in.
BOXES = {
    "EPSG:2180": "347936,496936,348000,497000",
    "EPSG:4326": "16.86,52.35,16.87,52.36",
    "EPSG:3857": "1876921,6873440,1878034,6874779",
}
# The proof request's side: a refusal and a drawing look the same at one pixel.
PROBE_SIDE = 64


def fetch(url, timeout=45):
    """One request, retried: GUGiK drops a TLS connection now and then."""
    last = None
    for attempt in range(4):
        try:
            request = urllib.request.Request(url, headers={"User-Agent": AGENT})
            with urllib.request.urlopen(request, timeout=timeout) as answer:
                return answer.read(), answer.headers.get("Content-Type", "")
        except urllib.error.HTTPError as error:
            last = error
            if error.code < 500:
                raise
        except Exception as error:
            last = error
        time.sleep(1.5 + attempt)
    raise last


def local(tag):
    return tag.split("}")[-1]


def query(endpoint, params):
    base = endpoint.split("?")[0].rstrip("&")
    return base + "?" + params


def coverages(endpoint):
    """The coverage names a WCS 1.0 or 2.0 capabilities document lists, with their bounds."""
    body, _ = fetch(query(endpoint, "SERVICE=WCS&VERSION=1.0.0&REQUEST=GetCapabilities"))
    root = ET.fromstring(body)
    found = {}
    for node in root.iter():
        if local(node.tag) == "CoverageOfferingBrief":
            names = [child_text(node, "name")]
        elif local(node.tag) == "CoverageSummary":
            names = [child_text(node, "CoverageId"), child_text(node, "Identifier")]
        else:
            continue
        box = lonlat_box(node)
        for name in names:
            if name and name not in found:
                found[name] = box
    return found


def lonlat_box(node):
    """The coverage's longitude/latitude bounds, from its envelope."""
    pos = [
        float(value)
        for child in node.iter()
        if local(child.tag) in ("pos", "lowerCorner", "upperCorner")
        for value in (child.text or "").split()
    ]
    if len(pos) < 4:
        return None
    west, south = min(pos[0], pos[-2]), min(pos[1], pos[-1])
    east, north = max(pos[0], pos[-2]), max(pos[1], pos[-1])
    return [round(west, 6), round(south, 6), round(east, 6), round(north, 6)]


def child_text(node, name):
    for child in node:
        if local(child.tag) == name:
            return (child.text or "").strip()
    return ""


def describe(endpoint, coverage):
    """The output formats and the request CRSes the coverage lists."""
    body, _ = fetch(query(endpoint, f"SERVICE=WCS&VERSION=1.0.0&REQUEST=DescribeCoverage&COVERAGE={coverage}"))
    text = body.decode("utf-8", "replace")
    formats = [value.strip() for value in re.findall(r"<formats>([^<]+)</formats>", text)]
    crss = [value.strip() for value in re.findall(r"<requestResponseCRSs>([^<]+)</requestResponseCRSs>", text)]
    crss += [value.strip() for value in re.findall(r"<requestCRSs>([^<]+)</requestCRSs>", text)]
    return formats, crss


def output_format(formats):
    """A GeoTIFF when the coverage offers one, else the ASCII grid it offers."""
    for name in formats:
        if "tif" in name.lower():
            return name
    for name in formats:
        if "aai" in name.lower() or "ascii" in name.lower():
            return name
    return ""


def output_crs(crss):
    """PL-1992 when it is offered, then the two web CRSes, then nothing."""
    offered = {value.upper().replace(" ", "") for value in crss}
    for code in ("EPSG:2180", "EPSG:4326", "EPSG:3857"):
        if code in offered:
            return code
    return ""


def probe(endpoint, coverage, fmt, crs):
    """One real GetCoverage: the magic bytes a GeoTIFF has, or an ASCII grid's header."""
    url = query(
        endpoint,
        f"SERVICE=WCS&VERSION=1.0.0&REQUEST=GetCoverage&FORMAT={fmt}&COVERAGE={coverage}"
        f"&BBOX={BOXES[crs]}&CRS={crs}&RESPONSE_CRS={crs}&WIDTH={PROBE_SIDE}&HEIGHT={PROBE_SIDE}",
    )
    body, _ = fetch(url, timeout=90)
    is_tiff = body[:4] in (b"II*\x00", b"MM\x00*", b"II+\x00", b"MM\x00+")
    if fmt.lower().find("tif") >= 0:
        if not is_tiff:
            head = body[:120].decode("utf-8", "replace").replace("\n", " ")
            return False, f"not a TIFF: {head}", url
    elif b"ncols" not in body[:80]:
        head = body[:120].decode("utf-8", "replace").replace("\n", " ")
        return False, f"not an ASCII grid: {head}", url
    return True, f"{fmt} {PROBE_SIDE} x {PROBE_SIDE} over Poznan", url


def datum(coverage):
    """The height datum a model coverage names, empty for imagery."""
    match = re.search(r"_(PL-[A-Z0-9-]+?)(?:_TIFF)?$", coverage)
    return f"PL-{match.group(1)[3:]}" if match else ""


def entry(endpoint, coverage, fmt, crs, detail, box):
    identity, label, theme, kind = COVERAGES[coverage]
    return {
        "id": identity,
        "cmd": "wcs-" + identity,
        "label": label,
        "source": SOURCE,
        "url": endpoint,
        "coverage": coverage,
        "box": box,
        "crs": crs,
        "format": fmt,
        "kind": kind,
        "theme": theme,
        "datum": datum(coverage) if kind != "imagery" else "",
        "probe": detail,
    }


def previous():
    if not os.path.exists(OUT):
        return []
    with io.open(OUT, encoding="utf-8") as handle:
        return json.load(handle)


def main():
    old = previous()
    fresh = {}
    failed = []
    for endpoint in SERVICES:
        try:
            names = coverages(endpoint)
        except Exception as error:
            failed.append((endpoint, f"capabilities {type(error).__name__}: {error}"))
            print(f"  FAIL {endpoint.split('/WCS/')[-1]:34} capabilities {type(error).__name__}")
            continue
        if not names:
            failed.append((endpoint, "no coverage"))
        for coverage in names:
            if coverage not in COVERAGES:
                failed.append((coverage, "no label in this tool"))
                print(f"  FAIL {coverage:34} no label in this tool")
                continue
            try:
                formats, crss = describe(endpoint, coverage)
                fmt = output_format(formats)
                crs = output_crs(crss)
                if not fmt or not crs:
                    failed.append((coverage, f"formats {formats}, CRS {crss}"))
                    print(f"  FAIL {coverage:34} formats {formats}, CRS {crss}")
                    continue
                if not names[coverage]:
                    failed.append((coverage, "no longitude/latitude bounds"))
                    print(f"  FAIL {coverage:34} no longitude/latitude bounds")
                    continue
                ok, detail, url = probe(endpoint, coverage, fmt, crs)
                if not ok:
                    failed.append((coverage, detail))
                    print(f"  FAIL {coverage:34} {detail[:90]}")
                    continue
                fresh[(endpoint, coverage)] = entry(endpoint, coverage, fmt, crs, detail, names[coverage])
                print(f"  ok   {COVERAGES[coverage][1]:34} {fmt} {crs}")
            except Exception as error:
                failed.append((coverage, f"{type(error).__name__}: {error}"))
                print(f"  FAIL {coverage:34} {type(error).__name__}: {error}")

    # The file only grows: a coverage that does not answer this run keeps its earlier entry.
    kept = []
    written = set()
    for endpoint in SERVICES:
        for coverage in COVERAGES:
            key = (endpoint, coverage)
            if key in fresh:
                kept.append(fresh[key])
                written.add(key)
    for item in old:
        key = (item.get("url"), item.get("coverage"))
        if key not in written:
            kept.append(item)

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with io.open(OUT, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(kept, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    print(f"\nkept {len(kept)}, failed {len(failed)} -> {os.path.normpath(OUT)}")
    if failed:
        os.makedirs(CACHE, exist_ok=True)
        with io.open(os.path.join(CACHE, "wcs-failures.txt"), "w", encoding="utf-8") as handle:
            for name, reason in failed:
                handle.write(f"{name}\t{reason}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
