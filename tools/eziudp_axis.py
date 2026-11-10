"""Probes the register's WFS services: the box order each one accepts, and the reply units.

For every WFS in data/eziudp.json the probe reads the capabilities, then asks for one feature
over a country box in both orders. The answer lands in data/eziudp-axis.json, keyed by URL:
{"filter": "lonlat"|"latlat"|"grid"|"none", "reply": "lonlat"|"latlon"|"grid"|"other"}.

    python tools/eziudp_axis.py [--workers 8] [--limit N]
"""

import argparse
import concurrent.futures
import io
import json
import re
import threading
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
OUT = DATA / "eziudp-axis.json"
POLAND = (14.0, 49.0, 24.2, 55.0)
TIMEOUT = 12
lock = threading.Lock()


def get(url):
    request = urllib.request.Request(url, headers={"User-Agent": "RustGIS polish-geodata axis probe"})
    with urllib.request.urlopen(request, timeout=TIMEOUT) as answer:
        return answer.read().decode("utf-8", "replace")


def sep(url):
    return "&" if "?" in url else "?"


def capabilities(url):
    for version in ("2.0.0", "1.1.0"):
        try:
            body = get(f"{url}{sep(url)}SERVICE=WFS&VERSION={version}&REQUEST=GetCapabilities")
        except Exception:
            continue
        names = re.findall(r"<FeatureType[^>]*>.*?<Name>([^<]+)</Name>", body, re.S)
        if names:
            return version, names[0]
    return None


def feature(url, version, type_name, bbox, lat_first):
    lon0, lat0, lon1, lat1 = bbox
    box = f"{lat0},{lon0},{lat1},{lon1}" if lat_first else f"{lon0},{lat0},{lon1},{lat1}"
    parameter = "TYPENAMES" if version.startswith("2") else "TYPENAME"
    query = (f"{url}{sep(url)}SERVICE=WFS&VERSION={version}&REQUEST=GetFeature"
             f"&{parameter}={urllib.parse.quote(type_name)}&SRSNAME=EPSG:4326&COUNT=1&MAXFEATURES=1&BBOX={box}")
    return get(query)


def classify(body):
    if "numberOfFeatures=\"0\"" in body:
        return None
    match = re.search(r"<gml:(?:pos|posList|coordinates)[^>]*>([^<]+)</gml:", body)
    if not match:
        return None
    numbers = re.findall(r"-?\d+(?:\.\d+)?", match.group(1))
    if len(numbers) < 2:
        return None
    a, b = float(numbers[0]), float(numbers[1])
    if abs(a) > 180 or abs(b) > 180:
        return "grid"
    if 14 <= a <= 25 and 48 <= b <= 55:
        return "lonlat"
    if 48 <= a <= 55 and 14 <= b <= 25:
        return "latlon"
    return "other"


def probe(row):
    url = row["url"]
    caps = capabilities(url)
    if not caps:
        return url, {"filter": "none"}
    version, type_name = caps
    marks = {}
    for lat_first in (False, True):
        try:
            body = feature(url, version, type_name, POLAND, lat_first)
        except Exception:
            continue
        found = classify(body)
        if found:
            marks["latlat" if lat_first else "lonlat"] = found
    if not marks:
        return url, {"filter": "none"}
    reply = next(iter(marks.values()))
    if reply == "grid":
        return url, {"filter": "grid", "reply": reply}
    if "lonlat" in marks:
        return url, {"filter": "lonlat", "reply": marks["lonlat"]}
    return url, {"filter": "latlat", "reply": marks["latlat"]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--limit", type=int)
    args = parser.parse_args()
    rows = [row for row in json.load(io.open(DATA / "eziudp.json", encoding="utf-8")) if row.get("kind") == "wfs"]
    done = {}
    if OUT.exists():
        done = json.load(io.open(OUT, encoding="utf-8"))
    todo = [row for row in rows if row["url"] not in done]
    if args.limit:
        todo = todo[: args.limit]
    print(f"{len(todo)} services to probe, {len(done)} already known", flush=True)
    finished = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = {pool.submit(probe, row): row for row in todo}
        for future in concurrent.futures.as_completed(futures):
            try:
                url, axis = future.result()
            except Exception:
                continue
            with lock:
                done[url] = axis
                finished += 1
                if finished % 10 == 0:
                    io.open(OUT, "w", encoding="utf-8").write(json.dumps(done, ensure_ascii=False, sort_keys=True) + "\n")
                    print(f"{finished}/{len(todo)}", flush=True)
    io.open(OUT, "w", encoding="utf-8").write(json.dumps(done, ensure_ascii=False, sort_keys=True) + "\n")
    print(f"done: {len(done)} services", flush=True)


if __name__ == "__main__":
    main()
