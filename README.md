# Dane PL: polskie dane geodezyjne w RustGIS

Wtyczka do [RustGIS](https://rustgis.app): jedna zakładka **Dane PL** z podkładami, usługami i
danymi GUGiK, bez kluczy API i bez pośredników.

Zasada, która trzyma tę wtyczkę w kupie: **polecenie tylko pokazuje coś na mapie, a pobiera
narzędzie.** Wstęga mówi to wprost nazwami grup: **WMS (podgląd)** tylko wyświetla, **Pobieranie**
jako jedyne ściąga dane, **Wyszukiwanie** pyta usługę punktową, **Narzędzia** zmieniają projekt. Polecenie wykonuje się w wątku interfejsu, więc pobieranie jako polecenie zamrażało
okno na czas transferu. Dlatego na wstędze nie ma już 108 przycisków pobierania: są pięć narzędzi
w panelu Narzędzia, działających w tle, z postępem i przerwaniem.

| Grupa | Przycisk | Co robi |
| --- | --- | --- |
| Podkłady | Rastry | Ortofoto (WMTS), Ortofoto Hi-Res (WMS), BDOT10k, cieniowanie NMT |
| | Wektory | Działki (KIEG), Adresy, Granice PRG, Nazwy miejscowości |
| **Narzędzia** | Obszar | Obszar z widoku (nie rysowanie), rysowanie, anulowanie, czyszczenie |
| | Pozostałe | Panel, Przytnij do granicy, Przybliż do Polski, Katalog (JSON) |
| **GUGiK** | GUGiK · Ortofoto i NMT | 19 usług — tylko podgląd na mapie |
| | GUGiK · Mapy | 21 usług |
| | GUGiK · Kataster i granice | 24 usługi |
| | GUGiK · Pozostałe dane | 11 usług |
| **Instytucje** | Mapy historyczne | 11 arkuszy WIG i map niemieckich |
| | Geologia (PIG) | 9 usług |
| | Zabytki (NID) | 4 usługi |
| | Lasy (BDL) | 4 usługi |
| | Pozostałe instytucje | GDOŚ, PKP PLK, GUS, IMGW-PIB |
| **Wody** | Warstwy usługi | Wypisuje usługi Wód Polskich i warstwy jednej usługi |
| | MZP (zagrożenie) | 11 usług: głębokość i prędkość przy 10%, 1% i 0,2%, zniszczenie wału i budowli |
| | MRP (ryzyko) | 12 usług: ludzie i straty, środowisko, kultura, gospodarka |
| | WORP | 4 usługi: obszary narażone i mapy sprzed 2020 |
| | Plany i susza | 5 usług: aPGW, IIaPGW, PZRP, PPSS, struktura PGW |
| **Pobieranie** | Pięć narzędzi | Jedyne miejsce, które ściąga dane: adresy i budynki, granice, osnowy, urzędy, pozostałe |
| **Wysokości** | Model terenu KRON86 | Model terenu 1 m (GeoTIFF) w barwach wysokości, od razu teren sceny 3D |
| | Model terenu EVRF2007 | To samo w PL-EVRF2007-NH (siatka ASCII) |
| | Model pokrycia NMPT | Model pokrycia 1 m, z budynkami i drzewami (siatka ASCII) |
| | Stylizuj model | Barwy, pasma albo cieniowanie najnowszego modelu |
| | Teren wyłącz | Wypłaszcza scenę 3D, gdy model zniknął z projektu |
| **Wyszukiwanie** | Działka z ULDK | Działka pod środkiem mapy |
| | Działki z widoku | Działki i budynki z WFS powiatu, tylko dla widocznego obszaru |
| | Rejestr dla widoku | Co powiat pod środkiem mapy zgłasza do EZiUDP |
| | Wysokość w punkcie | Wysokość NMT w PL-1992 (środek mapy albo x/y) |
| | Panel | Widok, obszar, katalog i zapisane odpowiedzi; przyciski do poleceń |

Łącznie: **169 poleceń** (107 usług, 32 usługi Wód Polskich i 30 własnych) i **5 narzędzi**.
Wcześniej było 228 poleceń, z czego 108 pobierało dane natychmiast, blokując okno.

Wstęga ma osiem grup. Katalog usług siedzi w dwóch (**GUGiK** i **Instytucje**), a Wody Polskie
mają własną grupę z czterema menu — wcześniej dziewięć menu w jednej grupie wypychało Pobieranie,
Wysokości i Wyszukiwanie za strzałkę przewijania.


### Usługi krajowe

Katalog obejmuje **107 usług WMS** z Geoportalu Krajowego i od innych instytucji: BDOT10k (w tym nowe
budynki i statystyki), ortofotomapę prawdziwą, wysokiej rozdzielczości i archiwalną, cieniowanie
NMT, EMUiA, PRG, PRNG, RCN, KIMPZP, ISOK, POLSA, osnowy, mapy geologiczne PIG, BDL Lasów
Państwowych, zabytki NID i inne.

Każda pozycja przechodzi realne żądanie kafla w EPSG:3857 i można to powtórzyć w każdej chwili:

```
python tools/probe.py            # audyt katalogu, kod wyjścia 1 gdy coś nie rysuje
python tools/probe.py --prune    # to samo, plus usunięcie martwych pozycji
python tools/probe.py --overlays # tylko wbudowane nakładki z src/catalog.rs
python tools/smoke.py            # 165 poleceń w prywatnej sesji, kod 1 gdy któreś padnie
```

Sonda prosi o kafel w pięciu miejscach (Poznań, Gdańsk, Zakopane, Wrocław, Warszawa) w trzech
skalach — cały kraj, 20 km i 1,2 km — bo część usług rysuje tylko z bliska, a część tylko w jednym
regionie. Audyt z 30 października 2026 wykrył dwie martwe pozycje i obie zostały usunięte:
`PZGIK/ORTO/WMS/StandardResolution` (cała usługa zwraca 404, ortofotomapę standardową daje WMTS
w Podkładach) oraz `PZGIK/NMT/GRID1/WMS/Hypsometry` (oddaje wyjątek w 3857 — usługa istnieje tylko
w EPSG:2180, którego RustGIS nie rysuje). Wcześniej katalog rósł i nigdy się nie kurczył, więc taka
pozycja zostawała w nim na zawsze.

`smoke.py` sprawdza drugą stronę: nie adresy, a polecenia. Uruchamia każde polecenie manifestu
w prywatnej sesji `rustgis-cli mcp` (z fiksturą obszaru i działek), zdejmuje kafle między
poleceniami — mapa mieści 32 warstwy kafelkowe, a katalog ma 139 — i traktuje jako błąd każdą
odpowiedź, która nie jest JSON-em. Model wysokości pobiera tylko z `--slow`. Workflow
`.github/workflows/catalogue.yml` uruchamia `probe.py` co poniedziałek, bo wydawcy wycofują usługi
bez ostrzeżenia.

Usługi stoją w dwóch grupach: **GUGiK** (cztery menu tematyczne) i **Instytucje** (pięć menu).
Podział jest instytucjonalny, bo tak szukają danych użytkownicy: najpierw *kto publikuje*, potem
*co*. Naciśnięcie nazwy wjeżdża na mapę jako warstwa
kafelkowa, drugie naciśnięcie ją zdejmuje. Każda usługa to osobne polecenie, więc działa też
wyszukiwarka poleceń: wpisz „cieniowanie” albo „ortofoto” i naciśnij wynik.

```
run_command  plugin:dane-pl/usluga-cieniowanie-wms
run_command  plugin:dane-pl/usluga-ceny-nieruchomosci
```

Osobne menu **Mapy historyczne** to jedenaście arkuszy z serwera Cartomatic: WIG 1:25 000,
1:100 000, 1:300 000 i 1:500 000 oraz mapy niemieckie (Karte des Deutschen Reiches 1:100 000,
Meßtischblätter 1:25 000, Übersichtskarte von Mitteleuropa 1:300 000). Każdy arkusz przeszedł
realne żądanie kafla, tak jak reszta katalogu. Serwer podaje 110 warstw (plany miast, mapy
katastralne), z których wybrano te jedenaście — reszta czeka na osobną decyzję.

### Wody Polskie: 32 usługi, 904 warstwy, jedno polecenie na usługę

Mapy powodziowe, suszowe i planistyczne PGW Wody Polskie to 32 usługi WMS na
`wody.isok.gov.pl/gpservices` i 904 warstwy. Jedno polecenie na warstwę dałoby 904 pozycje na
wstędze, więc **polecenie to usługa, a warstwa to parametr**:

```
run_command  plugin:dane-pl/wody-mzp-glebokosc-1
run_command  plugin:dane-pl/wody-mzp-glebokosc-1  {"warstwa": "wał przeciwpowodziowy"}
```

Bez `warstwa` polecenie kładzie warstwę domyślną usługi (dla map zagrożenia — obszar szczególnego
zagrożenia, nie wariant „od morza”). Warstwy szuka się po nazwie albo numerze, po polsku i bez
ogonków; przecinek łączy kilka w jedną nakładkę. **Warstwy usługi** wypisuje katalog:

```
run_command  plugin:dane-pl/warstwy
run_command  plugin:dane-pl/warstwy  {"usluga": "wody-mzp-glebokosc-1"}
run_command  plugin:dane-pl/warstwy  {"usluga": "wody-mzp-glebokosc-1", "szukaj": "wał"}
```

Nazwy warstw nie pochodzą z capabilities: te usługi to ArcGIS MapServery, które każdą warstwę
nazywają numerem i tytułują tym samym numerem. Czytelna tabela 904 warstw jest w danych
(`data/wody.json`), zbudowana z wtyczki QGIS „Wody Polskie — Baza WMS” przez `tools/wody.py`.

Usługi przyjmują EPSG:3857 (sprawdzone żądaniem kafla), więc działają w RustGIS bez przeliczeń.
Każda warstwa ma atrybucję „PGW Wody Polskie · wody.isok.gov.pl”.

### Dane z WFS

Katalog obejmuje **268 typów WFS** z krajowych usług pobierania: adresy i ulice z KI NA, budynki
i działki z EGiB, granice z PRG, osnowy, urzędy, sądy, kolej, obiekty morskie i statystyki BDOT.
Każdy typ przeszedł realne żądanie GetFeature, więc lista nie zawiera martwych adresów.

**106 z nich to dane mapowe** i te pokazują się w narzędziach pobierania. Pozostałe **162 to
skorowidze** (arkusze i siatki, które wskazują paczki do pobrania z Geoportalu) — wtyczka ich nie
pobiera, bo nie umie ściągać paczek, ale zostają w katalogu: skrypt może je wskazać po
identyfikatorze, np. `dane-skorowidz-danych-pomiarowych-lidar-2024`.

Pobieranie to **pięć narzędzi** w grupie **Dane › Pobieranie**. Każde ma listę swoich zestawów:

| Narzędzie | Zestawy |
| --- | --- |
| Pobierz adresy i budynki | 19 |
| Pobierz granice i jednostki | 24 |
| Pobierz osnowy i pomiary | 21 |
| Pobierz urzędy i sądy | 21 |
| Pobierz pozostałe dane | 21 |

1. Wskaż obszar: **Obszar z widoku**, **Narysuj obszar** albo własna warstwa.
2. Otwórz **Dane › Pobieranie** i wybierz grupę.
3. W panelu Narzędzia wybierz zestaw i warstwę obszaru, naciśnij **Uruchom**.

Narzędzie działa w tle: panel pokazuje postęp i ma przycisk przerwania. Wynik wjeżdża na mapę jako
warstwa wektorowa, jednym krokiem cofania. Opcje w formularzu to tylko zestaw i warstwa obszaru —
nic więcej nie trzeba ustawiać, bo nic więcej nie jest zgadywane.

Skrypt albo agent uruchamia to samo przez `gp.run`:

```
run_tool  plugin:dane-pl/pobierz-adresy  {"dane": "Budynki", "obszar": "Obszar pobierania"}
run_tool  plugin:dane-pl/pobierz-granice  {"dane": "A03_Granice_gmin", "obszar": "obszar"}
```

Typy spoza układu 4326 działają bez ustawień: wtyczka wysyła pierwsze żądanie we współrzędnych
geograficznych, a gdy usługa odpowie pustą kolekcją, powtarza je w układzie 2180 i przelicza
współrzędne z powrotem. Dotyczy to usługi adresowej KI NA.

### Działki z widoku: WFS powiatu, nie kafel

Nakładka **Działki (KIEG)** to raster: widać granice, ale nie da się z niej wybrać działki ani
przefiltrować jej po własnej granicy. Każdy powiat prowadzi natomiast własny WFS EGiB i ten oddaje
GeoJSON dla prostokąta. Polecenie **Działki z widoku** idzie tą drogą:

1. ULDK `GetCountyByXY` mówi, w jakim powiecie leży środek mapy (`1465|Warszawa`).
2. EZiUDP (`api.php?teryt=…&usluga=wfs`) podaje adres WFS EGiB tego powiatu — bez tabeli 380
   powiatów w kodzie, więc nowa usługa w rejestrze działa od razu.
3. Capabilities WFS dają wersję i nazwy typów; wtyczka bierze typ z `dzialk` i z `budynk` w nazwie.
4. Jedno `GetFeature` na typ, z prostokątem widoku i limitem 5000 obiektów, wjeżdża na mapę jako
   warstwa „Działki – <obszar>” i „Budynki – <obszar>”.

Polecenie działa od skali 1:25 000 i bliżej; wyżej widok obejmowałby więcej działek, niż ma sens
w jednym żądaniu. Nie pobiera całego powiatu (to do 50 000 obiektów), tylko to, co widać.

```
run_command  plugin:dane-pl/dzialki-z-widoku
```

Powiat, którego WFS nie oddaje GeoJSON, nie jest odsyłany z kwitkiem: wtyczka prosi wtedy o GML
i czyta go parserem napisanym dla krajowego katalogu WFS. GeoJSON oddaje 163 z 380 powiatów
(stan z października 2026), więc bez tego kroku dwie trzecie Polski nie miałoby działek z atrybutami.

Kolejność prób jest zmierzona, nie zgadywana:

1. GeoJSON (`OUTPUTFORMAT=application/json`, potem `json`),
2. GML w stopniach (`SRSNAME=EPSG:4326`, bez `OUTPUTFORMAT` — serwery powiatowe odrzucają format,
   którego nie ogłaszają),
3. GML w układzie 1992: WFS 2.0 czyta ten układ jako northing, easting i tylko URN to wymusza,
   WFS 1.x bierze easting, northing. Zmierzone na MapServerze powiatu poznańskiego: 2.0 z URN
   zwraca 5 obiektów, 2.0 z gołym kodem — zero, 1.1 z URN — zero.

Przykład: powiat poznański (tylko GML, 2180) — 1447 działek i 2054 budynki jednym żądaniem;
Warszawa (GeoJSON) — 247 i 142.


### Obszar: nic nie jest zgadywane

Narzędzie pobierania **wymaga warstwy obszaru**. Nie ma już łańcucha „aktywna warstwa, jej
zaznaczenie, największe zaznaczenie w projekcie, warstwa obszaru, a w ostateczności cała aktywna
warstwa”. To ostatnie ogniwo potrafiło pobrać województwo po kliknięciu w przycisk, a przedostatnie
brało obszar z warstwy, której użytkownik nie widział.

Model wysokości (polecenie, więc działa natychmiast) bierze obszar w tej kolejności:

1. warstwa podana w argumencie `area`,
2. zaznaczenie na aktywnej warstwie,
3. warstwa „Obszar pobierania” z przycisku **Obszar z widoku**,
4. aktywna warstwa w całości.

Zaznaczenie na innej warstwie nie jest brane pod uwagę. Zbyt duży obszar jest odrzucany z liczbami,
zanim poleci jakiekolwiek żądanie.

**Obszar z widoku** nie jest narzędziem rysowania i nie udaje go: bierze bieżący widok mapy i
zapisuje go jako prostokąt na warstwie „Obszar pobierania”. Prostokąt jest dokładnie tym, co widać.
Chcesz inny obszar? Przybliż mapę i naciśnij ponownie.

**Narysuj obszar** zostawia kształt taki, jak narysowany: klikaj narożniki i zakończ dwuklikiem
(kliknięcie w pierwszy narożnik nie zamyka obwodu — tak działa narzędzie RustGIS), albo przerwij
Escape. Pobieranie idzie za narysowanym kształtem. Model wysokości jest natomiast zawsze
prostokątem, więc obejmuje prostokąt wokół kształtu i wtyczka mówi wtedy, jaki to prostokąt, a
warstwa obszaru pokazuje go zamiast rysunku.

To rozróżnienie jest jedyną prawdą, jaką wtyczka może powiedzieć: RustGIS rysuje obiekty
edytowalne punkt po punkcie i nie ma narzędzia prostokąta (prostokąt „przeciągnij” istnieje tylko
dla grafiki, nie dla warstwy wektorowej). Dlatego prostokąt bierze się z widoku, a nie z rysowania,
i przycisk nazywa się **Obszar z widoku**, nie „prostokąt”.

**Wyczyść obszar** usuwa warstwę, więc kolejny prostokąt albo rysunek zaczyna od zera. Bez tego
drugi rysunek dokłada kształt do istniejącego i obszar się powiększa.

### Przycinanie do granicy

Filtr WFS zwraca **całe** obiekty, które dotykają obszaru i nie przycina ich. Dlatego po pobraniu
uruchom **Przytnij do obszaru**: wywołuje wbudowane narzędzie Clip i zastępuje warstwę wersją
przyciętą (jedna warstwa, nie dwie).

Granica jest zawsze **warstwą**, nigdy widokiem:

```
run_command  plugin:dane-pl/przytnij  {"obszar": "gminy", "layer": "budynki"}
run_command  plugin:dane-pl/przytnij  {"granica": "gminy", "warstwa": "budynki"}
```

- `obszar` (albo `granica`) to warstwa granicy; bez niej wtyczka bierze warstwę „Obszar pobierania”.
- `layer` (albo `warstwa`) to warstwa cięta; bez niej najnowsza warstwa wektorowa.
- **Zaznaczenie na warstwie granicy liczy się samo**: zaznacz jedną gminę w warstwie 2478 gmin, a
  cięcie pójdzie tą gminą. Wtyczka kopiuje zaznaczone obiekty do warstwy tymczasowej, tnie nią i
  warstwę usuwa.
- Gdy granicą jest prostokąt z widoku, komunikat mówi to wprost: „…(prostokąt z widoku)”. Nikt go
  nie narysował, więc nie udaje rysunku.

Pomiar: 5586 działek przecinało obszar, po przycięciu zostało 5583, dla działek różnica jest
mała, dla dróg i obiektów wielkości gminy duża.

### Model wysokości z obszaru

Grupa **Wysokości** ma trzy pokrycia:

| Przycisk | Pokrycie | Format |
| --- | --- | --- |
| Model terenu KRON86 | `DTM_PL-KRON86-NH_TIFF` | GeoTIFF |
| Model terenu EVRF2007 | `DTM_PL-EVRF2007-NH` | siatka ASCII |
| Model pokrycia NMPT | `DSM_PL-KRON86-NH` | siatka ASCII |

GeoTIFF pobiera sam RustGIS i bierze z niego układ odniesienia. Siatka ASCII nie niesie układu, więc
wtyczka pobiera ją sama i dopisuje plik `.prj` z EPSG:2180.

Model wjeżdża na mapę **w barwach wysokości** (ciągła rampa hipsometryczna) i od razu jest
ustawiany jako **teren sceny 3D** (`map.terrain`). Bez tego drugiego kroku przechylona mapa zostaje
płaska: raster jest wtedy tylko obrazem położonym na płaszczyźnie i nie ma z czego wziąć wysokości.
Cieniowanie jest jedną z opcji, nie domyślnym wyglądem, bo szary relief zasłania te liczby, które
model ze sobą niesie.

Parametry polecenia (wszystkie opcjonalne):

| Parametr | Znaczenie |
| --- | --- |
| `area` | nazwa warstwy obszaru; bez niej wybór jak w sekcji „Obszar" |
| `look` | `colours` (domyślnie), `classes` (pasma co 1/12 rozpiętości), `shading` (szary relief) |
| `exaggeration` | pionowe wzmocnienie sceny 3D, 0–20; domyślnie 1, czyli prawdziwe wysokości |
| `terrain` | `false` zostawia teren sceny bez zmian (sam raster, bez podnoszenia 3D) |
| `colours` | starsza flaga: `true` = `classes`, `false` = `shading` |

**Stylizuj model** zmienia wygląd najnowszego modelu: `{"look": "classes"}` albo `{"look": "shading"}`.
**Teren wyłącz** (`dem-terrain-off`) wypłaszcza scenę, gdy model zniknął z projektu — inaczej scena
stoi na warstwie, której już nie ma. Komunikat po dodaniu podaje też rozpiętość wysokości
(na przykład „869 do 1113 m”), bo to ona decyduje, czy barwy i relief w ogóle coś pokażą.

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

### Ile to trwa i ile waży

GUGiK wysyła geometrię w pełnej rozdzielczości i ignoruje parametry uogólniania
(`MAP_RESOLUTION`, `resolution`), sprawdzone. Dla całego kraju:

| Zestaw | Obiekty | Rozmiar odpowiedzi | Czas serwera |
| --- | --- | --- | --- |
| Województwa | 16 | 8 MB | ~10 s |
| Powiaty | 380 | 42 MB | ~30 s |

Dlatego obszar większy niż 1,5° jest dzielony na 4 części pobierane **równolegle** (ok. 2–3×
szybciej), a duplikaty z sąsiednich części są usuwane po `gml:id` lub zasięgu geometrii. Do samego
tła kartograficznego w całej Polsce lepiej użyć nakładki **Granice PRG** (WMS), bo jest natychmiastowa.

Limity: działki i budynki do **0,15°**, granice administracyjne do **20°**, maks. 6000 obiektów.

## Sterowanie z agenta (MCP)

Wtyczka jest dostępna przez MCP bez osobnej warstwy, bo RustGIS uruchamia ją jako polecenia
i narzędzia geoprzetwarzania. Agent używa `run_command` i `run_tool`:

```
run_command  plugin:dane-pl/obszar-widok  {}
run_tool     plugin:dane-pl/pobierz-granice  {"dane": "A03_Granice_gmin", "obszar": "Obszar pobierania"}
run_command  plugin:dane-pl/przytnij  {}
run_command  plugin:dane-pl/dem  {"area": "obszar"}
```

Cztery polecenia pytają usługi GUGiK wprost i są przede wszystkim dla agentów (na wstędze jest
tylko Wysokość w punkcie, bo reszta potrzebuje argumentu, którego nie ma gdzie wpisać):

```
run_command  plugin:dane-pl/geokoduj    {"adres": "Poznań, Głogowska 10"}
run_command  plugin:dane-pl/wysokosc    {"x": 357873, "y": 506191}
run_command  plugin:dane-pl/dzialka-id  {"id": "306401_1.0001.14/4"}
run_command  plugin:dane-pl/eziudp      {"instytucja": "Geolog"}
```

- **Geokoduj** — UUG GUGiK, `services.gugik.gov.pl/uug`, JSON; dodaje punkt z miejscowością,
  ulicą, numerem, kodem i pozycją PL-1992.
- **Wysokość w punkcie** — `services.gugik.gov.pl/nmt`, `GetHByXY`, PL-KRON86-NH.
- **Działka z numeru** — ULDK `GetParcelById`, identyfikator w postaci `TERYT_R.OBRĘB.NUMER/ARK`.
- **Rejestr EZiUDP** — `integracja.gugik.gov.pl/eziudp/api.php?format=json`: kto zgłasza jaki
  zbiór i pod jakim adresem są jego WMS, WFS i CSW. Bez filtra rejestr ma 12 000 rekordów i 9 MB,
  więc polecenie **odmawia** i prosi o zawężenie, zamiast go pobierać.

Agent zaczyna od katalogu, więc nie musi czytać manifestu:

```
run_command  plugin:dane-pl/katalog  {}
run_command  plugin:dane-pl/katalog  {"instytucja": "pig"}
run_command  plugin:dane-pl/katalog  {"szukaj": "geolog"}
```

Odpowiedź to `services` (identyfikator polecenia, nazwa, instytucja, menu), `datasets`
(identyfikator, tytuł, narzędzie, limit obszaru), `indexes` (skorowidze) oraz `counts`.
Identyfikatory z tej odpowiedzi są dokładnie tymi, które przyjmuje `run_command` i `gp.run`.

`describe_tool` i `search_tools` widzą wtyczkę po instalacji. `script.run` (łączenie kilku
poleceń w jednym wywołaniu) jest zarezerwowane dla użytkownika i agent go nie użyje, dlatego
`przytnij` działa bez argumentów.

## Dlaczego proces, a nie WebAssembly

Usługi WFS GUGiK zwracają **wyłącznie GML** (sprawdzone: `OUTPUTFORMAT=application/json`
kończy się `400`), a sandbox wtyczek WASM pozwala pobrać dokument o rozmiarze do 2 MB. Działki
dla miasta to dziesiątki megabajtów, więc konwersja GML→GeoJSON i pobieranie muszą dziać się
poza sandboxem. To ta sama droga, którą idą wtyczki QGIS (*Pobieracz danych GUGiK*, *Wtyczka
GIS Support*).

Wtyczka procesowa **nie jest sandboxowana**: instalując ją, świadomie uruchamiasz program.
Nie wymaga Pythona ani GDAL-a: to jeden statyczny plik `.exe`.

## Języki

Wtyczka mówi dwoma językami: angielskim (baza) i polskim (tłumaczenie). Innych nie ma i nie
będzie: kod języka innego niż `pl` daje angielski.

- **Tekst interfejsu**: angielski w polach manifestu, polski w `i18n.pl`. Obejmuje `name`,
  `description`, etykiety i podpowiedzi poleceń, nazwy i opisy narzędzi, etykiety parametrów,
  zakładkę, grupy, menu, panel oraz wartości `choices`.
- **Tekst w czasie działania**: `src/text.rs`. Każde zdanie ma tam wersję angielską i polską,
  a wybiera ją `lang` z żądania. Dotyczy to meldunków, nazw warstw, postępu pobierania, błędów
  HTTP, błędów protokołu i komunikatów ULDK.
- **Liczebniki**: polskie formy są obsłużone (1 obiekt, 3 obiekty, 12 obiektów), angielskie też
  (1 feature, 3 features).
- **Nazwy zestawów danych zostają polskie w obu językach**: „Działki”, „Budynki”, „Ceny
  nieruchomości” to nazwy własne zbiorów GUGiK, nie słowa do tłumaczenia. Tłumaczone jest to, co
  wtyczka mówi od siebie: przyciski, menu, błędy, postęp.
- **`choices`**: wartością jest polska nazwa zestawu, a kod przyjmuje też identyfikator i nazwę
  typu, więc zapisane wywołanie działa niezależnie od języka interfejsu.

`rustgis-cli` zawsze wysyła `lang: "en"`, bo język ustawia tylko aplikacja z interfejsem. Wersję
polską sprawdza się na poziomie protokołu: podaj żądanie z `"lang": "pl"` na standardowe wejście
razem z odpowiedziami gospodarza.

## Katalog i manifest

`data/services.json` i `data/wfs.json` to katalogi. `plugin.json` jest z nich **generowany**:

```powershell
python tools/manifest.py            # zapisuje plugin.json
python tools/manifest.py --check    # kończy się kodem 1, gdy plik jest nieaktualny
```

Warstwa redakcyjna (co jest przyciskiem, w której grupie i menu, jak brzmią zdania, tłumaczenia)
siedzi w `tools/manifest.base.json`. Generator rozwija listy zestawów w `choices` narzędzi i
dokłada po jednym poleceniu na usługę WMS.

Przynależność do instytucji siedzi w `data/services.json` (`institution`, a dla GUGiK
`institutionMenu`), a nie w manifeście: nowa usługa trafia do właściwego menu sama. Klasy
menu (nazwa, ikona, podpowiedź) są w `tools/manifest.base.json`.

Ręcznie pisany manifest rozjechał się z katalogami: 162 z 270 typów WFS nie miało żadnego
polecenia, 211 poleceń niosło klucz `command`, którego gospodarz nie czyta (czyta `params`), a dwie
osnowy występowały pod jednym tytułem. Teraz brak zestawu w grupie albo nieznany identyfikator
w grupie zatrzymuje budowę manifestu, a test `the_download_tools_offer_every_dataset_exactly_once`
pilnuje tego samego od strony Rusta.

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
| `dist\dane-pl-0.6.0.rgplugin` | paczka do instalacji i publikacji |
| `dist\dane-pl.json` | wpis do rejestru w formacie, który przyjmuje `rustgis-plugins` |

Paczka jest **deterministyczna**: wpisy w archiwum są posortowane, znaczniki czasu stałe, prawa
plików ustawiane ręcznie (`0755` dla `bin/`), więc dwa przebiegi na tych samych binariach dają ten
sam SHA-256. Bez `--allow-missing` pakowanie wymaga binarki dla każdej platformy z `run` i kończy
się błędem, gdy jej brakuje.

Trzy platformy buduje `.github/workflows/release.yml` na tagu `v*`: Windows (`dane-pl.exe`), Linux
(`dane-pl-linux`) i macOS jako binarka uniwersalna (`dane-pl-macos`, `lipo` z aarch64 i x86_64).
Workflow pakuje je razem, sprawdza, że drugi przebieg daje ten sam SHA-256, i dołącza `.rgplugin`
oraz wpis rejestru do wydania.

## Zgodność z rejestrem

- `runtime: "process"`, bez `entry`; `run` wskazuje osobną binarkę na każdą platformę
  (`windows`, `macos`, `linux`) i każda z nich jest w paczce.
- Uprawnienia: `edit`, `network` i `files`. `files` jest potrzebne, bo siatki ASCII wysokości
  wchodzą na mapę przez `layer.addData` z `path`; `network` przez `map.addTiles` i `layer.addUrl`.
- Ikony to nazwy ikon RustGIS (`basemap`, `buffer`, `bookmark`, `label`, `relief`, `erase`,
  `g_rect`, `polygon_tool`, `line_tool`, `globe_view`, `table`, `map`, `zoom_world`, `history`);
  nieznana nazwa dawałaby puzzel.
- `params` każdego polecenia opisuje jego argumenty dla skryptów i agentów; wcześniej ten opis
  leżał w kluczu `command`, który gospodarz ignoruje.
- Wpis rejestru ma kształt wymagany przez `rustgis-plugins`: `id`, `source.url`, `source.sha256`
  i `categories`.

## Próba bez instalacji

```powershell
rustgis-cli run --plugin .\dist\dane-pl --sample `
  --cmd 'plugin:dane-pl/obszar-widok={}'
```

W aplikacji: **Plugins › Plugin Manager › Settings › Add Plugin Folder…** i wskaż
`dist\dane-pl`.

## Źródła danych i licencje

Dane: Główny Urząd Geodezji i Kartografii, [geoportal.gov.pl](https://www.geoportal.gov.pl),
udostępniane nieodpłatnie. Przy każdej warstwie ustawiana jest atrybucja
„GUGiK · geoportal.gov.pl”. Wtyczka nie pośredniczy w danych.

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
- Skorowidzów (162 typy) nie da się pobrać: to indeksy paczek, nie dane mapowe.
- Brak pobierania paczek LAZ / NMT / ortofoto na dysk (to materiał na kolejną wersję).
- WFS GUGiK nie ma GeoJSON-u, więc konwersja jest lokalna; geometrie inne niż
  Point/LineString/Polygon i ich wersje multi nie są obsługiwane.
- ULDK przyjmuje tylko EPSG:2180, więc wtyczka sama rzutuje współrzędne (Transverse Mercator,
  zgodność z PROJ na poziomie < 1 mm dla punktu testowego).
- Prostokąta nie da się narysować przeciągnięciem: RustGIS nie ma narzędzia prostokąta dla
  warstw edytowalnych. Prostokąt bierze się z widoku (**Obszar z widoku**), a rysowanie zostawia
  kształt, jaki narysujesz.
- Wtyczka nie dodaje pozycji do menu kontekstowego mapy: manifest RustGIS 0.4 nie ma na to pola.
  Pobieranie idzie więc przez grupę **Dane › Pobieranie**, a nie spod prawego przycisku.
