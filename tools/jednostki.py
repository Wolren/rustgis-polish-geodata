"""Builds data/jednostki.json: every voivodeship, powiat and gmina with its TERYT code and name.

The panel's tree needs the whole country in the package, because a user picks a unit without a
network round trip. Run this after a PRG change; the file is small enough to ship.

    python tools/jednostki.py
"""

import json
import re
import ssl
import sys
import urllib.request

URL = "https://mapy.geoportal.gov.pl/wss/service/PZGIK/PRG/WFS/AdministrativeBoundaries"
UA = "Mozilla/5.0 (compatible; polish-geodata/0.1; tools/jednostki.py)"
TYPES = {
    "wojewodztwo": "ms:A01_Granice_wojewodztw",
    "powiat": "ms:A02_Granice_powiatow",
    "gmina": "ms:A03_Granice_gmin",
}
OUT = "data/jednostki.json"
CONTEXT = ssl.create_default_context()


def fetch(county_type: str) -> list[tuple[str, str]]:
    """Every code and name of one type, read in one page of the WFS."""
    query = (
        f"{URL}?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature&TYPENAMES={county_type}"
        f"&COUNT=4000&SRSNAME=EPSG:4326"
    )
    request = urllib.request.Request(query, headers={"User-Agent": UA})
    with urllib.request.urlopen(request, timeout=180, context=CONTEXT) as answer:
        body = answer.read().decode("utf-8", "replace")
    pairs = re.findall(
        r"<ms:JPT_KOD_JE>([^<]*)</ms:JPT_KOD_JE>.*?<ms:JPT_NAZWA_>([^<]*)</ms:JPT_NAZWA_>",
        body,
        re.S,
    )
    return [(code.strip(), name.strip()) for code, name in pairs]


def clean(name: str, level: str) -> str:
    """The name without the prefix the publisher puts on a level, in the case a person reads."""
    for prefix in ("powiat ", "gmina ", "miasto "):
        if name.lower().startswith(prefix):
            name = name[len(prefix):]
    name = name.strip()
    if level == "gmina":
        name = name[:1].upper() + name[1:]
    return name


def main() -> int:
    voivodeships = fetch(TYPES["wojewodztwo"])
    powiaty = fetch(TYPES["powiat"])
    gminy = fetch(TYPES["gmina"])
    print(f"prg: {len(voivodeships)} voivodeships, {len(powiaty)} powiaty, {len(gminy)} gminy")
    if not voivodeships or not powiaty or not gminy:
        print("the WFS answered nothing: nothing written")
        return 1

    by_code = {code: name for code, name in voivodeships}
    tree = []
    for code, name in sorted(voivodeships):
        tree.append({"kod": code, "nazwa": clean(name, "wojewodztwo"), "powiaty": []})
    index = {node["kod"]: node for node in tree}

    for code, name in sorted(powiaty):
        parent = index.get(code[:2])
        if parent is None:
            continue
        parent["powiaty"].append({"kod": code, "nazwa": clean(name, "powiat"), "gminy": []})

    powiat_index = {}
    for node in tree:
        for powiat in node["powiaty"]:
            powiat_index[powiat["kod"]] = powiat
    for code, name in sorted(gminy):
        parent = powiat_index.get(code[:4])
        if parent is None:
            continue
        parent["gminy"].append({"kod": code, "nazwa": clean(name, "gmina")})

    body = json.dumps({"wojewodztwa": tree}, ensure_ascii=False, separators=(",", ":"))
    with open(OUT, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(body + "\n")
    leaves = sum(len(p["gminy"]) for node in tree for p in node["powiaty"])
    print(f"wrote {OUT}: {len(tree)} voivodeships, {len(powiat_index)} powiaty, {leaves} gminy")
    return 0


if __name__ == "__main__":
    sys.exit(main())
