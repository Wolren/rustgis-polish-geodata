---
name: dane-pl
description: Polish national geodata from GUGiK, GDOŚ and PIG in RustGIS — pick an area, download WFS datasets as layers, look up a parcel, an address or a height. Use for Polish parcels, addresses, boundaries, orthophoto, Natura 2000, reserves, geological layers and anything "z Geoportalu" or "z GUGiK".
---

# Polish geodata (dane-pl)

Based on the plugin's README. Commands are `plugin:dane-pl/<id>`, tools `plugin:dane-pl/<id>`.

## Area first

Every download takes an area layer (`obszar`), a polygon layer on the map:

- `obszar-widok` makes the area from the current view.
- `obszar-rysuj` arms the map's polygon tool; the drawn shape lands in "Area to download".
- `obszar-prostokat` arms the app's rectangle tool: it draws a graphic, not a feature. Any plugin
  command then takes it, and `obszar-przejmij` does it at once.
- `obszar-anuluj` drops a pending drawing, `obszar-wyczysc` removes the area layer.

The layer is named "Area to download" (Polish: "Obszar pobierania"). A selection on it narrows the
download.

## Download

`run_tool` with one of the six tools and `{"dane": <label>, "obszar": <area layer>, "confirm": false}`:

| Tool | Datasets |
| --- | --- |
| `pobierz-adresy` | addresses, buildings, parcels of a district |
| `pobierz-granice` | PRG units, EGIB boundaries |
| `pobierz-osnowy` | control points, levelling |
| `pobierz-urzedy` | offices, courts, health |
| `pobierz-pozostale` | maritime, historic monuments, the rest |
| `pobierz-gdos` | Natura 2000, reserves, parks, Ramsar, corridors, wind farms |

`dane` is a label from the catalogue, so read it first: `run_command plugin:dane-pl/katalog {}`
gives every dataset's id, label, tool and area limit. `search_commands` finds datasets by word.

Rules the tool enforces, so plan for them:

- 6000 features per run. A bigger area returns what fits and says so.
- Datasets with a small `max_span` refuse a wide area by name; give a smaller area.
- 25 MB of WFS answers per run, unless the panel's slider or the form's `confirm` raises it; a
  confirmed run stops at 256 MB. The message names the size.
- The result is a new layer named after the dataset, added in one undo step.

## Lookups

- `geokoduj {"adres": "Poznań, Głogowska 10"}` adds a point for an address.
- `adres-odwrotnie {"lon": 21.02, "lat": 52.23}` names the address at a point (the map centre when
  the parameters are absent) with the distance in metres.
- `dzialka-id {"id": "…"}` fetches a parcel from ULDK by its identifier.
- `wysokosc {"x": 356000, "y": 487000}` reads the terrain height at PL-1992 coordinates; without
  them it uses the map centre.
- `parcel` takes the parcel under the map centre.

## Clip and panel

- `przytnij` cuts the area's layer to the boundary layer chosen in the panel.
- `panel-state` shows the view, the area, the layers, the catalogue and the stored answers.
- `mapa {"panel": {"values": {"mapa": "v:orto"}}}` adds a map from the panel's list; `v:<command>`
  is a map, `d:<tool>:<dataset>` runs a download. A folder row answers what to pick instead.
- The panel's checkbox and slider store the download size limit for later runs.

## Maps

`plugin:dane-pl/katalog` lists the overlays too: GUGiK (orthophoto, BDOT10k, parcels, addresses,
PRG, place names, hillshade), GDOŚ, PIG, PKP PLK, GUS, Wody Polskie, IMGW and 45 Cartomatic maps.
Each is one command that adds a tile layer, and a second run of the same command removes it.
