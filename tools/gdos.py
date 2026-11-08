"""Build data/gdos.json: the GDOŚ WFS feature types the download tool offers.

Every type is proved with a real GetFeature over Poland in EPSG:2180, because that WFS accepts a
spatial filter only in the national grid. It also refuses `STARTINDEX` ("Cannot do natural order
without a primary key"), so each entry carries `"pages": false` and the plugin asks for one page.

Run: python tools/gdos.py            # probe, merge with the file, write it
     python tools/gdos.py --check    # exit 1 when a listed type stops answering
"""

import io
import json
import os
import sys
import urllib.parse
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "data", "gdos.json")
URL = "https://sdi.gdos.gov.pl/wfs"
TOOL = "pobierz-gdos"
# Poland in PL-1992 metres, wide enough for every site the service holds.
POLAND_2180 = (170000.0, 140000.0, 880000.0, 880000.0)
TIMEOUT = 90

# (feature type, short label, full title); labels are the tool choices, unique and at most 32 characters.
TYPES = [
    ("GDOS:ObszarySpecjalnejOchrony", "Natura 2000 ptasie (OSO)", "Natura 2000 Obszary Specjalnej Ochrony"),
    ("GDOS:SpecjalneObszaryOchrony", "Natura 2000 siedliskowe (SOO)", "Natura 2000 Specjalne Obszary Ochrony"),
    ("GDOS:ObszaryChronionegoKrajobrazu", "Obszary chronionego krajobrazu", "Obszary Chronionego Krajobrazu"),
    ("GDOS:ParkiKrajobrazowe", "Parki krajobrazowe", "Parki Krajobrazowe"),
    ("GDOS:ParkiNarodowe", "Parki narodowe", "Parki Narodowe"),
    ("GDOS:Rezerwaty", "Rezerwaty", "Rezerwaty"),
    ("GDOS:PomnikiPrzyrodyPowierzchniowe", "Pomniki przyrody (pow.)", "Pomniki Przyrody (powierzchniowe)"),
    ("GDOS:PomnikiPrzyrodyPunktowe", "Pomniki przyrody (punktowe)", "Pomniki Przyrody (punktowe)"),
    ("GDOS:StanowiskaDokumentacyjne", "Stanowiska dokumentacyjne", "Stanowiska Dokumentacyjne"),
    ("GDOS:UzytkiEkologiczne", "Użytki ekologiczne", "Użytki Ekologiczne"),
    ("GDOS:ZespolyPrzyrodniczoKrajobrazowe", "Zespoły przyr.-krajobrazowe", "Zespoły Przyrodniczo-Krajobrazowe"),
    ("GDOS:ramsar", "Ramsar (obszary wodno-błotne)", "Obszary wodno-błotne (Konwencja Ramsarska)"),
    ("GDOS:korytarzeEkologiczne", "Korytarze ekologiczne", "Korytarze ekologiczne"),
    ("GDOS:Mezoregiony", "Mezoregiony fizycznogeograficzne", "Mezoregiony fizycznogeograficzne"),
    ("GDOS:ElektrownieWiatrowe", "Elektrownie wiatrowe", "Elektrownie wiatrowe"),
]


def label_of(type_name):
    """A short label for a type the table does not name: the type name split at its capitals."""
    bare = type_name.split(":")[-1]
    words, word = [], ""
    for char in bare:
        if char.isupper() and word:
            words.append(word)
            word = ""
        word += char
    words.append(word)
    return " ".join(words)[:32]


def probe(type_name):
    """Whether the type answers a real request, and how many features it holds over Poland."""
    params = {
        "SERVICE": "WFS",
        "VERSION": "1.1.0",
        "REQUEST": "GetFeature",
        "TYPENAME": type_name,
        "SRSNAME": "EPSG:2180",
        "BBOX": ",".join(f"{value:.0f}" for value in POLAND_2180),
        "MAXFEATURES": "2",
    }
    url = URL + "?" + urllib.parse.urlencode(params)
    request = urllib.request.Request(url, headers={"User-Agent": "polish-geodata/0.1 (+tools/gdos.py)"})
    with urllib.request.urlopen(request, timeout=TIMEOUT) as answer:
        body = answer.read().decode("utf-8", "replace")
    if "ExceptionReport" in body or "ExceptionText" in body:
        raise RuntimeError(body[:200])
    matched = body.split('numberOfFeatures="')
    if len(matched) > 1:
        count = int(matched[1].split('"')[0])
    elif "FeatureCollection" in body:
        count = body.count("<wfs:member") or body.count("<gml:featureMember")
    else:
        count = 0
    geometry = "Point" if "<gml:Point" in body else "Polygon"
    return count, geometry


def load_previous():
    if not os.path.exists(OUT):
        return []
    with io.open(OUT, encoding="utf-8") as handle:
        return json.load(handle)


def merge(previous, kept):
    """A type that answered before stays when it fails now, so a pass cannot shrink the file."""
    seen = {(entry["url"], entry["type"]) for entry in kept}
    for entry in previous:
        if (entry.get("url"), entry.get("type")) not in seen:
            kept.append(entry)
    return kept


def main():
    check = "--check" in sys.argv[1:]
    previous = load_previous()
    kept, failures = [], []
    for type_name, label, title in TYPES:
        try:
            count, geometry = probe(type_name)
        except Exception as error:  # noqa: BLE001 - every failure is reported, none is fatal
            failures.append(f"{type_name}: {error}")
            print(f"gdos: {type_name} did not answer: {error}")
            continue
        print(f"gdos: {type_name} -> {count} features, {geometry}")
        kept.append(
            {
                "id": "gdos-" + type_name.split(":")[-1].lower(),
                "label": label,
                "title": title,
                "url": URL,
                "type": type_name,
                "geom": "geom",
                "max_span": 20.0,
                "index": False,
                "tool": TOOL,
                "pages": False,
                # GDOŚ answers a pair as (easting, northing), GUGiK as (northing, easting).
                "xy": True,
            }
        )
    if not kept:
        print("gdos: nothing answered, the file is left alone")
        return 1
    kept = merge(previous, kept)
    labels = [entry["label"] for entry in kept]
    if len(set(labels)) != len(labels):
        print("gdos: two entries share a label")
        return 1
    for entry in kept:
        if len(entry["label"]) > 32:
            print(f"gdos: label too long: {entry['label']}")
            return 1
    if check:
        missing = {entry["type"] for entry in previous} - {entry["type"] for entry in kept}
        if missing:
            print(f"gdos: not answering now: {sorted(missing)}")
            return 1
        return 0
    text = json.dumps(kept, ensure_ascii=False, indent=1)
    with io.open(OUT, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text + "\n")
    print(f"gdos: wrote {len(kept)} types to data/gdos.json")
    if failures:
        print(f"gdos: {len(failures)} types answered nothing and were left out")
    return 0


if __name__ == "__main__":
    sys.exit(main())
