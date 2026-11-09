"""Probe the register's WFS addresses and write the ones that answer as catalogue candidates.

The register carries 1 554 WFS addresses and no types. This reads each one's capabilities, asks for
one feature type over a small Polish box, and keeps the address only when a real feature arrives.
Output is `data/wfs-more.json`; merging into `data/wfs.json` is a separate, reviewed step.

    python tools/wfs_more.py --limit 60 --report D:/Scratch/wfs-more.json
"""

import json
import os
import re
import ssl
import sys
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UA = "Mozilla/5.0 (compatible; polish-geodata/0.1; tools/wfs_more.py)"
CONTEXT = ssl.create_default_context()
# A small box over Poznań, small enough for any service and large enough to hold data.
BOX = (16.85, 52.35, 16.95, 52.45)
CACHE = os.path.join(ROOT, "tools", ".cache")
OUT = os.path.join(CACHE, "wfs-more.json")
# The type each answered address carries, so rejestr-pobierz can read it without capabilities.
TYPES_OUT = os.path.join(ROOT, "data", "eziudp-types.json")


def fetch(url: str, timeout: int = 45) -> str | None:
    request = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with urllib.request.urlopen(request, timeout=timeout, context=CONTEXT) as answer:
            return answer.read(4_000_000).decode("utf-8", "replace")
    except Exception:
        return None


def types_of(body: str) -> list[str]:
    """The feature type names a capabilities document lists, prefix kept for GeoServer."""
    listed = re.search(r"<FeatureTypeList>.*?</FeatureTypeList>", body, re.S)
    if not listed:
        return []
    out = []
    for name in re.findall(r"<Name>([^<]+)</Name>", listed.group(0)):
        if name and name not in out:
            out.append(name)
    return out


def geometry_field(caps: str, type_name: str) -> str:
    """The geometry property a feature type names in its capabilities entry."""
    described = re.search(rf"<Name>{re.escape(type_name)}</Name>.*?</FeatureType>", caps, re.S)
    if described:
        lowered = described.group(0)
        for field in ("msGeometry", "geometry", "geom", "SHAPE", "the_geom"):
            if field in lowered:
                return field
    return "msGeometry"


def probe(entry: dict) -> dict | None:
    """One register address: its first answering feature type, or nothing."""
    address = entry["url"]
    base = address.split("?")[0]
    for version in ("2.0.0", "1.1.0"):
        caps = fetch(f"{base}?SERVICE=WFS&VERSION={version}&REQUEST=GetCapabilities")
        if not caps or "FeatureTypeList" not in caps:
            continue
        names = types_of(caps)
        # A municipal service covers only its own unit, so a fixed box lands outside it: ask for any.
        for type_name in names[:3]:
            for box in (None, BOX):
                query = (
                    f"{base}?SERVICE=WFS&VERSION={version}&REQUEST=GetFeature"
                    f"&TYPENAME={urllib.parse.quote(type_name)}&COUNT=5"
                )
                if box:
                    query += f"&BBOX={box[0]},{box[1]},{box[2]},{box[3]}"
                body = fetch(query, 60)
                if not body or "ExceptionReport" in body:
                    continue
                members = body.count("gml:featureMember") + body.count("<wfs:member>")
                if members == 0:
                    continue
                return {
                    "url": base,
                    "version": version,
                    "type": type_name,
                    "geom": geometry_field(caps, type_name),
                    "features": members,
                    "institution": entry.get("institution", ""),
                    "name": entry.get("name", ""),
                }
    return None


def main() -> int:
    limit = 60
    args = sys.argv[1:]
    for index, arg in enumerate(args):
        if arg == "--limit" and index + 1 < len(args):
            limit = int(args[index + 1])
    with open(os.path.join(ROOT, "data", "eziudp.json"), encoding="utf-8") as handle:
        register = json.load(handle)
    with open(os.path.join(ROOT, "data", "wfs.json"), encoding="utf-8") as handle:
        known = {entry["url"].split("?")[0] for entry in json.load(handle)}
    addresses = []
    seen = set()
    for entry in register:
        if entry.get("kind") != "wfs" or entry.get("proven") is False:
            continue
        base = entry["url"].split("?")[0]
        if base in known or base in seen:
            continue
        seen.add(base)
        addresses.append({**entry, "url": base})
    addresses = addresses[:limit]
    print(f"{len(addresses)} register addresses to probe, {len(known)} already catalogued")

    found = []
    with ThreadPoolExecutor(max_workers=8) as pool:
        for result in pool.map(probe, addresses):
            if result:
                found.append(result)
                print(f"  {result['type']} <- {result['url']} ({result['features']} features)")
    print(f"{len(found)} of {len(addresses)} answer with a feature type")
    os.makedirs(CACHE, exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(found, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    # The long tail is addressed by its type, so each answered address gets one, keyed by address.
    types = {}
    if os.path.exists(TYPES_OUT):
        with open(TYPES_OUT, encoding="utf-8") as handle:
            types = json.load(handle)
    for result in found:
        types[result["url"]] = {
            "type": result["type"],
            "version": result["version"],
            "geom": result["geom"],
            "name": result.get("name", ""),
        }
    with open(TYPES_OUT, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(dict(sorted(types.items())), handle, ensure_ascii=False, indent=0)
        handle.write("\n")
    print(f"wrote {OUT} and {TYPES_OUT} ({len(types)} addresses with a type)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
