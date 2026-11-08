# Dane PL: polskie dane geodezyjne w RustGIS

Jedna zakładka z polskimi danymi publicznymi: podkładami, działkami, adresami, granicami,
obszarami chronionymi, geologią, modelami wysokości i przeszukiwalnym rejestrem 3 296 adresów
WMS i WFS. Bez kluczy API i bez pośredników.

## Instalacja

- **Z paczki:** zainstaluj `dane-pl-0.6.0.rgplugin` przez **Plugins › Plugin Manager › Install from file**.
- **Z folderu:** wskaż `dist\dane-pl` przez **Plugins › Plugin Manager › Settings › Add Plugin
  Folder…**.
- **Bez instalacji:**

```powershell
rustgis-cli run --plugin .\dist\dane-pl --sample --cmd 'plugin:dane-pl/obszar-widok={}'
```

Wtyczka to jeden statyczny plik `.exe`, który RustGIS uruchamia przy każdym wywołaniu. Nie wymaga
Pythona ani GDAL-a. Wymaga RustGIS 0.5.0 lub nowszego.

## Zakładka

| Grupa | Co zawiera |
| --- | --- |
| **Podkłady** | 4 mapy rastrowe i 4 nakładki wektorowe: ortofoto, BDOT10k, działki, adresy, granice |
| **Obszar** | obszar z widoku, rysowanie, prostokąt, przejęcie, anulowanie, usuwanie |
| **Narzędzia** | panel i przycinanie do granicy |
| **WMS (podgląd)** | GUGiK (76), mapy historyczne (11), geologia PIG (11), NID (4), BDL (4), pozostałe instytucje, **Więcej map** (Wody Polskie 30, Cartomatic 45), **Rejestr** (6) |
| **Pobieranie** | sześć narzędzi WFS, cztery narzędzia paczek, ochrona przyrody (GDOŚ) |
| **Wysokości** | trzy modele wysokości i przełącznik stylu |
| **Wyszukiwanie** | działka, adres, wysokość, znajdź usługę, aktualizacja rejestru |

Zakładkę trzyma jedna zasada: **polecenie pokazuje coś od razu, narzędzie pobiera w tle**, z
postępem i przyciskiem Anuluj. Wyjątkiem są trzy polecenia wysokości, bo narzędzie nie umie dodać
warstwy rastrowej.

## Obszar

Narzędzie pobierania wymaga warstwy obszaru. Pięć przycisków tworzy jedną:

| Przycisk | Co robi |
| --- | --- |
| **Obszar z widoku** | zapisuje bieżący widok jako obszar, dokładnie to, co widać |
| **Narysuj obszar** | rysuje poligon, a pobieranie idzie za kształtem |
| **Narysuj prostokąt** | uzbraja narzędzie prostokąta z aplikacji, które rysuje grafikę |
| **Prostokąt do obszaru** | przenosi tę grafikę do warstwy obszaru |
| **Usuń obszar** | zaczyna od pustej warstwy |

Wtyczka nie zgaduje obszaru z aktywnej warstwy. Narysowany poligon staje się prostokątem dla modelu
wysokości, a komunikat podaje, którym. Przycinanie w panelu idzie po warstwie obszaru albo po
dowolnej warstwie poligonowej wybranej w drzewie.

Skrypt ustawia obszar bez rysowania: `obszar-json` bierze `{"bbox": [zachód, południe, wschód,
północ]}` albo `{"geojson": <Feature>}` i używa jego zasięgu.

## Pobieranie

Sześć narzędzi bierze dane WFS po zbiorze: 106 zbiorów mapowych i 162 skorowidze. GDOŚ dokłada 15
typów obszarów chronionych. Cztery narzędzia paczek kładą skorowidz jako warstwę i pobierają pliki,
które wskazują jego obiekty.

| Limit | Wartość |
| --- | --- |
| Obiektów na pobranie | 6 000 |
| Zasięg dla działek i budynków | 0,15° |
| Zasięg dla granic i jednostek | 20° |
| Próg rozmiaru przed startem | 25 MB, w panelu suwak 5 do 500 MB |
| Twardy sufit | 256 MB |

Działki dla miasta ważą około 9 MB, zmierzone na Warszawie: 5 248 działek. Próg pyta raz, a
zaznaczenie **Pobierz mimo rozmiaru** odpowiada za wszystkie następne przebiegi.

Filtr WFS zwraca całe obiekty, które dotykają obszaru. **Przytnij do warstwy** tnie je wbudowanym
Clipem i zastępuje warstwę wynikiem.

Duża siatka wysokości czeka na drugie naciśnięcie. Pierwsze liczy komórki, MB i sekundy i nic nie
pobiera. Drugie, w ciągu 120 s, pobiera. Mała siatka idzie od razu, a pobrana wcześniej liczy się z
pliku w cache.

## Panel

Panel grupuje wszystko, czego potrzebuje jeden przebieg: statusy (widok, obszar, para „warstwa →
granica”), drzewo warstw granicznych z wyszukiwarką, drzewo **Mapa** z trzema korzeniami
(podkłady, pobieranie, rejestr), suwak i zaznaczenie limitu pobrania, katalog i zapisane odpowiedzi.
**Odśwież panel** czyta widok, obszar i katalog od nowa.

## Rejestr usług

`data/eziudp.json` niesie 3 296 adresów. Każdy odpowiedział na prawdziwe żądanie przy budowie:
1 742 WMS od 570 instytucji i 1 554 WFS. Trzy drogi do nich:

- **Panel, drzewo Mapa, korzeń Rejestr EZiUDP:** instytucja, a pod nią jej WMS. Klik kładzie usługę
  jako nakładkę.
- **Wstęga, menu Rejestr:** sześć usług, których katalog nie ma, wszystkie dowiedzione: GIOŚ
  (hałas, powietrze, wody podziemne) oraz własne EGiB Poznania, Wrocławia i Warszawy.
- **Polecenie `zrodla`:** szuka po nazwie zbioru, instytucji i TERYT, także wśród WFS-ów, a
  `{"dodaj": "<id>"}` dodaje jedną usługę.

Narzędzie **Aktualizuj rejestr** czyta rejestr EZiUDP na nowo. Sprawdza tylko adresy, których plik
nie ma, i zapisuje wynik poza paczką. Polecenie `eziudp` pyta rejestr na żywo i prosi o zawężenie,
bo bez filtra rejestr ma 9 MB.

Wody Polskie: `wody-warstwy` wypisuje warstwy usługi z tytułami, a parametr `warstwa` kładzie jedną
warstwę zamiast wszystkich. Tytuły pochodzą z `GetCapabilities` i leżą w `data/wody.json`.

## Wyszukiwania

| Polecenie | Usługa | Co odpowiada |
| --- | --- | --- |
| `geokoduj` | UUG | punkt, ulica, numer i kod dla adresu |
| `adres-odwrotnie` | UUG | adres pod środkiem mapy albo pod `lon`, `lat` |
| `wysokosc` | NMT | wysokość w PL-KRON86-NH w punkcie |
| `dzialka-id` | ULDK | granica działki dla identyfikatora |
| `parcel` | ULDK | działka pod środkiem mapy |

## Dla agenta

```text
run_command  plugin:dane-pl/katalog   {}
run_command  plugin:dane-pl/zrodla    {"instytucja": "GIOŚ", "rodzaj": "wms"}
run_command  plugin:dane-pl/mapa      {"panel": {"mapa": "v:bdot"}}
run_tool     plugin:dane-pl/pobierz-granice  {"dane": "A03_Granice_gmin", "obszar": "Obszar pobierania"}
```

Każde polecenie deklaruje swoje argumenty w `params`, więc `rustgis-cli commands` i asystent widzą je bez czytania kodu. `katalog` wypisuje każdą usługę, zbiór i identyfikator, więc agent nie czyta manifestu. Paczka
niesie `skills/dane-pl/SKILL.md`. Dzięki temu asystent w RustGIS zna polecenia obszaru, limity i
narzędzia pobierania, dopóki wtyczka jest włączona.

## Ograniczenia

- Pobranie kończy się na 6 000 obiektach. Większy obszar dostaje licznik i przerwanie.
- Przycinanie tnie po zasięgu granicy, nie po jej poligonach.
- Polecenia Wody Polskie pokazują wszystkie warstwy usługi. Nie ma wyboru jednej warstwy.
- WFS GUGiK odpowiada tylko GML-em. Wtyczka zamienia go lokalnie na GeoJSON.
- Brak plików LAZ i geobaz z dane.gov.pl. Narzędzia paczek biorą to, co wskazuje pole
  `url_do_pobrania` w skorowidzu.

## Repozytorium

| Ścieżka | Co to |
| --- | --- |
| `src/` | wtyczka, jeden moduł na temat |
| `data/` | katalogi: usługi, zbiory WFS, Wody, Cartomatic, GDOŚ, PIG, rejestr, wstęga |
| `tools/` | budowniczowie, którzy dowodzą każdy adres zapisany do `data/` |
| `packaging/` | pakowacz w Rust, który robi `.rgplugin` |
| `plugin.json` | generowany z `data/` i `tools/manifest.base.json` |

`plugin.json` niesie 223 polecenia, 11 narzędzi i 178 polskich tłumaczeń. Jest generowany, więc
zmieniaj `tools/manifest.base.json`.

```powershell
python tools/manifest.py            # zapisuje plugin.json
python tools/manifest.py --check    # kończy się kodem 1, gdy plugin.json jest nieaktualny
```

## Budowanie

```powershell
cargo test
cargo build --release
cargo run --release -p package -- --allow-missing
```

Pakowacz pisze `dist\dane-pl\` (folder dla menedżera wtyczek), `dist\dane-pl-0.6.0.rgplugin` i
`dist\dane-pl.json` (wpis do rejestru). Archiwum jest deterministyczne: dwa przebiegi na tych
samych binariach dają ten sam SHA-256.

## Języki

Angielski jest bazą, polski tłumaczeniem, a każdy inny kod języka odpowiada po angielsku.
Tekst interfejsu leży w `i18n.pl`, komunikaty czasu działania w `src/text.rs`, oba kompletne.
Build przerywa się, gdy tekst interfejsu nie ma polskiej wersji albo polska wersja nie pasuje do
żadnego tekstu. Nazwy danych zostają takie, jak je opublikowano, więc „Działki” i „GIOŚ: hałas”
czytają się w obu językach tak samo.

## Źródła

Dane pochodzą z GUGiK ([geoportal.gov.pl](https://www.geoportal.gov.pl)), GDOŚ, PIG, Wód Polskich i
Cartomatic. Każda warstwa niesie instytucję jako atrybucję. Wtyczka czyta publiczne usługi i nie
przepuszcza danych przez własny serwer.
