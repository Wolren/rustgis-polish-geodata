# Dane PL: polskie dane geodezyjne w RustGIS

Wtyczka do [RustGIS](https://rustgis.app): jedna zakładka **Dane PL** z podkładami i danymi
GUGiK, bez kluczy API i bez pośredników.

| Grupa | Przycisk | Co robi |
| --- | --- | --- |
| Podkłady | Ortofoto | Ortofotomapa WMTS (EPSG:3857) jako nakładka |
| | Ortofoto Hi-Res | Ortofotomapa w wysokiej rozdzielczości (WMS) |
| | BDOT10k | Baza Danych Obiektów Topograficznych (WMS, od skali miasta) |
| | Działki (WMS) | Granice działek i budynków z KIEG (od skali ok. 1:5000) |
| | Adresy | Punkty adresowe i ulice |
| | Granice PRG | Państwo, województwa, powiaty, gminy, miasta |
| | Nazwy miejscowości | PRNG |
| | NMT cieniowanie | Cieniowanie numerycznego modelu terenu |
| Obszar | **Obszar prostokąt** | Narzędzie poligonu na warstwie „Obszar pobierania”; po zbudowaniu modelu kształt zamienia się w prostokąt wokół narysowanych punktów |
| | **Obszar poligon** | To samo narzędzie, ale kształt zostaje taki, jak narysowany |
| | **Dwa narożniki** | Narzędzie linii: kliknij dwa przeciwległe narożniki, model obejmie prostokąt między nimi |
| | **Obszar z widoku** | Kopiuje bieżący widok mapy do warstwy obszaru, bez rysowania |
| | **Anuluj obszar** | Wyłącza narzędzie rysowania i wraca do przeglądania |
| | **Usługi** | Osiem menu z katalogiem krajowym; nazwa usługi to osobne polecenie, drugie naciśnięcie zdejmuje ją z mapy |
| Dane | **Pobierz dane…** | Narzędzie (panel Narzędzia): zestaw danych + warstwa obszaru, w tle z postępem i przerwaniem |
| Wysokości | **Model terenu KRON86** | Model terenu 1 m (GeoTIFF) dla zaznaczonego obszaru |
| | **Model terenu EVRF2007** | To samo w PL-EVRF2007-NH (siatka ASCII) |
| | **Model pokrycia NMPT** | Model pokrycia 1 m, z budynkami i drzewami (siatka ASCII) |
| | **Stylizuj model** | Cieniowanie najnowszego modelu, albo barwy |
| Działka | Działka z ULDK | Działka pod środkiem mapy: geometria + atrybuty |
| Narzędzia | **Przytnij do obszaru** | Tnie warstwę do granicy „Obszar pobierania”; bez argumentów bierze najnowszą warstwę |
| | Wyczyść cache | Usuwa zapisane odpowiedzi WFS (domyślnie 7 dni) |

### Usługi krajowe

Katalog obejmuje **97 usług WMS** z Geoportalu Krajowego i od innych instytucji: BDOT10k (w tym nowe
budynki i statystyki), ortofotomapę prawdziwą, wysokiej rozdzielczości i archiwalną, cieniowanie
NMT, hipsometrię NMT, EMUiA, PRG, PRNG, RCN, KIMPZP, ISOK, POLSA, osnowy, mapy geologiczne PIG,
BDL Lasów Państwowych, zabytki NID i inne. Każda pozycja przeszła realne żądanie kafla, więc lista
nie zawiera martwych adresów.

Usługi stoją w grupie **Usługi** na wstędze, w czterech menu: Ortofoto i NMT, Mapy,
Działki i plany, Instytucje.

1. Otwórz grupę **Usługi** i wybierz menu, na przykład **Ortofoto i NMT**.
2. Naciśnij nazwę usługi. Wjeżdża na mapę jako warstwa kafelkowa.
3. Naciśnij ją drugi raz, aby zdjąć ją z mapy.

Każda usługa to osobne polecenie, więc działa też wyszukiwarka poleceń: wpisz „cieniowanie”
albo „ortofoto” i naciśnij wynik. Skrypt albo agent może podać nazwę wprost:

```
run_command  plugin:dane-pl/usluga-cieniowanie-wms
run_command  plugin:dane-pl/usluga  {"service": "Cieniowanie (WMS)"}
```

Listę buduje `tools/services.py`: czyta katalog usług, pobiera GetCapabilities, wybiera warstwy
liści, sprawdza każdą kandydatkę żądaniem GetMap i zapisuje wynik do `data/services.json`. Ten plik
jest wkompilowany w binarkę, więc wtyczka nie czyta capabilities w czasie działania. Lista tylko
rośnie: usługa, która odpowiedziała w którymkolwiek przebiegu, zostaje.

### Dane z WFS

Katalog obejmuje **108 typów WFS** z krajowych usług pobierania: adresy i ulice z KI NA, budynki
i działki z EGiB, granice z PRG, osnowy, urzędy, sądy, kolej, obiekty morskie i statystyki BDOT.
Każdy typ przeszedł realne żądanie GetFeature, więc lista nie zawiera martwych adresów.

Typy stoją w grupie **Dane**, w pięciu menu: Adresy i budynki, Granice i jednostki,
Osnowy i pomiary, Urzędy i sądy, Pozostałe dane.

1. Wskaż obszar, na przykład przyciskiem **Narysuj obszar**.
2. Otwórz grupę **Dane** i wybierz menu, na przykład **Adresy i budynki**.
3. Naciśnij typ danych. Wtyczka pobiera obiekty z obszaru i wjeżdża nimi jako warstwa wektorowa.

Obszar bierze się z tej samej kolejności co dla modelu wysokości: argument `area`, zaznaczenie,
warstwa „Obszar pobierania”, aktywna warstwa.

Typy spoza układu 4326 działają bez ustawień: wtyczka wysyła pierwsze żądanie we współrzędnych
geograficznych, a gdy usługa odpowie pustą kolekcją, powtarza je w układzie 2180 i przelicza
współrzędne z powrotem. Dotyczy to usługi adresowej KI NA.

Skrypt albo agent może podać nazwę wprost:

```
run_command  plugin:dane-pl/dane-punkty-adresowe
run_command  plugin:dane-pl/dane-budynki    {"area": "Obszar pobierania"}
```

### Przycinanie do granicy

Filtr WFS zwraca **całe** obiekty, które dotykają obszaru i nie przycina ich. Dlatego po pobraniu
uruchom **Przytnij do obszaru**: wywołuje wbudowane narzędzie Clip i zastępuje warstwę wersją
przyciętą (jedna warstwa, nie dwie). Uchwyt `layer` jest opcjonalny: bez niego bierze najnowszą
warstwę wektorową, więc po poleceniu „Pobierz dane…” wystarczy jedno kliknięcie.

Pomiar: 5586 działek przecinało obszar, po przycięciu zostało 5583, dla działek różnica jest
mała, dla dróg i obiektów wielkości gminy duża.

### Model wysokości z obszaru

Model powstaje z obszaru, który wskażesz, a nie z widoku mapy. Obszar bierze się z jednego z tych
źródeł, w tej kolejności:

1. warstwa podana w argumencie `area`,
2. zaznaczenie na aktywnej warstwie,
3. zaznaczenie na dowolnej warstwie (wygrywa największe),
4. warstwa „Obszar pobierania” z przycisku **Narysuj obszar**,
5. aktywna warstwa w całości.

Zaznaczenie zawęża model do zaznaczonych obiektów, więc warstwa z wieloma poligonami daje model
tylko dla tych, które zaznaczysz. Warstwa bez zaznaczenia to cały jej zasięg. Widok nie ma tu nic
do rzeczy.

Najkrótsza droga to **Obszar prostokąt**: jedno kliknięcie tworzy warstwę „Obszar pobierania”,
ustawia ją jako aktywną i włącza narzędzie poligonu. Rysujesz obszar i naciskasz przycisk modelu.
Po zbudowaniu modelu warstwa pokazuje prostokąt, który model naprawdę pokrywa: siatka WCS umie
tylko prostokąt. **Obszar poligon** zostawia kształt, **Dwa narożniki** wystarcza na dwa kliknięcia,
a **Obszar z widoku** bierze widok mapy. Ta sama warstwa jest obszarem dla **Pobierz dane…**
i **Przytnij do obszaru**. Narzędzie rysowania wyłącza się samo po zbudowaniu modelu, więc kolejne
kliknięcia na mapie przesuwają mapę, a nie dodają wierzchołki.

Grupa **Wysokości** ma trzy pokrycia:

| Przycisk | Pokrycie | Format |
| Model terenu KRON86 | `DTM_PL-KRON86-NH_TIFF` | GeoTIFF |
| Model terenu EVRF2007 | `DTM_PL-EVRF2007-NH` | siatka ASCII |
| Model pokrycia NMPT | `DSM_PL-KRON86-NH` | siatka ASCII |

GeoTIFF pobiera sam RustGIS i bierze z niego układ odniesienia. Siatka ASCII nie niesie układu, więc
wtyczka pobiera ją sama i dopisuje plik `.prj` z EPSG:2180. Model trafia na mapę z cieniowaniem,
a **Stylizuj model** zmienia je na barwy.

Każde kliknięcie prosi o komórki 1 m: zasięg obszaru idzie do PL-1992 w całych metrach, więc sąsiednie
modele mają tę samą siatkę. Skorowidz arkuszy nie jest potrzebny, bo usługa WCS GUGiK zwraca dowolny
prostokąt.

Pomiar (Poznań, obszar 1,5 x 1,2 km, 27 października 2026): 1538 x 1244 komórek, 7,7 MB, 2 s,
wysokości 50,8 do 76,8 m. Siatka ASCII w 1 m: 632 x 686 komórek, 8,7 MB, 5 s.

Zbyt duży obszar jest odrzucany z liczbami: 192 mln komórek to 769 MB i około 130 s pobierania,
a jedno kliknięcie może trzymać okno 12 s. Limit bierze się z szybkości GUGiK: 5,9 MB/s dla GeoTIFF
i 2,0 MB/s dla ASCII, zmierzonych przy tych samych próbach. Dla większego obszaru wybierz mniejszy
albo pobierz kilka modeli obok siebie. Pobrane siatki ASCII zostają w
`%LOCALAPPDATA%\RustGIS\dane-pl\dem`, a Wyczyść cache usuwa je razem z odpowiedziami WFS.


### Sterowanie z agenta (MCP)

Wtyczka jest dostępna przez MCP bez osobnej warstwy, bo RustGIS uruchamia ją jako polecenia
i narzędzia geoprzetwarzania. Agent używa `run_command` i `run_tool`:

```
run_command  plugin:dane-pl/obszar-rysuj  {}
run_tool     plugin:dane-pl/pobierz       {"dane":"Działki (EGiB)","obszar":"Obszar pobierania"}
run_command  plugin:dane-pl/przytnij      {}
run_command  plugin:dane-pl/dem           {"area":"działki"}
```

`describe_tool` i `search_tools` widzą wtyczkę po instalacji. `script.run` (łączenie kilku
poleceń w jednym wywołaniu) jest zarezerwowane dla użytkownika i agent go nie użyje, dlatego
`przytnij` działa bez argumentów.

Kliknięcie podkładu drugi raz usuwa warstwę. Jeśli widok nie obejmuje Polski, wtyczka najpierw
przybliża mapę do Polski, bo usługi GUGiK pokrywają tylko Polskę i poza nią zwracają puste kafle.

Zakresy skalowe usług (sprawdzone na kaflach 256 i 512 px):

| Warstwa | Widoczna od | Kafel |
| --- | --- | --- |
| BDOT10k | z13 | 512 px (kompozycja rysuje dopiero od tej skali) |
| Działki (KIEG), Adresy | z15 | 256 px |
| Ortofoto, PRG, PRNG, NMT | z0 | 256 px |

### Pobieranie: dwa kliknięcia, bez zamrażania okna

1. **Narysuj obszar**: wtyczka tworzy warstwę „Obszar pobierania”, ustawia ją jako aktywną i włącza narzędzie poligonu. Klikaj narożniki, zakończ dwuklikiem.
2. **Pobierz dane…**: wybierz zestaw danych i warstwę obszaru, uruchom.

Narzędzie działa **w tle** (panel Narzędzia pokazuje postęp i ma przycisk przerwania), bo
polecenia wtyczek wykonują się w wątku interfejsu, a pobieranie jako polecenie zamrażało okno.

Zamiast prostokąta można wskazać własną warstwę (np. wcześniej pobrane granice gminy); jeśli
ma zaznaczenie, liczy się tylko zaznaczenie.

### Ile to trwa i ile waży

GUGiK wysyła geometrię w pełnej rozdzielczości i ignoruje parametry uogólniania
(`MAP_RESOLUTION`, `resolution`), sprawdzone. Dla całego kraju:

| Zestaw | Obiekty | Rozmiar odpowiedzi | Czas serwera |
| --- | --- | --- | --- |
| Województwa | 16 | 8 MB | ~10 s |
| Powiaty | 380 | 42 MB | ~30 s |

Dlatego obszar większy niż 1,5° jest dzielony na 4 części pobierane **równolegle**
(ok. 2–3× szybciej), a duplikaty z sąsiednich części są usuwane po `gml:id` lub zasięgu
geometrii. Do samego tła kartograficznego w całej Polsce lepiej użyć nakładki **Granice PRG**
(WMS), bo jest natychmiastowa.

Limity: działki i budynki do **0,15°**, granice administracyjne do **20°**, maks. 6000 obiektów.

## Dlaczego proces, a nie WebAssembly

Usługi WFS GUGiK zwracają **wyłącznie GML** (sprawdzone: `OUTPUTFORMAT=application/json`
kończy się `400`), a sandbox wtyczek WASM pozwala pobrać dokument o rozmiarze do 2 MB. Działki
dla miasta to dziesiątki megabajtów, więc konwersja GML→GeoJSON i pobieranie muszą dziać się
poza sandboxem. To ta sama droga, którą idą wtyczki QGIS (*Pobieracz danych GUGiK*, *Wtyczka
GIS Support*).

Wtyczka procesowa **nie jest sandboxowana**: instalując ją, świadomie uruchamiasz program.
Nie wymaga Pythona ani GDAL-a: to jeden statyczny plik `.exe`.

## Budowanie

Wymagania: Rust (toolchain GNU lub MSVC) oraz, dla toolchainu GNU, mingw-w64 w `PATH`
(`gcc`, `dlltool`).

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\.local\mingw64\bin;$env:Path"
cargo test
cargo build --release
cargo run --release -p package -- --allow-missing   # pakuje to, co jest zbudowane
```

Pakowanie jest osobnym crate'em `packaging/`, w całości w Rust. Tworzy on:

| Plik | Co to |
| --- | --- |
| `dist\dane-pl\` | folder wtyczki (do ładowania w trakcie pracy) |
| `dist\dane-pl-0.1.0.rgplugin` | paczka do instalacji i publikacji |
| `dist\dane-pl.json` | wpis do rejestru w formacie, który przyjmuje `rustgis-plugins` |

Paczka jest **deterministyczna**: wpisy w archiwum są posortowane, znaczniki czasu stałe, prawa
plików ustawiane ręcznie (`0755` dla `bin/`), więc dwa przebiegi na tych samych binariach dają ten
sam SHA-256. Bez `--allow-missing` pakowanie wymaga binarki dla każdej platformy z `run` i kończy
się błędem, gdy jej brakuje.

Trzy platformy buduje `.github/workflows/release.yml` na tagu `v*`: Windows (`dane-pl.exe`), Linux
(`dane-pl-linux`) i macOS jako binarka uniwersalna (`dane-pl-macos`, `lipo` z aarch64 i x86_64).
Workflow pakuje je razem, sprawdza, że drugi przebieg daje ten sam SHA-256, i dołącza `.rgplugin`
oraz wpis rejestru do wydania.

## Języki

Wtyczka mówi dwoma językami: angielskim (baza) i polskim (tłumaczenie). Innych nie ma i nie
będzie: kod języka innego niż `pl` daje angielski.

- **Tekst manifestu**: angielski w polach, polski w `i18n.pl`. Obejmuje `name`, `description`,
  etykiety i podpowiedzi poleceń, nazwę i opis narzędzia, etykiety parametrów, zakładkę, grupy
  oraz wartości `choices`. Dzięki temu interfejs angielski pokazuje angielskie nazwy, a polski
  polskie (RustGIS tłumaczy nazwę i opis wtyczki przez `i18n`).
- **Tekst w czasie działania**: `src/text.rs`. Każde zdanie ma tam wersję angielską i polską,
  a wybiera ją `lang` z żądania. Dotyczy to meldunków, nazw warstw, postępu pobierania, błędów
  HTTP, błędów protokołu i komunikatów ULDK.
- **Liczebniki**: polskie formy są obsłużone (1 obiekt, 3 obiekty, 12 obiektów), angielskie też
  (1 feature, 3 features).
- **Nazwy warstw**: warstwa dodana po polsku nosi polską nazwę, po angielsku angielską. Warstwa
  dodana w jednym języku jest rozpoznawana także po przełączeniu na drugi, więc `przytnij` i
  `obszar-widok` działają dalej.
- **`choices`**: wartości są angielskie, a polskie etykiety siedzą w `i18n.pl`. Kod przyjmuje
  nazwę zestawu w obu językach i po identyfikatorze, więc zapisane wywołanie narzędzia działa
  niezależnie od języka interfejsu.

`rustgis-cli` zawsze wysyła `lang: "en"`, bo język ustawia tylko aplikacja z interfejsem. Wersję
polską sprawdza się na poziomie protokołu: podaj żądanie z `"lang": "pl"` na standardowe wejście
razem z odpowiedziami gospodarza.

## Zgodność z rejestrem

- `runtime: "process"`, bez `entry`; `run` wskazuje osobną binarkę na każdą platformę
  (`windows`, `macos`, `linux`) i każda z nich jest w paczce.
- Uprawnienia zminimalizowane do `edit` i `network`. `files` nie jest potrzebne: wtyczka nie
  używa poleceń gospodarza czytających pliki użytkownika, a własny cache trzyma poza RustGIS.
- Ikony to nazwy ikon RustGIS (`basemap`, `buffer`, `bookmark`, `label`, `relief`); nieznana nazwa
  dawałaby puzzel.
- Tekst manifestu jest po angielsku, a polski siedzi w `i18n.pl` (łącznie z `name` i
  `description`, które rejestr kopiuje do `registry.json`). Dlatego interfejs angielski pokazuje
  angielskie nazwy, a polski polskie.
- `choices` w narzędziu zostają po polsku, bo kod dopasowuje je po wartości; tłumaczenie ich
  w `i18n` zmieniłoby wartość podawaną wtyczce.
- Wpis rejestru ma kształt wymagany przez `rustgis-plugins`: `id`, `source.url`, `source.sha256`
  i `categories`.


## Próba bez instalacji

```powershell
rustgis-cli run --plugin .\dist\dane-pl --sample `
  --cmd 'plugin:dane-pl/dzialki={"bbox":[18.76,52.00,18.79,52.02]}'
```

W aplikacji: **Plugins › Plugin Manager › Settings › Add Plugin Folder…** i wskaż
`dist\dane-pl`.

## Źródła danych i licencje

Dane: Główny Urząd Geodezji i Kartografii, [geoportal.gov.pl](https://www.geoportal.gov.pl),
udostępniane nieodpłatnie. Przy każdej warstwie ustawiana jest atrybucja
„GUGiK · geoportal.gov.pl”. Wtyczka nie pośredniczy w danych i nie cache'uje ich poza RustGIS.

| Usługa | Adres |
| --- | --- |
| Ortofoto WMTS | `mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMTS/StandardResolution` |
| Ortofoto Hi-Res WMS | `…/PZGIK/ORTO/WMS/HighResolution` |
| BDOT10k WMS | `…/pub/guest/kompozycja_BDOT10k_WMS/MapServer/WMSServer` |
| KIEG (działki) | `integracja.gugik.gov.pl/cgi-bin/KrajowaIntegracjaEwidencjiGruntow` |
| Adresy | `…/wss/ext/KrajowaIntegracjaNumeracjiAdresowej` |
| PRG WMS / WFS | `…/PZGIK/PRG/WMS/AdministrativeBoundaries`, `…/PRG/WFS/AdministrativeBoundaries` |
| PRNG | `…/wss/service/PZGiK/PRNG/WMS/GeographicalNames` |
| NMT | `…/PZGIK/NMT/GRID1/WMS/ShadedRelief` |
| EGiB WFS | `…/PZGIK/EGIB/WFS/UslugaZbiorcza` |
| ULDK | `uldk.gugik.gov.pl` |

## Ograniczenia

- Narzędzie tnie po **zasięgu (bbox)** warstwy obszaru; nie przycina geometrii do jej poligonów.
- Limit obiektów: 6000 na pobranie (po 4 strony równolegle, po 500 obiektów). Większy obszar =
  komunikat o przerwaniu.
- Brak pobierania paczek LAZ / NMT / ortofoto na dysk (to materiał na wersję 2).
- WFS GUGiK nie ma GeoJSON-u, więc konwersja jest lokalna; geometrie inne niż
  Point/LineString/Polygon i ich wersje multi nie są obsługiwane.
- ULDK przyjmuje tylko EPSG:2180, więc wtyczka sama rzutuje współrzędne (Transverse Mercator,
  zgodność z PROJ na poziomie < 1 mm dla punktu testowego).
