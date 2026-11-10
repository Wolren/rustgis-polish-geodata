---
name: polish-geodata
description: Polish national geodata in RustGIS: raster overlays, WCS coverages, vector WFS datasets downloaded as layers, the county EGiB, a 3 296-address service register, 1 m elevation models with 3D terrain, and ULDK, UUG and NMT lookups. Use for Polish parcels, addresses, boundaries, orthophoto, protected areas, geology, elevation, and anything "z Geoportalu" or "z GUGiK".
---

# Polish geodata

Based on the plugin's README. Commands are `plugin:polish-geodata/<id>`, tools
`plugin:polish-geodata/<id>`. Every command declares its arguments, so `search_commands` and
`rustgis-cli commands` show them.

## Two halves

A **command** shows something at once: it adds a tile layer or a layer from ULDK. A **tool** runs in
the background with progress and Cancel, and returns layers. The elevation and WCS commands are the
exception: they download inside the command, because a tool cannot add a raster layer.

## Area first

Every download takes an area layer, a polygon layer on the map. The layer is "Area to download"
(Polish "Obszar pobierania"). A selection on it narrows the download.

| Command | What it sets |
| --- | --- |
| `obszar-widok` | the current view, as it is |
| `obszar-json` | `{"bbox": [west, south, east, north]}` or `{"geojson": <Feature>}` |
| `obszar-jednostka` | `{"nazwa": "Poznań", "poziom": "gmina"}`; levels are `gmina`, `powiat`, `wojewodztwo`, `obreb`, `miasto` |
| `obszar-jednostka` + `przytnij` | the same, and a named layer is cut to that unit in the same run |
| `obszar-drzewo` | `{"unit": "1416"}`, a TERYT code from the panel's tree: 2 digits a voivodeship, 4 a powiat, 7 a gmina |
| `granice` (tool) | form fields: Level, Voivodeship, Powiat or gmina, Name filter, Size limit; the name filter runs on the register |
| `granice` | a bare call answers the country's voivodeships; the panel's Boundaries tree adds the other levels, with the chosen unit read from the unit tree |
| `granice` (tool) | form fields: Level, Voivodeship, Powiat or gmina, Name filter, Size limit; the name filter runs on the register |
| `granice` | `{"poziom": "gminy", "zakres": "1416", "filtr": "ostr"}`: one level of state boundaries for the country or a TERYT code, names filtered accents aside |
| `pobierz-typ` | `{"dane": "Punkty adresowe", "unit": "1464011"}`, one call: the unit sets the area, the dataset downloads and the layer comes cut to the unit |
| `obszar-rysuj` | arms the polygon tool; a drawn shape lands in the area |
| `obszar-prostokat` | arms the app's rectangle tool, which draws a graphic |
| `obszar-przejmij` | moves that graphic into the area layer |
| `obszar-anuluj` | drops a pending drawing |
| `obszar-wyczysc` | removes the area layer |
| `obszar-dodaj` | copies the current area to Area 1, Area 2, or takes `{"nazwa": "Gmina A", "bbox": [w, s, e, n]}`; no ribbon button |

A city that PRG lists only as a gmina falls back to its gmina boundary. An inverted or short box is
refused with the shape the command wants.

## Batch over several areas

`obszar-dodaj` has no ribbon button; a script or the assistant calls it to copy the current area to a
layer of its own (Area 1, Area 2). `gp.batch` then runs one download tool over all of them:

```text
run_tool  gp.batch  {"tool": "plugin:polish-geodata/pobierz-granice", "inputs": ["Gmina A", "Gmina B"], "params": {"dane": "A03_Granice_gmin"}}
```

One layer comes back per input, in one background run with progress and Cancel.

## Downloads

`run_tool` with a tool id and `{"dane": <dataset id>, "obszar": <area layer>}`. Read the catalogue
first: `run_command plugin:polish-geodata/katalog {}` lists every dataset's id, label, tool and
limit.

| Tool | What it takes |
| --- | --- |
| `pobierz-adresy` | address points, streets, buildings |
| `pobierz-granice` | PRG units, obręby, EGiB boundaries |
| `pobierz-osnowy` | control networks |
| `pobierz-przejazdy` | level crossings, PKP PLK and the other operators |
| `pobierz-urzedy` | offices, courts, health facilities |
| `pobierz-pozostale` | the rest, including monuments and maritime data |
| `pobierz-gdos` | 15 protected-area types: parks, reserves, Natura 2000, landscape parks |
| `pobierz-arkusze`, `pobierz-skorowidz-orto`, `pobierz-skorowidz-nmt`, `pobierz-skorowidz-inne` | sheet indexes and the files their features point at |
| `aktualizuj-rejestr` | reads the EZiUDP register again and writes the new addresses |

Limits the tools enforce:

- 10 000 features per run by default, 50 000 with `confirm`; a bigger area gets the count and a refusal.
- A small `max_span` dataset refuses a wide area by name.
- 25 MB per run unless the panel raises it; the slider sets 5 to 500 MB, the hard ceiling is 256 MB.
- An area the service counts at zero stops before the first page. The Osnowa services ignore the area:
  the plugin fetches the whole set (under 7 000 points), cuts it to the area's box, and says so.

The result is a layer named after the dataset, in one undo step. A WFS filter returns whole features
that touch the area, so **Clip to layer** (`przytnij`) is what cuts them to the boundary.

## County parcels and any register service

- `dzialki-powiat {"limit": 20000}` reads ULDK for the county, the register for its EGiB WFS, and
  pages one bounded request per type. It tries every WFS the county reports before it refuses.
- `rejestr-pobierz {"usluga": "<register id>", "typy": "dzialki,budynki"}` reads any WFS the register
  holds the same way. A server that refuses WFS 2.0.0 is asked again as 1.1.0.

## Register of services

- `zrodla {"szukaj": "geologia"}` searches 3 296 proved addresses by dataset, institution and TERYT.
- `zrodla {"dodaj": "<id>"}` adds one service as a tile layer.
- `rejestr-widok {"rodzaj": "wfs"}` narrows the register to the county under the map view.
- `bdot500` and `gesut` take the gmina or county under the view, add the most specific register WMS
  (gmina, county, then national K-GESUT), or name the WFS for `rejestr-pobierz`.
- `eziudp {"instytucja": "GIOŚ"}` asks the live register, which needs a filter: the whole register is
  9 MB.
- The panel's **Mapa** tree has a Register EZiUDP root: institution, then its WMS.

## Elevation

- `dem`, `dem-evr`, `dem-nmpt` add a 1 m model from the area: `{"area": "<layer>"}` optional.
- `look` is `colours`, `classes` or `shading`; `terrain` is on by default; `exaggeration` runs 0 to 20.
- A model becomes the 3D scene's ground with `map.terrain`, and the message names the height range.
- `dem-terrain-off` takes the model off the ground again.
- `dem-style {"layer": "…", "look": "classes"}` restyles a model already on the map.
- A big grid waits for a second press within 120 s; the first press counts cells, MB and seconds.

## Coverages (WCS)

`wcs-<id>` commands add one of eight GUGiK coverages over the area, as a raster layer:

| Command | Coverage | Format |
| --- | --- | --- |
| `wcs-orto-standard`, `wcs-orto-hi-res`, `wcs-orto-true` | the orthophoto series | GeoTIFF |
| `wcs-nmt-kron86`, `wcs-nmt-evrf2007` | the terrain model in PL-KRON86-NH or PL-EVRF2007-NH | ASCII grid |
| `wcs-nmt-kron86-tiff` | the terrain model as the publisher's float32 GeoTIFF | GeoTIFF |
| `wcs-nmpt-kron86`, `wcs-nmpt-evrf2007` | the surface model in either datum | ASCII grid |

- The area comes from a selection on the active layer, the drawn area layer, or the active vector
  layer, as the elevation model does. An area whose centre is outside the coverage's bounds is
  refused, with the bounds named.
- `{"service": "<command or label>", "rozdzielczosc": 1}`: the cell follows the area;
  `rozdzielczosc` asks for a coarser cell in metres. A finer cell than the 12 s budget allows is
  refused with the cell that fits.
- A height model gets `look` (`colours` by default, `classes` or `shading`); an orthophoto lands as
  it is. A model layer is named `NMT <cell> m <datum>`, so `dem-style` finds it.
- An ASCII coverage is converted once into a float32 GeoTIFF with EPSG:2180 in the plugin's cache
  (4.9 MB instead of 28); later adds and project opens read that. A grid with a no-data value stays
  text, with a `.prj`.
- `katalog` lists the eight under `coverages`, with their coverage ids.

## Lookups

| Command | What it answers |
| --- | --- |
| `geokoduj {"adres": "Poznań, Głogowska 10"}` | a point for an address |
| `adres-odwrotnie {"lon": 21.02, "lat": 52.23}` | the address at a point, else under the map centre |
| `wysokosc {"x": 356000, "y": 487000}` | the terrain height in PL-KRON86-NH |
| `dzialka-id {"id": "306401_1.0001.14/4"}` | a parcel boundary by ULDK identifier |
| `parcel` | the parcel under the map centre |
| `obreby {"teryt": "306401_1"}` | the cadastral obręby of a unit, with the codes parcels use |

## CSV tools

Both take a file through the host: the tool receives the bytes in `files`, and the parameter holds
the name. A script may pass `{"name": "x.csv", "data": <base64>}` instead.

- `geokoduj-csv {"file": …, "kolumna": "adres", "limit": 500}` geocodes a CSV with UUG and returns one
  point layer, plus a count of the addresses it did not find.
- `dzialki-csv {"file": …, "kolumna": "id", "limit": 200}` checks ULDK identifiers and returns the
  parcels it found, plus a count of the misses.

## Clip, panel, settings

- `przytnij {"layer": "<layer>", "obszar": "<boundary>"}` cuts a layer to a boundary. A selection on
  the boundary layer is copied to a temporary layer, so the cut follows the selected shapes.
- `panel-state` shows the view, the area, the boundary pair, the layers, the catalogue and the stored
  answers.
- `mapa {"panel": {"mapa": "v:orto"}}` adds a map from the panel's list: `v:<command>` is a map,
  `d:<tool>:<dataset>` runs a download. A folder row answers what to pick instead.
- `ustawienia {"potwierdz-duze": true, "limit-mb": 50}` stores the download size limit.
- `wyczysc-cache` clears the stored answers and the downloaded models.
- `zoom-polska` brings the map to Poland, the only extent GUGiK serves.

## Maps and Water

`katalog` lists the overlays: 112 WMS, including GUGiK (76), historical maps, PIG geology, NID, BDL,
IMGW, GUS, Wody Polskie (32 services) and Cartomatic (45 maps). Each is one command that adds a tile
layer, and a second run of the same command removes it. The same list holds the eight WCS coverages
under `coverages`; those add a raster of the area instead.

`wody-warstwy {"usluga": "wody-apgw"}` lists a service's layers with their titles, and
`wody-apgw {"warstwa": "Główne rzeki"}` puts one layer on the map instead of all of them.

## Habits that avoid wasted runs

- Read `katalog` before a download; a wrong dataset id names itself in the refusal.
- Make the area as small as the question needs: parcels weigh about 1.3 KB each, a commune boundary
  about 140 KB.
- Ask for a refusal to be read: every limit message names the measured size and the next step.
- A repeated run is cheap: answers are cached for seven days, pages are streamed and parsed as they
  arrive.
