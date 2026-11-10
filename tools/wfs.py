"""Build data/wfs.json: every national WFS feature type that answers a real GetFeature.

Reads the endpoint list, asks each endpoint for its capabilities, then proves every feature
type with one small request over Poznan. A type that answers stays, a type that errors or
returns an exception report is dropped with the reason printed.

Run: python tools/wfs.py [endpoints.txt]
"""

import hashlib
import io
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from concurrent.futures import ThreadPoolExecutor

AGENT = "Mozilla/5.0 (compatible; RustGIS-polish-geodata/0.1)"
CACHE = os.path.join(os.path.dirname(__file__), ".cache")
OUT = os.path.join(os.path.dirname(__file__), "..", "data", "wfs.json")
BOX_4326 = (16.900, 52.395, 16.930, 52.410)
BOX_2180 = None
WORKERS = 8


def box_2180():
    global BOX_2180
    if BOX_2180 is None:
        from pyproj import Transformer

        forward = Transformer.from_crs("EPSG:4326", "EPSG:2180", always_xy=True)
        x1, y1 = forward.transform(BOX_4326[0], BOX_4326[1])
        x2, y2 = forward.transform(BOX_4326[2], BOX_4326[3])
        BOX_2180 = (min(x1, x2), min(y1, y2), max(x1, x2), max(y1, y2))
    return BOX_2180


def fetch(url, params, timeout=90):
    full = url + ("&" if "?" in url else "?") + urllib.parse.urlencode(params)
    os.makedirs(CACHE, exist_ok=True)
    path = os.path.join(CACHE, hashlib.sha1(full.encode()).hexdigest() + ".xml")
    if os.path.exists(path):
        with io.open(path, "rb") as handle:
            return handle.read()
    request = urllib.request.Request(full, headers={"User-Agent": AGENT})
    with urllib.request.urlopen(request, timeout=timeout) as answer:
        data = answer.read()
    with io.open(path, "wb") as handle:
        handle.write(data)
    return data


def local(tag):
    return tag.rsplit("}", 1)[-1]


def types_of(data):
    root = ET.fromstring(data)
    found = []
    for element in root.iter():
        if local(element.tag) != "FeatureType":
            continue
        name, title = None, None
        for child in element:
            if local(child.tag) == "Name" and name is None:
                name = (child.text or "").strip()
            if local(child.tag) == "Title" and title is None:
                title = (child.text or "").strip()
        if name:
            found.append((name, title or name))
    return found


def capabilities(url):
    for version in ("1.1.0", "2.0.0"):
        try:
            data = fetch(
                url,
                {"SERVICE": "WFS", "VERSION": version, "REQUEST": "GetCapabilities"},
            )
        except Exception as error:
            last = f"{type(error).__name__}: {error}"
            continue
        if b"FeatureTypeList" in data:
            try:
                return version, types_of(data)
            except ET.ParseError as error:
                last = f"XML: {error}"
                continue
        last = "no FeatureTypeList"
    raise RuntimeError(last)


def geom_of(data):
    """The geometry property a feature carries, read from the answer itself."""
    try:
        root = ET.fromstring(data)
    except ET.ParseError:
        return None
    for element in root.iter():
        if local(element.tag) in (
            "boundedBy",
            "Envelope",
            "lowerCorner",
            "upperCorner",
        ):
            continue
        if element.get("srsName") or local(element.tag) in (
            "Point",
            "LineString",
            "Polygon",
            "MultiSurface",
            "MultiCurve",
            "MultiPoint",
            "Surface",
            "Curve",
            "MultiPolygon",
            "MultiLineString",
        ):
            parent_hint = local(element.tag)
            if parent_hint in (
                "Point",
                "LineString",
                "Polygon",
                "Surface",
                "Curve",
                "MultiSurface",
                "MultiCurve",
                "MultiPoint",
                "MultiPolygon",
                "MultiLineString",
            ):
                return None
    return None


def describe(url, version, type_name):
    """The geometry property from the schema, so an empty box still proves a type."""
    try:
        data = fetch(
            url,
            {
                "SERVICE": "WFS",
                "VERSION": version,
                "REQUEST": "DescribeFeatureType",
                "TYPENAME": type_name,
            },
        )
    except Exception:
        return None
    try:
        root = ET.fromstring(data)
    except ET.ParseError:
        return None
    for element in root.iter():
        kind = element.get("type") or ""
        if (
            local(element.tag) == "element"
            and kind.startswith("gml:")
            and kind.endswith("PropertyType")
        ):
            return element.get("name")
    return None


def probe(url, version, type_name):
    """One feature over Poznan, in lon and lat first, then in the national grid."""
    ok, detail, count = _probe(url, version, type_name, "EPSG:4326", BOX_4326)
    if ok:
        return ok, detail, count
    if "srsName" in detail or "geometry" in detail or "short answer" in detail:
        second = _probe(url, version, type_name, "EPSG:2180", box_2180())
        if second[0]:
            return second
        return False, f"{detail} | 2180: {second[1]}", second[2]
    return False, detail, count


def _probe(url, version, type_name, srs, box):
    params = {
        "SERVICE": "WFS",
        "VERSION": version,
        "REQUEST": "GetFeature",
        "TYPENAME": type_name,
        "MAXFEATURES": "1",
        "SRSNAME": srs,
        "BBOX": ",".join(f"{v:.6f}" for v in box),
    }
    try:
        data = fetch(url, params)
    except Exception as error:
        return False, f"{type(error).__name__}: {error}", 0
    text = data.decode("utf-8", "replace")
    if "ExceptionReport" in text or "ServiceException" in text:
        reason = re.sub(r"\s+", " ", re.sub(r"<[^>]+>", " ", text))[:120]
        return False, "exception: " + reason.strip(), 0
    if len(data) < 200:
        return False, f"short answer ({len(data)} bytes)", 0
    count = (
        text.count("<gml:featureMember")
        or text.count("<wfs:member")
        or text.count("<member")
    )
    geom = None
    root = ET.fromstring(data)
    for element in root.iter():
        tag = local(element.tag)
        if tag in (
            "boundedBy",
            "Envelope",
            "lowerCorner",
            "upperCorner",
            "FeatureCollection",
        ):
            continue
        if tag in (
            "Point",
            "LineString",
            "Polygon",
            "MultiSurface",
            "MultiCurve",
            "MultiPoint",
            "Surface",
            "Curve",
            "MultiPolygon",
            "MultiLineString",
        ):
            geom = None
            break
    # The geometry property: the child of the feature that holds a geometry element.
    for member in root.iter():
        if local(member.tag) not in ("featureMember", "member"):
            continue
        for feature in list(member):
            for prop in list(feature):
                for child in prop.iter():
                    if local(child.tag) in (
                        "Point",
                        "LineString",
                        "Polygon",
                        "MultiSurface",
                        "MultiCurve",
                        "MultiPoint",
                        "Surface",
                        "Curve",
                        "MultiPolygon",
                        "MultiLineString",
                    ):
                        geom = local(prop.tag)
                        break
                if not geom and prop.get("srsName"):
                    geom = local(prop.tag)
                if geom:
                    break
            if geom:
                break
        if geom:
            break
    if not geom:
        geom = describe(url, version, type_name)
        if geom:
            return True, geom, count
        return False, f"no geometry property (members {count})", count
    return True, geom, count


def defaults(url, type_name, title):
    """The fields a new type needs to be a catalogue entry, not just a proof.

    `data/wfs.json` carries the id a command and a script name, the label a picker shows, the
    widest area one download may cover and whether the type is a sheet index. A type that is new
    to the file gets these defaults; only a run that never saw it can, because `merge` keeps the
    previous entry, hand-set fields included.

    The span default is the cautious one: the plugin refuses an area wider than it rather than
    asking GUGiK for a province. `index` stays false so a new type shows up in a picker (and, if
    its title clashes with another, `tools/manifest.py` fails the build) instead of vanishing.

    Two service families get their measured quirks: the address service answers positions as
    (easting, northing) and lists longitude first in 4326; the Osnowa services ignore every
    spatial filter and answer the national grid (except the gravity one, which answers degrees).
    """
    slug = re.sub(r"[^a-z0-9]+", "-", type_name.split(":")[-1].lower()).strip("-")
    entry = {"id": "dane-" + slug, "label": title, "max_span": 0.15, "index": False}
    if "KrajowaIntegracjaNumeracjiAdresowej" in url:
        entry["xy"] = True
        entry["lonlat_first"] = True
    if "Osnowa" in url or "PodstawowaOsnowa" in url:
        entry["unfiltered"] = True
        if "Grawimetryczna" not in url:
            entry["grid_reply"] = True
    return entry


def load_previous():
    """The catalogue already on disk, as a list of entries."""
    if not os.path.exists(OUT):
        return []
    try:
        return json.loads(io.open(OUT, encoding="utf-8").read())
    except Exception:
        return []


def merge(previous, kept):
    """The previous entries plus the new ones, one entry per (url, type).

    A run that loses types to a dropped TLS connection must not delete verified entries, so the
    previous entry wins and keeps its hand-set id, label and index flag.
    """
    seen, unique = set(), []
    for entry in previous + kept:
        key = (entry["url"], entry["type"])
        if key in seen:
            continue
        seen.add(key)
        unique.append(entry)
    return unique


def main():
    source = (
        sys.argv[1]
        if len(sys.argv) > 1
        else os.path.join(os.path.dirname(__file__), "wfs-endpoints.txt")
    )
    endpoints = [
        line.strip()
        for line in io.open(source, encoding="utf-8")
        if line.strip() and not line.startswith("#")
    ]
    print(f"{len(endpoints)} endpoints")

    found = []
    for url in endpoints:
        try:
            version, types = capabilities(url)
        except Exception as error:
            print(f"  skip {url.rsplit('/', 1)[-1]:34} {error}")
            continue
        print(f"  {url.rsplit('/', 1)[-1]:34} WFS {version}, {len(types)} types")
        for name, title in types:
            found.append((url, version, name, title))
    print(f"{len(found)} types to prove")

    kept, failed = [], []
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        results = list(pool.map(lambda row: probe(row[0], row[1], row[2]), found))
    for (url, version, name, title), (ok, detail, count) in zip(found, results):
        if ok:
            kept.append(
                {
                    "url": url,
                    "type": name,
                    "geom": detail,
                    "title": title,
                    "version": version,
                    **defaults(url, name, title),
                }
            )
        else:
            failed.append((name, detail))
            print(f"  FAIL {name:40} {detail[:90]}")

    unique = merge(load_previous(), kept)
    print(f"kept {len(kept)}, failed {len(failed)}, catalogue {len(unique)}")
    with io.open(OUT, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(json.dumps(unique, indent=2, ensure_ascii=False) + "\n")
    print("wrote", OUT)


if __name__ == "__main__":
    main()
