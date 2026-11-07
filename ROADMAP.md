# Roadmap

_Stan na 2026-11-07, wtyczka 0.6.0 plus rejestr EZiUDP._

Cel: jedna wtyczka do RustGIS, która kondensuje polskie dane — podzielone najpierw według
instytucji, potem według tematu — i którą w całości da się sterować przez MCP.

## Zrobione (0.6.0)

- Pobieranie przestało być 108 poleceniami na wstędze: pięć narzędzi w tle, z postępem i
  przerwaniem, jedna lista zbiorów na narzędzie (`data/wfs.json` zna swoje narzędzie).
- Obszar: prostokąt z widoku, rysowanie, anulowanie, czyszczenie. Bez łańcucha zgadywania.
- Wstęga mówi, co robi: **WMS (podgląd)** tylko wyświetla, **Pobieranie** tylko ściąga.
- **Wysokości** to jedyny wyjątek od tej zasady: trzy polecenia pobierają raster, bo narzędzie
  nie umie dodać warstwy rastrowej.
- Katalog generowany z danych (`tools/manifest.py` + `tools/manifest.base.json`), instytucja
  i menu siedzą w `data/services.json`.
- `katalog` — cały katalog jako JSON dla agentów.
- `geokoduj` (UUG), `wysokosc` (NMT API), `dzialka-id` (ULDK po identyfikatorze),
  `eziudp` (krajowy rejestr zbiorów i usług).
- 109 usług WMS, w tym 11 arkuszy historycznych (WIG, mapy niemieckie) z Cartomatic.
- Wstęga mieści się w oknie: wszystkie grupy w układzie `small`, katalog zwinięty według
  instytucji (**GUGiK** z czterema tematami w podmenu, pięć menu pozostałych instytucji), a
  pobieranie to pięć osobnych przycisków i menu **Pakiety**. 11 kolumn zamiast 28, a w polskim
  interfejsie treść kończy się na zmierzonych 1390 px z 1480 (zrzut z `ui_shot`), bez strzałki
  przewijania: **Wysokości** i **Wyszukiwanie** to po jednym menu. Wymaga RustGIS 0.5.0
  (zagnieżdżone menu i kolumny `small`), stąd `minRustgisVersion`.
- **Przycinanie do dowolnej granicy.** Panel ma drzewo warstw poligonowych z liczbą obiektów
  i wyszukiwarką; wybór ustawia granicę, a wiersz **Przytnij** pokazuje parę
  „warstwa → granica”. Cięta jest warstwa aktywna, a bez niej najnowsza.
- **Panel bez wypychania przycisków.** Statusy, para „warstwa → granica” i drzewo granicy stoją
  nad przyciskami, a **Odśwież panel** czyta widok, obszar, katalog i zapisane odpowiedzi od nowa.
  Każde narzędzie pobierania ma parametr `clear` zamiast pola wyboru w panelu.
- **Wody Polskie: 30 usług, 890 warstw.** Menu **Więcej map** ma folder **Wody Polskie** z jednym
  poleceniem na usługę: przycisk kładzie na mapę wszystkie warstwy tej usługi, drugie naciśnięcie je
  zdejmuje, a podpowiedź niesie atrybucję i liczbę warstw („PGW Wody Polskie (ISOK) · 11 warstw”).
  Generuje to `tools/manifest.py` z `data/wody.json`, więc nowa usługa trafia na wstęgę sama.
  Nie zrobione: wybór pojedynczej warstwy w środku usługi (jedna usługa to nadal jedno polecenie).
- **Cartomatic: 45 map.** Pozostałe warstwy serwera (plany miast i atlasy, mapy topograficzne
  i przeglądowe) stoją w dwóch folderach tego samego menu, obok jedenastu arkuszy historycznych
  w menu **Mapy historyczne**. Etykieta folderu jest angielska w manifeście, a polska w `i18n`,
  więc oba języki pokazują swoją nazwę serii. Gdy folder przekroczy 32 pozycje, generator dzieli
  go po pierwszym słowie tytułu i wypisuje, co podzielił.
- **Pakiety: skorowidz, folder, CSV.** Menu **Pakiety** w grupie **Pobieranie** ma trzy narzędzia
  skorowidzy (ortofoto 54 zbiory, NMT 44, pozostałe 64; katalog WFS nie ma pola `theme`, więc
  generator czyta temat z tytułu: „ortofotomap”, „pokrycia terenu”, „modelu terenu”; każde
  narzędzie bierze zbiór i warstwę obszaru, jak pięć narzędzi WFS) i czwarte, **Pobierz arkusze**:
  bierze warstwę skorowidza, folder i `clear`, pobiera pliki wskazane w polu `url_do_pobrania`
  zaznaczonych obiektów i pisze raport CSV. Wybór w trzech narzędziach to tytuł zbioru; katalog ma
  już unikalne tytuły, a gdyby się powtórzyły, wybór niesie w nawiasie identyfikator.
  Nie zrobione: LAZ i geobazy z dane.gov.pl (PGW, WORP, PZRP, PPSS, MZP) oraz wybór pojedynczych
  arkuszy w środku usługi.

## Do przeniesienia z 0.7.0

Gałąź `origin/main` ma commit `50ee781` (0.7.0), który odszedł od tego samego commita bazowego
`291f358` co nasza linia. Scalanie odłożone: konflikt w 11 plikach, a nasza linia zostaje główna.
Do przeniesienia wybrane rzeczy, w kolejności wartości:

1. **`src/egib.rs`: działki i budynki wprost z powiatu.** ULDK `GetCountyByXY` nazywa powiat,
   EZiUDP daje jego własny WFS EGiB, `GetCapabilities` daje typy (`wfs:dzialki`, `wfs:budynki`),
   a jeden ograniczony `GetFeature` na typ, przycięty do widoku, kładzie warstwę wektorową.
   To omija ograniczenie krajowego KIEG, który tylko rasteryzuje działki. Wymaga `quick-xml`
   oraz modułów `src/gml.rs`, `src/filter.rs`, `src/gk92.rs` z tej gałęzi.
2. **`src/wody.rs` i `data/wody.json`: 32 usługi i 904 warstwy.** Jedno polecenie na usługę, ale
   parametr `warstwa` wybiera pojedynczą warstwę, a `warstwy` wypisuje, co usługa publikuje.
   Nasza linia ma 30 usług bez wyboru warstwy w środku.
3. **`src/clip.rs`: granica z zaznaczenia.** Warstwa poligonowa z zaznaczeniem jest kopiowana do
   warstwy tymczasowej, więc cięcie idzie po wybranych kształtach. Dochodzi znacznik `from_view`,
   który mówi, że granicą jest prostokąt widoku, a nie rysunek.
4. **`src/nmt.rs`: teren 3D i wygląd modelu.** `set_terrain` stawia scenę 3D na modelu,
   `height_range` podaje min i max, `Look` (barwy, pasma, cieniowanie) z `look_json`, mnożnik
   `exaggeration` i polecenie `dem-terrain-off`. Nasza linia ma tylko barwy albo cieniowanie
   i nie ustawia terenu.
5. **`src/eziudp.rs`: `rejestr-widok`.** Rejestr zawężony do powiatu z widoku, jednym poleceniem.
6. **`tools/probe.py`, `tools/smoke.py`, `tools/fixtures/*.geojson`.** Próba end-to-end i dymna
   z fixturami (obszar, maski, działki), plus `.github/workflows/catalogue.yml` sprawdzający
   katalogi w CI.

Nie przenosimy z 0.7.0: ich wersji `main.rs` i `text.rs` (nasze mają więcej), ani usunięcia
rodzin Cartomatic z wstęgi.

## Następne

### Dane

- **Wody/geologia/ochrona środowiska** - przejrzeć EZiUDP po instytucjach i dobrać brakujące
  usługi narodowe; wszystko przez `tools/services.py` (weryfikacja realnym kafliem). Zmierzone:
  IMGW ma w rejestrze 3 zbiory (mamy 1), GUS 2 (mamy 1), GUNB i KOWR rejestr nie zna pod tą nazwą.

### Wyszukiwanie i ULDK

- **Geokodowanie z pliku CSV** — narzędzie z parametrem `file` (CSV → warstwa punktowa),
  tak jak „Geokodowanie Adresów UUG GUGiK". Dziś `geokoduj` obsługuje jeden adres.
- **ULDK po obrębie** — wybór z listy województwo → powiat → gmina → obręb → numer, oraz
  import z CSV i „Sprawdź warstwę" (walidacja całej warstwy działek). Wzorzec: wtyczka
  GIS Support.
- **Obszar z jednostki administracyjnej** — gmina/powiat jako obszar pobierania, bez rysowania.
  Wymaga kaskady (dwa narzędzia: województwo, potem jednostka po nazwie), bo formularz
  narzędzi nie ma pól zależnych.

### Host (RustGIS)

- **Menu kontekstowe dla wtyczek** — `commands[].contexts` w manifeście plus wpisy w
  `mapmenu.rs` i `layer_menu`. Bez tego pobieranie „tutaj" spod prawego przycisku jest
  niemożliwe; wtyczka nie może dodać własnej pozycji.
- **Narzędzie prostokąta dla warstw edytowalnych** (`createRectangle`) — dziś przeciąganie
  prostokąta istnieje tylko dla grafiki. Prostokąt obszaru bierzemy z widoku.
- **Okno parametrów polecenia** — polecenia bez argumentów nie da się uruchomić z wstęgi,
  dlatego `geokoduj`, `dzialka-id` i `eziudp` są tylko dla skryptów i agentów.

### Interfejs

- **`eziudp` → dodanie usługi z rejestru.** Rejestr zna adresy WMS/WFS; wtyczka mogłaby
  dodać usługę spoza katalogu, gdyby host przyjmował szablon kafli zbudowany w locie.
  To samo dotyczy „Integratora usług danych przestrzennych" z przeglądu wtyczek 2024.
