"""Builds `data/wody.json` from the QGIS plugin "Wody Polskie - Baza WMS".

The 32 Wody Polskie services carry 900-odd layers, and their WMS capabilities are no help:
ArcGIS names every layer with a number and titles it with that number too. The QGIS plugin ships
the only readable layer table, so this script reads it and writes the catalogue:

    python tools/wody.py path/to/wody_polskie_wms/Wody_Polskie_WMS.py

Each service becomes one command in the ribbon and one entry in the data file; the layers stay
data, looked up by the `warstwa` parameter, so 900 layers do not become 900 commands.

The script is run by hand when the plugin is updated, and the output is committed: the build
needs no network and no QGIS.
"""

import collections
import io
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "data", "wody.json")

# The service's path on wody.isok.gov.pl -> (command suffix, ribbon label, menu).
# A label is a proper name of a product, so it reads the same in both interface languages.
SERVICES = {
    "KZGW2/REG_ISOK_RZGW": (
        "pgw-struktura",
        "PGW · nadzory, zarządy zlewni, RZGW",
        "wody-plan",
    ),
    "KZGW/ISOK_PGW": (
        "apgw",
        "aPGW · plan gospodarowania wodami",
        "wody-plan",
    ),
    "KZGW/ISOK_IIaPGW2023": (
        "iiapgw-2023",
        "IIaPGW 2023 · aktualizacja planów",
        "wody-plan",
    ),
    "KZGW/ISOK_PZRP": (
        "pzrp",
        "PZRP · plan zarządzania ryzykiem powodziowym",
        "wody-plan",
    ),
    "KZGW/ISOK_PPSS_WMS": (
        "ppss",
        "PPSS · przeciwdziałanie skutkom suszy",
        "wody-plan",
    ),
    "KZGW/ISOK_WORP_ONNP_1": (
        "worp-morze",
        "WORP · obszary narażone, od morza",
        "wody-worp",
    ),
    "KZGW/ISOK_WORP_ONNP_2": (
        "worp-rzeki",
        "WORP · obszary narażone, rzeczne",
        "wody-worp",
    ),
    "KZGW/ISOK_WORP_ONNP_3": (
        "worp-budowle",
        "WORP · obszary narażone, budowle piętrzące",
        "wody-worp",
    ),
    "KZGW/ISOK_WORP_v2": (
        "worp-v2",
        "WORP · zagrożenie i ryzyko (v2)",
        "wody-worp",
    ),
    "KZGW/MZP20_Glebokosc_NiskiePrawdopodPowodzi": (
        "mzp-glebokosc-02",
        "MZP · głębokość, 0,2%",
        "wody-mzp",
    ),
    "KZGW/MZP20_Glebokosc_SredniePrawdopodPowodzi": (
        "mzp-glebokosc-1",
        "MZP · głębokość, 1%",
        "wody-mzp",
    ),
    "KZGW/MZP20_Glebokosc_WysokiePrawdopodPowodzi": (
        "mzp-glebokosc-10",
        "MZP · głębokość, 10%",
        "wody-mzp",
    ),
    "KZGW/MZP20_Predkosc_NiskiePrawdopodPowodzi": (
        "mzp-predkosc-02",
        "MZP · prędkość, 0,2%",
        "wody-mzp",
    ),
    "KZGW/MZP20_Predkosc_SredniePrawdopodPowodzi": (
        "mzp-predkosc-1",
        "MZP · prędkość, 1%",
        "wody-mzp",
    ),
    "KZGW/MZP20_Predkosc_WysokiePrawdopodPowodzi": (
        "mzp-predkosc-10",
        "MZP · prędkość, 10%",
        "wody-mzp",
    ),
    "KZGW/MZP20_Glebokosc_ZniszczenieWaluPowodziowego": (
        "mzp-zniszczenie-walu",
        "MZP · głębokość, zniszczenie wału",
        "wody-mzp",
    ),
    "KZGW/MZP20_Glebokosc_ZniszczenieBudowliPietrzacejj": (
        "mzp-zniszczenie-budowli",
        "MZP · głębokość, zniszczenie budowli piętrzącej",
        "wody-mzp",
    ),
    "KZGW/MZP20_Glebokosc_ZniszczenieWaluSztormowego": (
        "mzp-zniszczenie-walu-sztormowego",
        "MZP · głębokość, zniszczenie wału sztormowego",
        "wody-mzp",
    ),
    "KZGW/MZP20_ObszarySzczegolnegoZagrozeniaPowodzia": (
        "mzp-oszp",
        "MZP · obszary szczególnego zagrożenia",
        "wody-mzp",
    ),
    "KZGW/MZP20__MapaOrientacyjna": (
        "mzp-orientacyjna",
        "MZP · mapa orientacyjna zasięgów",
        "wody-mzp",
    ),
    "KZGW/MRP20_SkutkiZycieZdrowiePotencjalneStraty_WysokiePrawdopodPowodzi": (
        "mrp-ludzie-10",
        "MRP · ludzie i straty, 10%",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiZycieZdrowiePotencjalneStraty_SredniePrawdopodPowodzi": (
        "mrp-ludzie-1",
        "MRP · ludzie i straty, 1%",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiZycieZdrowiePotencjalneStraty_NiskiePrawdopodPowodzi": (
        "mrp-ludzie-02",
        "MRP · ludzie i straty, 0,2%",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiZycieZdrowiePotencjalneStraty_ZniszczenieWaluPowodziowego": (
        "mrp-ludzie-wal",
        "MRP · ludzie i straty, zniszczenie wału",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiZycieZdrowiePotencjalneStraty_ZniszczenieWaluSztormowego": (
        "mrp-ludzie-wal-sztormowy",
        "MRP · ludzie i straty, wał sztormowy",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiZycieZdrowiePotencjalneStraty_ZniszczenieBudowliPietrzacej": (
        "mrp-ludzie-budowla",
        "MRP · ludzie i straty, zniszczenie budowli",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiSrodowiskoKulturaGospodarka_WysokiePrawdopodPowodzi": (
        "mrp-srodowisko-10",
        "MRP · środowisko i gospodarka, 10%",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiSrodowiskoKulturaGospodarka_SredniePrawdopodPowodzi": (
        "mrp-srodowisko-1",
        "MRP · środowisko i gospodarka, 1%",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiSrodowiskoKulturaGospodarka_NiskiePrawdopodPowodzi": (
        "mrp-srodowisko-02",
        "MRP · środowisko i gospodarka, 0,2%",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiSrodowiskoKulturaGospodarka_ZniszczenieWaluPowodziowego": (
        "mrp-srodowisko-wal",
        "MRP · środowisko i gospodarka, zniszczenie wału",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiSrodowiskoKulturaGospodarka_ZniszczenieWaluSztormowego": (
        "mrp-srodowisko-wal-sztormowy",
        "MRP · środowisko i gospodarka, wał sztormowy",
        "wody-mrp",
    ),
    "KZGW/MRP20_SkutkiSrodowiskoKulturaGospodarka_ZniszczenieBudowliPietrzacej": (
        "mrp-srodowisko-budowla",
        "MRP · środowisko i gospodarka, zniszczenie budowli",
        "wody-mrp",
    ),
}

TRIPLE = re.compile(r'\["(\d+)",\s*"([^"]*)",\s*"(https://[^"]+)"\]')


def main():
    if len(sys.argv) < 2:
        raise SystemExit("give the path to Wody_Polskie_WMS.py from the QGIS plugin")
    text = io.open(sys.argv[1], encoding="utf-8", errors="replace").read()

    by_url = collections.OrderedDict()
    for layer_id, name, url in TRIPLE.findall(text):
        url = url.rstrip("?")
        by_url.setdefault(url, []).append({"id": layer_id, "name": name})

    services = []
    for url, layers in by_url.items():
        segment = url.split("/gpservices/")[1].split("/MapServer")[0]
        if segment not in SERVICES:
            raise SystemExit(f"no label for {segment}: add it to SERVICES")
        suffix, label, menu = SERVICES[segment]
        seen, unique = set(), []
        for layer in layers:
            if layer["id"] in seen:
                continue
            seen.add(layer["id"])
            unique.append(layer)
        # The plugin lists a marine variant first on the coastal services; inland is what a map
        # of Poland usually wants, so the default is the first layer that is not "od morza".
        default = next(
            (layer for layer in unique if "morza" not in layer["name"].lower()),
            unique[0],
        )
        services.append(
            {
                "id": "wody-" + suffix,
                "name": label,
                "url": url,
                "menu": menu,
                "default": default["id"],
                "layers": unique,
            }
        )

    missing = set(SERVICES) - {
        url.split("/gpservices/")[1].split("/MapServer")[0] for url in by_url
    }
    if missing:
        raise SystemExit(f"label for a service the plugin no longer ships: {sorted(missing)}")

    services.sort(key=lambda service: service["name"].lower())
    with io.open(OUT, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(services, handle, indent=1, ensure_ascii=False)
        handle.write("\n")
    layers = sum(len(service["layers"]) for service in services)
    print(f"{len(services)} services, {layers} layers -> {os.path.normpath(OUT)}")


if __name__ == "__main__":
    main()
