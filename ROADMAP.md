# Roadmap

_Stan na 2026-11-10, wtyczka 0.1.4._

Cel: jedna wtyczka do RustGIS, która kondensuje polskie dane, dzieli je najpierw według instytucji,
potem według tematu, i którą całą da się sterować przez MCP albo przez panel.

## Zrobione

### Zakładka i zasada

- 253 polecenia i 15 narzędzi. Każde polecenie deklaruje argumenty w `params`, a `katalog` wypisuje
  usługi i zbiory dla agenta.
- Jedna zasada trzyma wstęgę: polecenie pokazuje coś od razu, narzędzie pobiera w tle z postępem.

### Usługi i dane

- WMS (112 nakładek, w tym Wody Polskie i Cartomatic), WFS (268 zbiorów krajowych), WCS (8 pokryć
  GUGiK), WMTS (szablon GetTile albo adres capabilities) i dowolne WCS przez `dodaj-wcs`.
- Rejestr EZiUDP: 3 296 usług (1 742 WMS, 1 554 WFS), wyszukiwanie `euziudp`, pobieranie
  `rejestr-pobierz`, polecenia powiatowe `bdot500` i `gesut` (panel, kod TERYT albo widok; WMS jako
  nakładka, sam WFS czytany z własnej usługi).
- Skorowidze, paczki, ortofoto, NMT 1 m i 5 m, CSV geokodowania, ULDK, GDOŚ, PIG.

### Pobieranie

- 10 000 obiektów domyślnie, 50 000 po potwierdzeniu, bez limitu z zaznaczeniem **Bez limitu
  obiektów**; przedstartowy licznik usunięty, przebieg zatrzymuje się na progu albo na limicie MB.
- Cache stron gzip (7 dni, 33 MB dla wszystkich gmin), zapis warstwy do GeoPackage (`zapisz-gpkg`),
  rejestr i WFS powiatowy do 50 000 obiektów.

### CRS i osie

- WFS GUGiK (lat, lon), KINR (lon, lat, 2180), Osnowa (2180 bez filtra), rejestr (2180 albo stopnie).
- Odpowiedź 2180 jest składana w stopnie po pudełku obszaru; para niejednoznaczna zostaje w metrach
  z ostrzeżeniem. Retry pudełka lat-first i normalizacja osi.

### Interfejs i testy

- Panel z drzewem 16 województw, 380 powiatów, 2 479 gmin; obszar z PRG, rysowanie, zaznaczenie,
  przycinanie do granicy. Formularze mają domyślne wartości.
- 192 testy (1 ignorowany), e2e na żywym RustGIS: 25/25 (WCS, obszar, cache, odmowy).
- CI na Ubuntu, macOS i Windows: testy, manifest, README, pakowanie. Paczka zawiera binarkę Windows
  i Linux (cross-build zig); macOS czeka na SDK.

## Następne

### Rejestr WFS

- **Sonda osi** (`tools/eziudp_axis.py`): dla każdej z 1 554 usług WFS zapisuje przyjęty porządek
  pudełka i jednostki odpowiedzi do `data/eziudp-axis.json`, żeby pierwsze żądanie szło od razu we
  właściwym porządku. Skrypt gotowy; pierwsze usługi GIS-Expert zwracają 400 na GetFeature, więc
  builder żądania musi dokładnie powtórzyć `page_url` z `egib.rs` (TYPENAME, MAXFEATURES,
  STARTINDEX, wersja 1.1.0 po 400). Pełny przebieg: około 15-40 minut na 8 wątkach.
- **Typy z rejestru**: 164 z 1 554 adresów mają zapisany typ; `tools/wfs_more.py` uzupełnia resztę
  (około 40 minut).

### Wydanie

- **macOS**: link wymaga SDK (Security, CoreFoundation); lokalnie brak. Ścieżka to runner macOS w CI.
- **Tag 0.2.0**: wymaga zgody; publikuje paczkę i wpis w rejestrze.
- **E2E w CI**: obecny e2e jest harnessem w `D:/Scratch/dsh`; warto przenieść do repo i uruchamiać
  na żywych usługach po harmonogramie.

### Host (RustGIS)

- R1 utrata projektu, R3 nazwy warstw, R6 wyprane style, R12 nagłówki dla usług z poświadczeniami.
- #1547 okno parametrów polecenia; otwieranie panelu z polecenia; menu kontekstowe wtyczek.

### Odłożone

- CSW: krajowy CSW odrzuca zapytania z ograniczeniem (CQL i Filter kończą się wyjątkiem).
- ATOM: linki GUGiK `httpauth` zwracają pusty feed albo 401.
- Poświadczenia: czekają na nagłówki w `map.addTiles` i `service.add`.
