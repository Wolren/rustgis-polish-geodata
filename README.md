# Dane PL — polskie dane geodezyjne w RustGIS

Wtyczka do [RustGIS](https://rustgis.app): jedna zakładka **Dane PL** z podkładami i danymi
GUGiK — bez kluczy API, bez pośredników.

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
| Dane | **Obszar z widoku** | Dodaje prostokąt bieżącego widoku jako warstwę „Obszar pobierania” |
| | **Pobierz dane…** | Narzędzie (panel Narzędzia): zestaw danych + warstwa obszaru, w tle z postępem i przerwaniem |
| Działka | Działka z ULDK | Działka pod środkiem mapy: geometria + atrybuty |
| Narzędzia | **Przytnij do obszaru** | Tnie warstwę do granicy „Obszar pobierania”; bez argumentów bierze najnowszą warstwę |
| | Wyczyść cache | Usuwa zapisane odpowiedzi WFS (domyślnie 7 dni) |

### Przycinanie do granicy

Filtr WFS zwraca **całe** obiekty, które dotykają obszaru — nie przycina ich. Dlatego po pobraniu
uruchom **Przytnij do obszaru**: wywołuje wbudowane narzędzie Clip i zastępuje warstwę wersją
przyciętą (jedna warstwa, nie dwie). Uchwyt `layer` jest opcjonalny: bez niego bierze najnowszą
warstwę wektorową, więc po poleceniu „Pobierz dane…” wystarczy jedno kliknięcie.

Pomiar: 5586 działek przecinało obszar, po przycięciu zostało 5583 — dla działek różnica jest
mała, dla dróg i obiektów wielkości gminy duża.

### Sterowanie z agenta (MCP)

Wtyczka jest dostępna przez MCP bez osobnej warstwy, bo RustGIS uruchamia ją jako polecenia
i narzędzia geoprzetwarzania. Agent używa `run_command` i `run_tool`:

```
run_command  plugin:dane-pl/obszar-widok  {}
run_tool     plugin:dane-pl/pobierz       {"dane":"Działki (EGiB)","obszar":"Obszar pobierania"}
run_command  plugin:dane-pl/przytnij      {}
```

`describe_tool` i `search_tools` widzą wtyczkę po instalacji. `script.run` (łączenie kilku
poleceń w jednym wywołaniu) jest zarezerwowane dla użytkownika i agent go nie użyje, dlatego
`przytnij` działa bez argumentów.

Kliknięcie podkładu drugi raz usuwa warstwę. Jeśli widok nie obejmuje Polski, wtyczka najpierw
przybliża mapę do Polski — usługi GUGiK pokrywają tylko Polskę i poza nią zwracają puste kafle.

Zakresy skalowe usług (sprawdzone na kaflach 256 i 512 px):

| Warstwa | Widoczna od | Kafel |
| --- | --- | --- |
| BDOT10k | z13 | 512 px (kompozycja rysuje dopiero od tej skali) |
| Działki (KIEG), Adresy | z15 | 256 px |
| Ortofoto, PRG, PRNG, NMT | z0 | 256 px |

### Pobieranie: dwa kliknięcia, bez zamrażania okna

1. **Obszar z widoku** — na mapie pojawia się prostokąt „Obszar pobierania”.
2. **Pobierz dane…** — wybierz zestaw danych i warstwę obszaru, uruchom.

Narzędzie działa **w tle** (panel Narzędzia pokazuje postęp i ma przycisk przerwania), bo
polecenia wtyczek wykonują się w wątku interfejsu — pobieranie jako polecenie zamrażało okno.

Zamiast prostokąta można wskazać własną warstwę (np. wcześniej pobrane granice gminy); jeśli
ma zaznaczenie, liczy się tylko zaznaczenie.

### Ile to trwa i ile waży

GUGiK wysyła geometrię w pełnej rozdzielczości i ignoruje parametry uogólniania
(`MAP_RESOLUTION`, `resolution`) — sprawdzone. Dla całego kraju:

| Zestaw | Obiekty | Rozmiar odpowiedzi | Czas serwera |
| --- | --- | --- | --- |
| Województwa | 16 | 8 MB | ~10 s |
| Powiaty | 380 | 42 MB | ~30 s |

Dlatego obszar większy niż 1,5° jest dzielony na 4 części pobierane **równolegle**
(ok. 2–3× szybciej), a duplikaty z sąsiednich części są usuwane po `gml:id` lub zasięgu
geometrii. Do samego tła kartograficznego w całej Polsce lepiej użyć nakładki **Granice PRG**
(WMS) — jest natychmiastowa.

Limity: działki i budynki do **0,15°**, granice administracyjne do **20°**, maks. 6000 obiektów.

## Dlaczego proces, a nie WebAssembly

Usługi WFS GUGiK zwracają **wyłącznie GML** (sprawdzone: `OUTPUTFORMAT=application/json`
kończy się `400`), a sandbox wtyczek WASM pozwala pobrać dokument o rozmiarze do 2 MB. Działki
dla miasta to dziesiątki megabajtów, więc konwersja GML→GeoJSON i pobieranie muszą dziać się
poza sandboxem. To ta sama droga, którą idą wtyczki QGIS (*Pobieracz danych GUGiK*, *Wtyczka
GIS Support*).

Wtyczka procesowa **nie jest sandboxowana** — instalując ją, świadomie uruchamiasz program.
Nie wymaga Pythona ani GDAL-a: to jeden statyczny plik `.exe`.

## Budowanie

Wymagania: Rust (toolchain GNU lub MSVC) oraz, dla toolchainu GNU, mingw-w64 w `PATH`
(`gcc`, `dlltool`).

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\.local\mingw64\bin;$env:Path"
cargo test
cargo build --release
.\scripts\package.ps1
```

`package.ps1` tworzy:

| Plik | Co to |
| --- | --- |
| `dist\dane-pl\` | folder wtyczki (do ładowania w trakcie pracy) |
| `dist\dane-pl-0.1.0.rgplugin` | paczka do instalacji |
| `dist\dane-pl.json` | wpis do rejestru (SHA-256, rozmiar) |

## Próba bez instalacji

```powershell
rustgis-cli run --plugin .\dist\dane-pl --sample `
  --cmd 'plugin:dane-pl/dzialki={"bbox":[18.76,52.00,18.79,52.02]}'
```

W aplikacji: **Plugins › Plugin Manager › Settings › Add Plugin Folder…** i wskaż
`dist\dane-pl`.

## Źródła danych i licencje

Dane: Główny Urząd Geodezji i Kartografii — [geoportal.gov.pl](https://www.geoportal.gov.pl),
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
- ULDK przyjmuje tylko EPSG:2180 — wtyczka sama rzutuje współrzędne (Transverse Mercator,
  zgodność z PROJ na poziomie < 1 mm dla punktu testowego).
