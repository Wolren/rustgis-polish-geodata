# Roadmap

_Stan na 2026-11-09, wtyczka 0.1.4, repozytorium publiczne._

Cel: jedna wtyczka do RustGIS, która kondensuje polskie dane, dzieli je najpierw według instytucji,
potem według tematu, i którą całą da się sterować przez MCP albo przez panel.

## Zrobione

### Zakładka i zasada

- Jedna zasada trzyma wstęgę: polecenie pokazuje coś od razu, narzędzie pobiera w tle z postępem
  i przyciskiem Anuluj. Wyjątkiem są trzy polecenia modeli wysokości, bo narzędzie nie umie dodać
  warstwy rastrowej.
- 239 poleceń i 13 narzędzi. Dziesięć menu WMS w dwóch rzędach po scaleniu: Wody Polskie w jednym
  menu 32 usług, dziedzictwo i lasy w menu instytucji 17, dwa menu GUGiK w jednym 32.
- Każde polecenie deklaruje argumenty w `params`. `katalog` wypisuje usługi, zbiory i identyfikatory
  dla agenta.

### Wybór jednostki

- **Drzewo w panelu**: 16 województw, 380 powiatów i 2 479 gmin z kodem TERYT, offline w paczce
  (`data/jednostki.json`, 113 KB), z wyszukiwaniem. Wybór węzła ustawia obszar z PRG.
- Buduje je `tools/jednostki.py` z PRG; test pilnuje, że każde piętro się rozwiązuje.
- Dwa powiaty noszą nazwę „ostrowski”, więc obszar filtruje PRG po `JPT_KOD_JE`, gdy kod jest znany.

### Jedno wywołanie, jeden typ

- `pobierz-typ {"dane": "Punkty adresowe", "unit": "1464011"}`: kod na jednostkę, granica na obszar,
  pobranie zbioru, cięcie do jednostki i warstwa nazwana typem. Zmierzone: 688 punktów adresowych dla
  gminy Siedlce, 21 gmin dla powiatu ostrowskiego, 59 powiatów dla mazowieckiego.
- Cięcie zostawia obiekt graniczny sąsiada, gdy usługa odpowiedziała prostokątem: liczba jest górną
  granicą. Punkty i linie przechodzą nietknięte, wielokąty bez powierzchni są usuwane.

### Dane

- **Katalog WMS**: 112 usług, w tym GUGiK (76), mapy historyczne (11), geologia PIG (11), NID (4),
  BDL (4), IMGW (4), GUS (2), Wody Polskie (32) i Cartomatic (45). Każda sprawdzona kaflem przez
  `tools/services.py` i `tools/probe.py`.
- **WFS**: 106 zbiorów mapowych, 162 skorowidze, 15 typów obszarów chronionych GDOŚ. Pobieranie przez
  sześć narzędzi, narzędzie ochrony przyrody i cztery narzędzia paczek.
- **Rejestr EZiUDP offline**: `data/eziudp.json`, każdy adres dowiedziony żądaniem przy budowie.
- **Rejestr na żywo**: `eziudp` pyta `api.php` i odmawia bez filtra, bo cały rejestr to 9 MB.
- **Menu Rejestr**: sześć usług dowiedzionych przez budowniczego: GIOŚ (hałas, powietrze, wody
  podziemne) i EGiB Poznania, Wrocławia i Warszawy.
- **Aktualizacja rejestru w locie**: `aktualizuj-rejestr` sprawdza 12 nowych adresów domyślnie,
  jednym podejściem i z twardym terminem 8 s na adres, i zapisuje plik obok paczki przez plik
  tymczasowy. Przebieg domyślny: 7,2 s. Zostało 21 adresów na następny przebieg.
- **Każdy WFS z rejestru**: `rejestr-pobierz` czyta `GetCapabilities`, bierze typy z żądania,
  ściąga strony po 1 000 i ponawia żądanie jako WFS 1.1.0, gdy 2.0.0 jest odrzucone.
- **Typy długiego ogona**: `tools/wfs_more.py` przepytał 200 adresów WFS z rejestru i zapisał typ
  każdego, który odpowiedział, do `data/eziudp-types.json` (164 wpisy, głównie MPZP i SUIKZP gmin).
  `rejestr-pobierz` bierze typ z tego pliku i nie pyta o capabilities, gdy żądanie nie podaje typu.
- **EGiB powiatu**: `dzialki-powiat` nazywa powiat przez ULDK, bierze jego WFS z rejestru i stronicuje
  do 20 000 obiektów. Próbuje każdego WFS-u powiatu, zanim odmówi.

### Obszar, pobieranie, granica

- Obszar to zawsze warstwa: prostokąt z widoku, rysowanie, prostokąt z narzędzia aplikacji, usuwanie.
  `obszar-json` bierze `bbox` albo GeoJSON. `obszar-jednostka` bierze jednostkę, a `przytnij` tnie
  warstwę do niej w tym samym przebiegu. `obszar-drzewo` robi to samo z kodu drzewa.
- `obszar-dodaj` kopiuje bieżący obszar do własnej warstwy (Obszar 1, Obszar 2) i nie ma przycisku:
  woła je skrypt. `gp.batch` przebiega wtedy jednym narzędziem po kilku obszarach. Zmierzone: dwa
  obszary, dwa przebiegi, dwie warstwy.
- `obreby {"teryt": "306401_1"}` wypisuje obręby jednostki z PRG. Zmierzone: Poznań ma 40.
- **Filtr granicy**: `<Intersects>` z jednym lub kilkoma pierścieniami w `<Or>`, skracany do budżetu
  żądania. GUGiK odrzuca skróconą granicę PRG błędem Oracle, więc przebieg mówi o tym i ponawia
  z prostokątem.
- **Bramka rozmiaru**: pobranie powyżej limitu pyta najpierw. Limit ma domyślnie 25 MB, suwak pozwala
  5 do 500 MB, sufit to 256 MB. Pusty obszar kończy się przed pierwszą stroną.
- **Limit rozmiaru w formularzu**: narzędzie pobierające ma własne pole **Limit rozmiaru (MB)**
  5-500, więc panel, który odmawia, sam podnosi limit. Zmierzone: 200 MB nad Mazowieckiem to
  628 gmin w jednym przebiegu.
- Limit pobrania: 10 000 obiektów (50 000 po zaznaczeniu **Pobierz mimo rozmiaru**) i 25 MB
  (256 MB albo suwak, do 500 MB po zaznaczeniu); bramka liczy obiekty w obszarze i odmawia z tą liczbą. Zasięg
  0,15° został tylko dla adresów, ulic, działek i budynków, a 71 warstw rzadkich dostało 20°:
  turbiny wiatrowe nad 4° × 2° to 262 obiekty w jednym przebiegu, budynki nad 1° × 0,3° to 169 818
  i odmowa z tą liczbą.

### Pobieranie: szybkość

- Jedna współdzielona sesja HTTP trzyma połączenie: cztery strony adresów spadły z 0,87 s do 0,63 s.
- Gzip: strona gmin z 362 701 do 164 041 bajtów (45 %).
- **Przeskok stron**: przy obiektach powyżej 30 KB strona zmniejsza się do 50, a offsety kroczą
  teraz tą samą wielkością. Wcześniej kroczyły po 1 000, więc przebieg zbierał okna po 50 obiektów
  z dziurami i kończył na 70-80 % zbioru. Sprawdzone licznikiem usługi: pudełko mazowieckie
  629 obiektów w odpowiedzi `RESULTTYPE=hits` i 629 pobranych.
- Pierwsza strona schodzi samotnie, a przy obiektach powyżej 30 KB strona zmniejsza się do 50.
  Obszar 2°: 15 s przed zmianami, 5,2 s po nich, 1,6 s przy ciepłym cache.
- Treść jest parsowana w trakcie czytania, po jednym członie: cztery strony adresów 0,71 s zimne
  i 0,27 s ciepłe, strona gmin 5 MB 1,11 s ciepła. Pamięć ograniczona do jednego członu.
- **Czytanie par liczb bez listy tokenów sprawdzone i odrzucone**: 0,13 s wobec 0,11 s na stronie
  15,9 MB, bo `split` i `parse` są już zoptymalizowane. Zostaje ścieżka z `split`.
- Cache sparsowanych stron zmierzony jako brak zysku (1,29 s wobec 1,34 s) i nie wszedł do kodu.

### Wyszukiwania, CSV i asystent

- `geokoduj`, `adres-odwrotnie`, `wysokosc`, `dzialka-id`, `parcel`.
- Narzędzia **Geokoduj CSV** i **Sprawdź działki z CSV**: plik idzie w `files` jako base64, wynik to
  jedna warstwa z liczbą braków. Plik jest wymagany, więc wołacz bez pliku dostaje odmowę od razu.
- `skills/polish-geodata/SKILL.md` w paczce, więc asystent widzi ją jako
  `polish-geodata:polish-geodata` i zna całą powierzchnię wtyczki.

### Wysokości

- Trzy modele 1 m, `look` (barwy, pasma, cieniowanie), mnożnik przesady 0 do 20, `map.terrain`
  i `dem-terrain-off`. Sprawdzone: „in bands, heights 63.3 to 91.1 m, set as the 3D terrain”.

### Języki, build, wydanie

- Angielski jest bazą, polski tłumaczeniem: każdy wpis `i18n.pl` ma swoje miejsce, zero braków i zero
  sierot.
- Manifest generowany z `data/` i `tools/manifest.base.json`, z `--check` dla CI.
- README po angielsku i `README.pl.md` po polsku, oba pilnowane przez `tools/readme_check.py`,
  z ośmioma zrzutami ekranu z prawdziwej aplikacji.
- `tools/manifest_check.py` powtarza reguły rejestru i limity hosta, także dla paczki z trzema
  platformami.
- 149 testów. Repozytorium publiczne `Wolren/rustgis-polish-geodata`, CI zielone na Ubuntu, macOS
  i Windows, wydanie buduje trzy platformy (macOS jako uniwersalny binarny przez `lipo`) i sprawdza
  determinizm paczki.
- `tools/fuzz.py`, `tools/probe.py` i `tools/smoke.py` sprawdzają pobieranie, katalog i polecenia.

## Następne

### Do poprawy w kodzie

- **Cięcie do jednostki bez sąsiada**: obiekt, który tylko dotyka granicy, zostaje po cięciu hosta,
  więc liczba dla zbioru granicznego jest górną granicą. Przełącznik **Przytnij pobranie do granicy
  jednostki** w panelu jest domyślnie włączony; wyłączenie zostawia cały prostokąt.
- **Gęstość w fuzzerze**: zrobione. Każde pobranie porównuje obiekty na km² z prognozą
  (`FORECAST`), pasmo 10× w obie strony; pudełko mniejsze niż obiekt jest pomijane.

### Dane

- **GUNB i KOWR**: rejestr ich nie zna pod tą nazwą, więc nie ma czego dodać bez nowego źródła.
- **BDOT10k jako typy**: usługa typowana GUGiK odpowiada 401, więc publiczna droga to paczki powiatowe
  (`BDOT10k - powiaty do pobrania`), nie warstwy per typ.
- **Więcej typów z rejestru**: 164 z 1 554 adresów mają zapisany typ; przebieg na resztę to jedno
  uruchomienie `tools/wfs_more.py`, około 40 minut.

### Host (RustGIS)

- **Okno parametrów polecenia** ([#1547](https://github.com/opengeos/RustGIS/issues/1547)): po nim
  `geokoduj`, `dzialka-id`, `euziudp` i wiersz rejestru dostaną formularz ze wstęgi.
- **Polecenie otwierające panel**: treść panelu czyta `panel-state`, samego okna agent nie otworzy.
- **Menu kontekstowe dla wtyczek**: `commands[].contexts` plus wpisy w `mapmenu.rs`.
- **`createRectangle` dla warstw edytowalnych**: prostokąt z narzędzia aplikacji rysuje grafikę,
  więc obszar wymaga kroku **Prostokąt do obszaru**.
- **Dialog zamiast dwóch naciśnięć**: duża siatka wysokości prosi o drugie naciśnięcie; po #1547
  dostanie okno z MB, sekundami i przyciskami.

### Wydanie

- **Wpis w rejestrze**: PR do `opengeos/rustgis-plugins` z `dist/polish-geodata.json`, czeka na
  przegląd opiekuna.
- **Tag `v0.1.4`**: uruchamia wydanie i publikuje paczkę razem z wpisem.
