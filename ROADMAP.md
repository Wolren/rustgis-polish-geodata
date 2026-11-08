# Roadmap

_Stan na 2026-11-08, wtyczka 0.1.0, repozytorium publiczne, CI zielone._

Cel: jedna wtyczka do RustGIS, która kondensuje polskie dane, dzieli je najpierw według instytucji,
potem według tematu, i którą całą da się sterować przez MCP.

## Zrobione

### Zakładka i zasada

- Jedna zasada trzyma wstęgę: polecenie pokazuje coś od razu, narzędzie pobiera w tle z postępem
  i przyciskiem Anuluj. Wyjątkiem są trzy polecenia modeli wysokości, bo narzędzie nie umie dodać
  warstwy rastrowej.
- 236 poleceń i 13 narzędzi. Grupy w układzie `small`, katalog zwinięty według instytucji.
  Zmierzone: treść kończy się na 1290 px w oknie 1366 px, bez przewijania w poziomie.
- Każde polecenie deklaruje argumenty w `params`. `katalog` wypisuje usługi, zbiory i identyfikatory
  dla agenta.

### Dane

- **Katalog WMS**: 112 usług, w tym GUGiK (76), mapy historyczne (11), geologia PIG (11), NID (4),
  BDL (4), IMGW (4), GUS (2), Wody Polskie (32) i Cartomatic (45). Każda sprawdzona kaflem przez
  `tools/services.py` i `tools/probe.py`.
- **WFS**: 106 zbiorów mapowych, 162 skorowidze, 15 typów obszarów chronionych GDOŚ. Pobieranie przez
  sześć narzędzi, narzędzie ochrony przyrody i cztery narzędzia paczek.
- **Rejestr EZiUDP offline**: `data/eziudp.json`, 3 296 adresów, każdy dowiedziony żądaniem przy
  budowie: 1 742 WMS od 570 instytucji i 1 554 WFS.
- **Rejestr na żywo**: `eziudp` pyta `api.php` i odmawia bez filtra, bo cały rejestr to 9 MB.
- **Menu Rejestr**: sześć usług dowiedzionych przez budowniczego: GIOŚ (hałas, powietrze, wody
  podziemne) i EGiB Poznania, Wrocławia i Warszawy.
- **Aktualizacja rejestru w locie**: narzędzie `aktualizuj-rejestr` sprawdza tylko nowe adresy
  i zapisuje plik obok paczki. Zapis przez plik tymczasowy.
- **Każdy WFS z rejestru**: `rejestr-pobierz` czyta `GetCapabilities`, bierze typy z żądania,
  ściąga strony po 1 000 i ponawia żądanie jako WFS 1.1.0, gdy 2.0.0 jest odrzucone.
- **EGiB powiatu**: `dzialki-powiat` nazywa powiat przez ULDK, bierze jego WFS z rejestru i stronicuje
  do 20 000 obiektów. Próbuje każdego WFS-u powiatu, zanim odmówi.

### Obszar, pobieranie, granica

- Obszar to zawsze warstwa: prostokąt z widoku, rysowanie, prostokąt z narzędzia aplikacji, usuwanie.
  `obszar-json` bierze `bbox` albo GeoJSON. `obszar-jednostka` bierze jednostkę, a `przytnij` tnie
  warstwę do niej w tym samym przebiegu.
- `obreby {"teryt": "306401_1"}` wypisuje obręby jednostki z PRG. Zmierzone: Poznań ma 40.
- Filtr WFS zwraca całe obiekty, więc po pobraniu jest **Przytnij do warstwy**: wbudowany Clip
  zastępuje warstwę wynikiem. Zaznaczenie na warstwie granicy idzie do warstwy tymczasowej
  „Granica cięcia”, a ta znika po cięciu.
- **Bramka rozmiaru**: pobranie powyżej limitu pyta najpierw. Limit ma domyślnie 25 MB, suwak pozwala
  5 do 500 MB, sufit to 256 MB. Pusty obszar kończy się przed pierwszą stroną.
- Limit pobrania: 6 000 obiektów, zasięg 0,15° dla działek i budynków, 20° dla granic.

### Pobieranie: szybkość

- Jedna współdzielona sesja HTTP trzyma połączenie: cztery strony adresów spadły z 0,87 s do 0,63 s.
- Gzip: strona gmin z 362 701 do 164 041 bajtów (45 %).
- Pierwsza strona schodzi samotnie, a przy obiektach powyżej 30 KB strona zmniejsza się do 50.
  Obszar 2°: 15 s przed zmianami, 5,2 s po nich, 1,6 s przy ciepłym cache.
- Treść jest parsowana w trakcie czytania, po jednym członie: cztery strony adresów 0,71 s zimne
  i 0,27 s ciepłe, strona gmin 5 MB 1,11 s ciepła. Pamięć ograniczona do jednego członu.
- Cache sparsowanych stron zmierzony jako brak zysku (1,29 s wobec 1,34 s) i nie wszedł do kodu.

### Wyszukiwania, CSV i asystent

- `geokoduj`, `adres-odwrotnie`, `wysokosc`, `dzialka-id`, `parcel`.
- Narzędzia **Geokoduj CSV** i **Sprawdź działki z CSV**: plik idzie w `files` jako base64, wynik to
  jedna warstwa z liczbą braków.
- `skills/polish-geodata/SKILL.md` w paczce, więc asystent widzi ją jako
  `polish-geodata:polish-geodata` i zna całą powierzchnię wtyczki.

### Wysokości

- Trzy modele 1 m, `look` (barwy, pasma, cieniowanie), mnożnik przesady 0 do 20, `map.terrain`
  i `dem-terrain-off`. Sprawdzone: „in bands, heights 63.3 to 91.1 m, set as the 3D terrain”.

### Języki, build, wydanie

- Angielski jest bazą, polski tłumaczeniem: 209 wpisów `i18n.pl`, zero braków i zero sierot.
- Manifest generowany z `data/` i `tools/manifest.base.json`, z `--check` dla CI.
- README po angielsku i `README.pl.md` po polsku, oba pilnowane przez `tools/readme_check.py`.
- Wersja 0.1.0, repozytorium publiczne `Wolren/rustgis-polish-geodata`, CI zielone na Ubuntu, macOS
  i Windows, wydanie buduje trzy platformy (macOS jako uniwersalny binarny przez `lipo`) i sprawdza
  determinizm paczki.
- `tools/fuzz.py`, `tools/probe.py` i `tools/smoke.py` sprawdzają pobieranie, katalog i polecenia.

## Następne

### Do poprawy w kodzie

- **Liczby w `posList`**: największy koszt ciepłego przebiegu to budowa węzłów dla współrzędnych,
  około 0,8 s na 5 MB strony. Usprawnienie: czytać pary liczb wprost z tekstu, bez węzłów.
- **Gęstość w fuzzerze**: mierzy czas i werdykt, ale nie porównuje obiektów na km² z prognozą.
- **Zależne pola w formularzach**: kaskada województwo → powiat → gmina → obręb czeka na hosta.

### Dane

- **GUNB i KOWR**: rejestr ich nie zna pod tą nazwą, więc nie ma czego dodać bez nowego źródła.
- **Więcej WFS**: rejestr ma 1 554 adresy WFS; `rejestr-pobierz` czyta każdy, ale nie ma dla nich
  katalogu z typami i limitami.

### Host (RustGIS)

- **Okno parametrów polecenia** ([#1547](https://github.com/opengeos/RustGIS/issues/1547)): po nim
  `geokoduj`, `dzialka-id`, `eziudp` i wiersz rejestru dostaną formularz ze wstęgi.
- **Polecenie otwierające panel**: treść panelu czyta `panel-state`, samego okna agent nie otworzy.
- **Menu kontekstowe dla wtyczek**: `commands[].contexts` plus wpisy w `mapmenu.rs`.
- **`createRectangle` dla warstw edytowalnych**: prostokąt z narzędzia aplikacji rysuje grafikę,
  więc obszar wymaga kroku **Prostokąt do obszaru**.
- **Dialog zamiast dwóch naciśnięć**: duża siatka wysokości prosi o drugie naciśnięcie; po #1547
  dostanie okno z MB, sekundami i przyciskami.

### Wydanie

- **Wpis w rejestrze**: PR do `opengeos/rustgis-plugins` z `dist/polish-geodata.json`.
- **Tag `v0.1.0`**: uruchamia wydanie i publikuje paczkę razem z wpisem.
