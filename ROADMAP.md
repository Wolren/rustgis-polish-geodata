# Roadmap

_Stan na 2026-11-08, wtyczka 0.6.0 plus rejestr EZiUDP._

Cel: jedna wtyczka do RustGIS, która kondensuje polskie dane, dzieli je najpierw według instytucji,
potem według tematu, i którą całą da się sterować przez MCP.

## Zrobione

### Zakładka i zasada

- Jedna zasada trzyma wstęgę: polecenie pokazuje coś od razu, narzędzie pobiera w tle z postępem
  i przyciskiem Anuluj. Wyjątkiem są trzy polecenia modeli wysokości, bo narzędzie nie umie dodać
  warstwy rastrowej.
- 223 polecenia (37 własnych, 111 usług katalogu, 30 Wody Polskie, 45 Cartomatic, 6 rejestru)
  i 11 narzędzi. Grupy w układzie `small`, katalog zwinięty według instytucji, 10 kolumn.
  Zmierzone: treść kończy się na 1290 px w oknie 1366 px, bez przewijania w poziomie.
- Każde polecenie deklaruje argumenty w `params`. `katalog` wypisuje usługi, zbiory i identyfikatory
  dla agenta.

### Dane

- **Katalog WMS**: 109 usług GUGiK i innych instytucji plus dwie własne warstwy PIG. Każda
  sprawdzona prawdziwym kaflem przez `tools/services.py`.
- **WFS**: 106 zbiorów mapowych, 162 skorowidze, 15 typów obszarów chronionych GDOŚ. Pobieranie
  przez sześć narzędzi i narzędzie ochrony przyrody, plus cztery narzędzia paczek (skorowidz jako
  warstwa i pliki z pola `url_do_pobrania`).
- **Rejestr EZiUDP offline**: `data/eziudp.json`, 3 296 adresów, każdy dowiedziony żądaniem przy
  budowie: 1 742 WMS od 570 instytucji i 1 554 WFS. `tools/eziudp.py` grupuje adresy i sprawdza
  je równolegle, z ponowieniem w WMS 1.1.1.
- **Rejestr na żywo**: `eziudp` pyta `api.php` i odmawia bez filtra, bo cały rejestr to 9 MB.
  Zawężone zapytanie wraca w całości.
- **Menu Rejestr**: sześć usług, których katalog nie ma, dowiedzionych przez budowniczego: GIOŚ
  (hałas, powietrze, wody podziemne) i własne EGiB Poznania, Wrocławia i Warszawy.
- **Aktualizacja rejestru w locie**: narzędzie `aktualizuj-rejestr` czyta rejestr, sprawdza tylko
  nowe adresy i zapisuje plik obok cache, który ma pierwszeństwo przed kopią w binarium. Zapis przez
  plik tymczasowy, więc przerwanie zostawia stary rejestr.

### Obszar, pobieranie, granica

- Obszar to zawsze warstwa: prostokąt z widoku, rysowanie, prostokąt z narzędzia aplikacji
  (`obszar-prostokat` i `obszar-przejmij`), anulowanie i usuwanie. `obszar-json` bierze `bbox`
  albo GeoJSON, więc skrypt ustawia obszar bez rysowania. Wtyczka nie zgaduje obszaru z aktywnej
  warstwy.
- Filtr WFS zwraca całe obiekty, więc po pobraniu jest **Przytnij do warstwy**: wbudowany Clip
  zastępuje warstwę wynikiem. Granicą jest warstwa wybrana w drzewie, domyślnie warstwa obszaru.
- **Duża siatka wysokości**: pierwsze naciśnięcie liczy komórki, MB i sekundy i nic nie pobiera.
  Drugie, w ciągu 120 s, pobiera. Zmierzone na 1,6 x 2,1 km: 1559 x 2211 komórek i 70,3 MB.
- **Bramka rozmiaru**: pobranie powyżej limitu pyta najpierw. Limit ma domyślnie 25 MB, a suwak w
  panelu pozwala 5 do 500 MB. Twardy sufit to 256 MB. Zaznaczenie w panelu odpowiada za wszystkie
  następne przebiegi bez pytania.
- Limit pobrania: 6 000 obiektów, zasięg 0,15° dla działek i budynków, 20° dla granic.

### Panel i dialog

- Panel ma statusy (widok, obszar, para „warstwa → granica”), drzewo warstw granicznych, drzewo
  **Mapa** z trzema korzeniami (podkłady, pobieranie, rejestr EZiUDP), suwak i zaznaczenie limitu,
  katalog i zapisane odpowiedzi. Wybór wiersza folderu odpowiada podpowiedzią zamiast błędu.
- `mapa` uruchamia wiersz drzewa, `ustawienia` zapisuje wartości panelu, `panel-state` czyta stan.

### Wody Polskie

- 30 usług, 875 tytułów warstw pobranych z `GetCapabilities` i zapisanych w `data/wody.json`.
  `wody-warstwy` wypisuje warstwy usługi z tytułami, a parametr `warstwa` kładzie jedną warstwę
  zamiast wszystkich.

### Wyszukiwania i asystent

- `geokoduj` (UUG), `adres-odwrotnie` (UUG, z `lon` i `lat`), `wysokosc` (NMT), `dzialka-id`
  (ULDK), `parcel` (działka pod środkiem mapy).
- `skills/dane-pl/SKILL.md` w paczce, więc asystent widzi ją jako `dane-pl:dane-pl`.

### Języki i build

- Angielski jest bazą, polski tłumaczeniem: 178 wpisów `i18n.pl`, zero braków i zero sierot.
  Generator odrzuca build w obu tych przypadkach.
- Manifest generowany z `data/` i `tools/manifest.base.json`, z `--check` dla CI. Budowniczowie
  dowodzą każdy adres, który zapisują.

## Następne

### Sterowanie zdalne (MCP)

- **Brak polecenia otwierającego panel.** Treść panelu czyta `panel-state`, ale samego okna agent
  nie otworzy. Wymaga pola w manifeście, którego host 0.5.0 nie ma.
- **Okno parametrów polecenia** ([#1547](https://github.com/opengeos/RustGIS/issues/1547), otwarte,
  bez PR). Po wejściu `geokoduj`, `dzialka-id`, `eziudp` i wiersz rejestru dostaną formularz ze
  wstęgi. Obszar bez rysowania już działa przez `obszar-json`.
- **Dialog zamiast dwóch naciśnięć.** Duża siatka wysokości prosi o drugie naciśnięcie. Po #1547
  to samo miejsce dostanie okno z MB, sekundami i przyciskami OK/Anuluj.

### Dane

- **Brakujące usługi narodowe.** Rejestr zna IMGW 3 zbiory (mamy 1), GUS 2 (mamy 1). GUNB i KOWR
  rejestr nie zna pod tą nazwą. Wszystko przez `tools/services.py` z weryfikacją kaflem.
- **Rejestr: instytucje bez WMS.** 1 554 adresy WFS są w rejestrze, ale nie mają ścieżki pobierania
  po typie. Wymaga odczytu `GetCapabilities` i wspólnego szablonu z narzędziami WFS.

### Wyszukiwanie i ULDK

- **ULDK po obrębie**: kaskada województwo → powiat → gmina → obręb → numer, import z CSV i kontrola
  całej warstwy działek.
- **Obszar z jednostki administracyjnej**: zrobione jako `obszar-jednostka` dla gminy, powiatu,
  województwa, obrębu i miasta, z opcjonalnym `przytnij`. Miasto nieobecne w warstwie miast wraca
  jako gmina.

### Host (RustGIS)

- **Menu kontekstowe dla wtyczek**: `commands[].contexts` w manifeście plus wpisy w `mapmenu.rs`
  i `layer_menu`. Bez tego pobieranie spod prawego przycisku jest niemożliwe.
- **`createRectangle` dla warstw edytowalnych**: prostokąt z narzędzia aplikacji rysuje grafikę,
  więc obszar wymaga kroku **Prostokąt do obszaru**.

## Fuzz pobierania

`tools/fuzz.py` uruchamia prawdziwe pobrania z ziarnem i limitem czasu (`--cases`, `--seed`,
`--timeout`, `--report`) i klasyfikuje wynik na OK, REFUSED, TIMEOUT i CRASH. Trzy przebiegi po
9 do 12 przypadków nie dały ani jednego crashu i ani jednego timeoutu.

Co wykrył i co z tego wynika:

- **Naprawione: umowa pliku w narzędziu.** Gospodarz podaje plik w `files` jako base64, a parametr
  trzyma samą nazwę, więc `geokoduj-csv` czytał ścieżkę i kończył się błędem „fuzz.csv could not be
  written”. Teraz czyta `files`, a ścieżkę traktuje jako wariant zapasowy.
- **Naprawione: powiat z nie swoją usługą.** Rejestr podaje dla Poznania adres „EGiB”, który
  naprawdę serwuje EMUiA, więc polecenie odmawiało komunikatem o braku typów. Teraz próbuje
  wszystkich WFS-ów powiatu, zanim odmówi.
- **Do poprawy: bramka rozmiaru mierzy po pobraniu.** Obszar 2° pobrał 32 MB w 15 s i dopiero wtedy
  odmówił przy 25 MB. Usprawnienie: oszacować rozmiar przed żądaniem (zasięg × zmierzona gęstość
  z `tools/wfs.py`) albo spytać przed startem.
- **Do poprawy: brak oceny gęstości w teście.** Fuzzer mierzy czas i werdykt, ale nie sprawdza, czy
  liczba obiektów zgadza się z zasięgiem. Usprawnienie: porównać obiekty na km² z prognozą.

## Do przeniesienia z 0.7.0

Gałąź `backup/v0.7.0` ma commit `50ee781`, który odszedł od tego samego commita bazowego `291f358`.
Scalanie odłożone: konflikt w 11 plikach, nasza linia zostaje główna. Do przeniesienia, w kolejności
wartości:

1. **`src/egib.rs`: działki i budynki wprost z powiatu.** ULDK `GetCountyByXY` nazywa powiat,
   EZiUDP daje jego WFS EGiB, a `GetCapabilities` daje typy. Ograniczony `GetFeature` na typ kładzie
   warstwę wektorową zamiast rastra KIEG. Wymaga `quick-xml` oraz `src/gml.rs`, `src/filter.rs`
   i `src/gk92.rs` z tej gałęzi.
2. **`src/wody.rs` i `data/wody.json`: 32 usługi** z wyborem warstwy po stronie gałęzi. Nasza
   linia ma 30 usług, 875 tytułów warstw i parametr `warstwa`, więc zostaje tylko nowszy katalog.
3. **`src/clip.rs`: granica z zaznaczenia**, kopiowana do warstwy tymczasowej, plus znacznik
   `from_view`.
4. **`src/nmt.rs`: teren 3D i wygląd modelu** (`set_terrain`, `height_range`, `Look`,
   `exaggeration`, `dem-terrain-off`).
5. **`src/eziudp.rs`: `rejestr-widok`**, rejestr zawężony do powiatu z widoku.
6. **`tools/probe.py`, `tools/smoke.py`, `tools/fixtures/*.geojson`** i workflow katalogów w CI.

Nie przenosimy: ich `main.rs` i `text.rs` (nasze mają więcej) ani usunięcia Cartomatic ze wstęgi.
