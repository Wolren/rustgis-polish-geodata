# Roadmap

_Stan na 2026-10-30, wtyczka 0.7.0._

Cel: jedna wtyczka do RustGIS, która kondensuje polskie dane — podzielone najpierw według
instytucji, potem według tematu — i którą w całości da się sterować przez MCP.

## Zrobione (0.7.0)

- **Wody Polskie: 32 usługi i 904 warstwy** — największa luka w katalogu. Jedno polecenie na usługę,
  warstwa jako parametr (`warstwa`), katalog warstw w `data/wody.json` (bo ArcGIS tytułuje warstwy
  numerami) i polecenie **Warstwy usługi** do szukania nazw. Usługi przyjmują 3857, sprawdzone
  realnym żądaniem kafla.
- **Model wysokości jest terenem, nie obrazkiem.** Model wjeżdża w barwach wysokości (ciągła rampa
  hipsometryczna), a `map.terrain` ustawia go jako teren sceny 3D — wcześniej przechylona mapa
  zostawała płaska, bo nikt nie powiedział aplikacji, z którego rasteru brać wysokości. Parametry:
  `look` (`colours` / `classes` / `shading`), `exaggeration` 0–20, `terrain: false`; **Teren wyłącz**
  wypłaszcza scenę, gdy model zniknął z projektu.
- **Działki z widoku** — ULDK mówi, w jakim powiecie leży środek mapy, EZiUDP podaje WFS EGiB tego
  powiatu, a `GetFeature` z prostokątem widoku (limit 5000) daje warstwy działek i budynków.
  Powiaty bez GeoJSON (217 z 380) działają przez GML, także w układzie 1992 — powiat poznański:
  1447 działek i 2054 budynki jednym żądaniem, Warszawa: 247 i 142.
- **Audyt i test dymny.** `tools/probe.py` sprawdza 107 usług i 7 wbudowanych nakładek realnym
  żądaniem kafla w pięciu miejscach i trzech skalach; wykrył i usunął dwie martwe usługi
  (`PZGIK/ORTO/WMS/StandardResolution` — 404, `PZGIK/NMT/GRID1/WMS/Hypsometry` — tylko 2180).
  `tools/smoke.py` uruchamia 165 poleceń w prywatnej sesji, a workflow co tydzień powtarza sondę.
- **Wstęga w ośmiu grupach**: katalog rozdzielony na **GUGiK** i **Instytucje**, Wody Polskie mają
  własną grupę z czterema menu, Obszar wszedł do Narzędzi. Dziewięć menu w jednej grupie wypychało
  Pobieranie, Wysokości i Wyszukiwanie za strzałkę przewijania.
- **Rejestr dla widoku** — EZiUDP dla powiatu pod środkiem mapy, bez wpisywania TERYT; przycisk
  w panelu. Panel przyjmuje tylko tekst, checkbox, suwak i przycisk, więc pól do wpisywania nie ma
  i `geokoduj`/`dzialka-id` pozostają poleceniami dla skryptów i agentów.
- **Przytnij do granicy** — granica jest zawsze warstwą: `obszar`/`granica`, a bez niej warstwa
  obszaru. Zaznaczenie na warstwie granicy liczy się samo (jedna gmina z 2478), a gdy granicą jest
  prostokąt z widoku, komunikat mówi to wprost. Wcześniej przycisk „Przytnij do obszaru” brał
  bieżącą warstwę obszaru, czyli zwykle prostokąt z widoku, i nie było widać, że to on.
- **Obszar z widoku, nie „prostokąt”** — przycisk nazywał się „Obszar prostokąt”, choć nie rysuje
  prostokąta, tylko kopiuje widok. Teraz nazywa się **Obszar z widoku** i mówi, że to nie narzędzie
  rysowania; prawdziwy prostokąt wymaga `createRectangle` w hoście (patrz niżej).

## Zrobione (0.6.0)

- Pobieranie przestało być 108 poleceniami na wstędze: pięć narzędzi w tle, z postępem i
  przerwaniem, jedna lista zestawów na narzędzie (`data/wfs.json` zna swoje narzędzie).
- Obszar: prostokąt z widoku, rysowanie, anulowanie, czyszczenie. Bez łańcucha zgadywania.
- Wstęga mówi, co robi: **WMS (podgląd)** tylko wyświetla, **Pobieranie** tylko ściąga.
- Katalog generowany z danych (`tools/manifest.py` + `tools/manifest.base.json`), instytucja
  i menu siedzą w `data/services.json`.
- `katalog` — cały katalog jako JSON dla agentów.
- `geokoduj` (UUG), `wysokosc` (NMT API), `dzialka-id` (ULDK po identyfikatorze),
  `eziudp` (krajowy rejestr zbiorów i usług).
- 109 usług WMS, w tym 11 arkuszy historycznych (WIG, mapy niemieckie) z Cartomatic.

## Następne

### Dane

- **Wody Polskie: geobazy do pobrania** (PGW, WORP, PZRP, PPSS, MZP) — pliki, nie WMS.
- **Pozostałe warstwy Cartomatic** — serwer podaje 110 warstw, wciągnięto 11 (WIG i mapy
  niemieckie). Zostały plany miast i mapy katastralne.
- **Wody/geologia/ochrona środowiska** — przejrzeć EZiUDP po instytucjach i dobrać brakujące
  usługi narodowe; wszystko przez `tools/services.py` (weryfikacja realnym kafliem).
- **Wody Polskie: warstwy z capabilities, nie z danych** — `data/wody.json` pochodzi z wtyczki QGIS
  i trzeba go odświeżać ręcznie; lista z `GetCapabilities` byłaby zawsze aktualna, ale ArcGIS
  tytułuje warstwy numerami, więc zostaje pytanie, skąd brać czytelne nazwy.

### Wyszukiwanie i ULDK

- **Geokodowanie z pliku CSV** — narzędzie z parametrem `file` (CSV → warstwa punktowa),
  tak jak „Geokodowanie Adresów UUG GUGiK". Dziś `geokoduj` obsługuje jeden adres.
- **ULDK po obrębie** — wybór z listy województwo → powiat → gmina → obręb → numer, oraz
  import z CSV i „Sprawdź warstwę" (walidacja całej warstwy działek). Wzorzec: wtyczka
  GIS Support.
- **Obszar z jednostki administracyjnej** — gmina/powiat jako obszar pobierania, bez rysowania.
  Wymaga kaskady (dwa narzędzia: województwo, potem jednostka po nazwie), bo formularz
  narzędzi nie ma pól zależnych.

### Pobieranie paczek

- **LAZ / NMT / ortofoto jako paczki** — parytet z „Pobieracz danych GUGiK": wybór arkuszy
  z warstwy skorowidza (te 162 typy `index` w katalogu WFS są właśnie po to), pobranie na
  dysk, raport CSV. Osobna architektura: indeks + wiele arkuszy + zapis plików.
- **Wybór skorowidza w interfejsie** — 162 typy są dziś osiągalne tylko po identyfikatorze.

### Host (RustGIS)

- **Menu kontekstowe dla wtyczek** — `commands[].contexts` w manifeście plus wpisy w
  `mapmenu.rs` i `layer_menu`. Bez tego pobieranie „tutaj" spod prawego przycisku jest
  niemożliwe; wtyczka nie może dodać własnej pozycji.
- **Narzędzie prostokąta dla warstw edytowalnych** (`createRectangle`) — dziś przeciąganie
  prostokąta istnieje tylko dla grafiki. Prostokąt obszaru bierzemy z widoku.
- **Okno parametrów polecenia** — polecenia bez argumentów nie da się uruchomić z wstęgi,
  dlatego `geokoduj`, `dzialka-id` i `eziudp` są tylko dla skryptów i agentów. Panel tego nie
  zastąpi: przyjmuje tekst, checkbox, suwak i przycisk, żadnego pola do wpisania wartości.
  Częściowo obejściem jest **Rejestr dla widoku** (EZiUDP po TERYT z ULDK, bez wpisywania).

### Interfejs

- **`eziudp` → dodanie usługi z rejestru.** Rejestr zna adresy WMS/WFS; wtyczka mogłaby
  dodać usługę spoza katalogu, gdyby host przyjmował szablon kafli zbudowany w locie.
  To samo dotyczy „Integratora usług danych przestrzennych" z przeglądu wtyczek 2024.
