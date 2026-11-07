# Dane PL: polskie dane geodezyjne w RustGIS

Wtyczka do [RustGIS](https://rustgis.app): jedna zakładka **Dane PL** z podkładami, usługami
i danymi GUGiK, Wody Polskie oraz Cartomatic, bez kluczy API i bez pośredników.

Zasada, która trzyma tę wtyczkę w kupie: **polecenie tylko pokazuje coś na mapie, a pobiera
narzędzie.** Wstęga mówi to wprost nazwami grup: **WMS (podgląd)** tylko wyświetla, **Pobieranie**
jako jedyne ściąga dane wektorowe i pliki, **Wyszukiwanie** pyta usługę punktową, **Narzędzia**
zmieniają projekt. Polecenie wykonuje się w wątku interfejsu, więc pobieranie jako polecenie
zamrażało okno na czas transferu. Dlatego na wstędze nie ma już 108 przycisków pobierania: jest
dziewięć narzędzi w panelu Narzędzia, wszystkie w tle, z postępem i przyciskiem Anuluj, w tym pięć
pobierających zbiory WFS i cztery od paczek.

Jeden wyjątek jest opisany niżej: trzy polecenia w menu **Model** grupy **Wysokości** pobierają
raster. Narzędzie geoprzetwarzania nie umie dodać warstwy rastrowej, więc raster pobiera i kładzie
na mapie polecenie.

Wszystkie grupy jadą w układzie `small`, czyli trzy kontrolki w kolumnie, a katalog jest zwinięty
najpierw według instytucji: GUGiK to jedna pozycja z czterema tematami w podmenu. Wstęga ma przez
to 11 kolumn zamiast 28 i mieści się w oknie 1480 px bez strzałki przewijania. Zmierzone na
zrzucie z `ui_shot` (1480 x 760, polski interfejs): treść kończy się na 1390 px, po zwinięciu
**Wysokości** i **Wyszukiwania** w jedno menu każde. Przed zwinięciem było 1439 px.

| Grupa | Pozycja | Co robi |
| --- | --- | --- |
| **Podkłady** | Rastry | Ortofoto (WMTS), Ortofoto Hi-Res (WMS), BDOT10k (nakładka), Cieniowanie (NMT) |
| | Wektory | Działki (KIEG), Adresy, Granice PRG, Nazwy geograficzne |
| **Obszar** | **Obszar z widoku** | Prostokąt z widoku mapy: jedno kliknięcie, bez rysowania. Dokładnie to, co widać |
| | Narysuj obszar | Narzędzie poligonu na warstwie „Obszar pobierania”; kształt zostaje taki, jak narysowany |
| | Narysuj prostokąt | Narzędzie prostokąta z aplikacji; rysuje grafikę na warstwie „Rysunek obszaru” |
| | Prostokąt do obszaru | Przenosi narysowany prostokąt do „Obszar pobierania” i usuwa warstwę rysunku |
| | Anuluj rysowanie | Wyłącza narzędzie rysowania i wraca do przeglądania |
| | Usuń obszar | Usuwa warstwę „Obszar pobierania”, żeby zacząć od pustej |
| **Narzędzia** | Panel | Panel ze statusami (widok, obszar, para warstwa → granica), przyciskami, drzewem warstw granicznych, drzewem map z wyszukiwarką, katalogiem i zapisanymi odpowiedziami; **Odśwież panel** czyta je od nowa |
| | Przytnij do warstwy | Tnie warstwę do wybranej w panelu granicy, domyślnie do „Obszar pobierania” |
| **WMS (podgląd)** | GUGiK | 76 usług w czterech tematach: Ortofoto i NMT (20), Mapy (21), Kataster i granice (24), Pozostałe dane (11) |
| | Mapy historyczne | 11 arkuszy WIG i map niemieckich |
| | Geologia (PIG) | 9 usług |
| | Zabytki (NID) | 4 usługi |
| | Lasy (BDL) | 4 usługi |
| | Pozostałe instytucje | GDOŚ, PKP PLK, GUS, Wody Polskie, IMGW-PIB |
| | Więcej map | 30 usług Wody Polskie (890 warstw) w trzech folderach i 45 map Cartomatic w dwóch folderach |
| **Pobieranie** | Adresy i budynki | 19 zbiorów WFS |
| | Granice i jednostki | 24 zbiory |
| | Osnowy i pomiary | 21 zbiorów |
| | Urzędy i sądy | 21 zbiorów |
| | Pozostałe dane | 21 zbiorów |
| | Pakiety | Trzy skorowidze arkuszy (ortofoto, NMT, pozostałe) i pobieranie arkuszy do folderu |
| **Wysokości** | Model | Teren KRON86 (GeoTIFF), teren EVRF2007, pokrycie NMPT i ustawianie stylu modelu |
| **Wyszukiwanie** | Wyszukiwanie | Działka z ULDK i wysokość w punkcie |

Łącznie: **211 poleceń** (27 własnych, 109 usług katalogu krajowego, 30 usług Wody Polskie i 45 map
Cartomatic) oraz **9 narzędzi**. Wcześniej było 228 poleceń, z czego 108 pobierało dane natychmiast,
blokując okno.

### Usługi krajowe

Katalog obejmuje **109 usług WMS** z Geoportalu Krajowego i od innych instytucji: BDOT10k (w tym nowe
budynki i statystyki), ortofotomapę prawdziwą, wysokiej rozdzielczości i archiwalną, cieniowanie
NMT, hipsometrię NMT, EMUiA, PRG, PRNG, RCN, KIMPZP, ISOK, POLSA, osnowy, mapy geologiczne PIG,
BDL Lasów Państwowych, zabytki NID i inne. Każda pozycja przeszła realne żądanie kafla, więc lista
nie zawiera martwych adresów.

Usługi stoją w grupie **WMS (podgląd)**: **GUGiK** to jedna pozycja z czterema tematami w podmenu,
a każda pozostała instytucja ma własne menu. Podział jest instytucjonalny, bo tak szukają danych
użytkownicy: najpierw *kto publikuje*, potem *co*. Naciśnięcie nazwy wjeżdża na mapę jako warstwa
kafelkowa, drugie naciśnięcie ją zdejmuje. Każda usługa to osobne polecenie, więc działa też
wyszukiwarka poleceń: wpisz „cieniowanie” albo „ortofoto” i naciśnij wynik.

```
run_command  plugin:dane-pl/usluga-cieniowanie-wms
run_command  plugin:dane-pl/usluga-ceny-nieruchomosci
```

Osobne menu **Mapy historyczne** to jedenaście arkuszy z serwera Cartomatic: WIG 1:25 000,
1:100 000, 1:300 000 i 1:500 000 oraz mapy niemieckie (Karte des Deutschen Reiches 1:100 000,
Meßtischblätter 1:25 000, Übersichtskarte von Mitteleuropa 1:300 000). Każdy arkusz przeszedł
realne żądanie kafla, tak jak reszta katalogu. Serwer podaje 110 warstw, z których wtyczka bierze
56: jedenaście historycznych stoi w tym menu, a 45 (plany miast i atlasy oraz mapy topograficzne
i przeglądowe) w menu **Więcej map**.

### Wody Polskie i seria Cartomatic

Menu **Więcej map** zbiera dwie rodziny, które nie mieszczą się w katalogu instytucji: 30 usług Wody
Polskie (890 warstw) i 45 map Cartomatic. Wody Polskie dzielą się na trzy foldery, a o przynależności
decyduje identyfikator usługi, nie jej tytuł:

| Folder | Co się w nim kryje | Przycisków |
| --- | --- | --- |
| Wody: PGW | plany gospodarowania wodami (PGWWP, aPGW) i MPHP | 3 |
| Wody: powódź | zagrożenie powodziowe (WORP, MZP) | 13 |
| Wody: ryzyko | ryzyko powodziowe (MRP, PZRP, PPSS) | 14 |
| Plany miast i atlasy | plany miast i atlasy z serwera Cartomatic | 20 |
| Mapy topograficzne i przeglądowe | mapy topograficzne i przeglądowe z tego samego serwera | 25 |

Podział bierze się z prefiksu: `wody-pgwwp`, `wody-apgw*` i `wody-mphp*` idą do **Wody: PGW**,
`wody-worp*` i `wody-mzp*` do **Wody: powódź**, a `wody-mrp*`, `wody-pzrp` i `wody-ppss*` do
**Wody: ryzyko**. Rodzina, której żaden prefiks nie łapie, trafia do **Wody: inne**, więc nowa usługa
w katalogu nie wywala builda. Funkcja `wody_folder` w `tools/manifest.py` trzyma tę mapę, a każdy
folder mieści najwyżej 32 pozycje, bo tyle przyjmuje menu gospodarza.

Tytuły usług sięgają 122 znaków i taki wiersz rozwlekał popup na cały ekran. Etykieta łamie się
więc na spacji co 44 znaki (`wrapped` w `tools/manifest.py`): wiersz menu zachowuje złamanie, a mały
przycisk zamienia je na spację, więc oba czytają to samo.

Przycisk to jedna usługa albo jedna mapa: naciśnięcie kładzie na mapę wszystkie jej warstwy, drugie
je zdejmuje. Podpowiedź niesie atrybucję wydawcy („PGW Wody Polskie (ISOK) · 11 warstw”,
„Cartomatic / IH PAN”), więc wiadomo, kto publikuje obraz.

Warstwy dobiera się po usłudze, nie pojedynczo. Serwer ISOK trzyma po kilkadziesiąt warstw pod
jednym adresem, a „jedna warstwa = jedno polecenie” dałoby 890 przycisków, więc jedna usługa to
jedno polecenie. Wybór pojedynczej warstwy w środku usługi zostaje na później (patrz Ograniczenia).

### Dane z WFS

Katalog obejmuje **268 typów WFS** z krajowych usług pobierania: adresy i ulice z KI NA, budynki
i działki z EGiB, granice z PRG, osnowy, urzędy, sądy, kolej, obiekty morskie i statystyki BDOT.
Każdy typ przeszedł realne żądanie GetFeature, więc lista nie zawiera martwych adresów.

**106 z nich to dane mapowe** i te pokazują się w narzędziach pobierania. Pozostałe **162 to
skorowidze** (arkusze i siatki, które wskazują paczki do pobrania z Geoportalu) i te stoją w menu
**Pakiety**: trzy narzędzia dodają skorowidz jako warstwę, a czwarte pobiera pliki, które wskazują
jego obiekty. Skorowidz nadal da się wskazać po identyfikatorze, np.
`dane-skorowidz-danych-pomiarowych-lidar-2024`.

Pobieranie to **pięć narzędzi**, każde jako osobny przycisk w grupie **Pobieranie**. Każde ma listę
swoich zbiorów:

| Narzędzie | Zbiory |
| --- | --- |
| Adresy i budynki | 19 |
| Granice i jednostki | 24 |
| Osnowy i pomiary | 21 |
| Urzędy i sądy | 21 |
| Pozostałe dane | 21 |

1. Wskaż obszar: **Obszar z widoku**, **Narysuj obszar** albo własna warstwa.
2. Naciśnij przycisk w grupie **Pobieranie**, na przykład **Adresy i budynki**.
3. W panelu Narzędzia wybierz zbiór i warstwę obszaru, naciśnij **Uruchom**.

Narzędzie działa w tle: panel pokazuje postęp i ma przycisk przerwania. Wynik wjeżdża na mapę jako
warstwa wektorowa, jednym krokiem cofania. Opcje w formularzu to zbiór, warstwa obszaru i
opcjonalne **Najpierw usuń zapisane odpowiedzi**: nic więcej nie trzeba ustawiać, bo nic więcej nie
jest zgadywane.

Skrypt albo agent uruchamia to samo przez `gp.run`:

```
run_tool  plugin:dane-pl/pobierz-adresy  {"dane": "Budynki", "obszar": "Obszar pobierania"}
run_tool  plugin:dane-pl/pobierz-granice  {"dane": "A03_Granice_gmin", "obszar": "obszar"}
```

Typy spoza układu 4326 działają bez ustawień: wtyczka wysyła pierwsze żądanie we współrzędnych
geograficznych, a gdy usługa odpowie pustą kolekcją, powtarza je w układzie 2180 i przelicza
współrzędne z powrotem. Dotyczy to usługi adresowej KI NA.

### Pakiety: skorowidz, folder, CSV

Menu **Pakiety** w grupie **Pobieranie** zamienia skorowidz w listę arkuszy do pobrania:

| Narzędzie | Co robi | Wybór |
| --- | --- | --- |
| Skorowidze: ortofoto | dodaje skorowidz ortofotomap jako warstwę | 54 zbiory, obszar |
| Skorowidze: NMT | to samo dla modeli terenu i pokrycia | 44 zbiory, obszar |
| Skorowidze: inne | LIDAR, zdjęcia lotnicze i siatki arkuszy | 64 zbiory, obszar |
| Pobierz arkusze | pobiera pliki wskazane przez obiekty skorowidza | warstwa, folder, `clear`, `confirm` |

Kolejność jest zawsze ta sama:

1. Wskaż obszar, tak jak przy każdym pobieraniu: **Obszar z widoku**, **Narysuj obszar** albo
   własna warstwa.
2. Uruchom jedno z trzech narzędzi skorowidza, wybierz zbiór (np. „Skorowidz ortofotomapy 2024”)
   i warstwę obszaru. Skorowidz wjeżdża na mapę jako warstwa arkuszy.
3. Zaznacz arkusze, które chcesz mieć, albo zostaw całą warstwę: bez zaznaczenia liczy się każdy
   obiekt.
4. Uruchom **Pobierz arkusze**: wskaż warstwę skorowidza i folder.
5. Pliki lądują w folderze, a obok nich raport CSV z listą arkuszy i wynikiem każdego pobrania.

**Rozmiar przed pobraniem.** Pierwsza linia postępu mówi, ile plików i ile megabajtów (co najmniej,
bo skorowidz zaokrągla `wlk_pliku_mb`). Powyżej 0,5 GB narzędzie odmawia i podaje policzony rozmiar;
duży przebieg jest decyzją, więc trzeba zaznaczyć **Pobierz mimo rozmiaru** i uruchomić ponownie.
Nic się wtedy jeszcze nie pobrało, więc to prawdziwe potwierdzenie, a nie ostrzeżenie po fakcie.

**Cztery pliki naraz.** Pobieranie idzie czterema wątkami, bo to wiele niezależnych plików z jednego
hosta: 3 GB w kilkaset sekund zamiast kilkunastu minut. Każdy plik leci do `.part` i wraca pod
właściwą nazwę po skończeniu, więc przerwany przebieg nie zostawia połowy archiwum.

**Tło i Anuluj.** Wszystkie narzędzia pobierania uruchamiane z panelu **Narzędzia** działają w tle:
okno zostaje responsywne, widać postęp, jest **Anuluj**, który zabija proces. Dlatego wiersze
„Pobieranie” w drzewie **Mapa** w panelu nie startują pobrania same: polecenie nie umie uruchomić
narzędzia w tle, więc wiersz tylko wskazuje narzędzie i grupę, w której ono stoi. Wyjątkiem od tej
zasady są trzy polecenia **Wysokości**: pobierają raster na wątku interfejsu, bo narzędzie nie umie
dodać warstwy rastrowej, i dlatego mają budżet 12 s i próg rozmiaru.


Adres każdego pliku bierze się z pola `url_do_pobrania` obiektu skorowidza. Wartością wyboru w
trzech narzędziach skorowidza jest tytuł zbioru, więc dwa zbiory o tym samym tytule niosą
w nawiasie identyfikator.

Skrypt robi to samo:

```
run_tool  plugin:dane-pl/pobierz-skorowidz-orto  {"dane": "Skorowidz ortofotomapy 2024", "obszar": "Obszar pobierania"}
run_tool  plugin:dane-pl/pobierz-arkusze  {"skorowidz": "Skorowidz ortofotomapy 2024", "folder": "D:/orto"}
```

### Obszar: nic nie jest zgadywane

Narzędzie pobierania **wymaga warstwy obszaru**. Nie ma już łańcucha „aktywna warstwa, jej
zaznaczenie, największe zaznaczenie w projekcie, warstwa obszaru, a w ostateczności cała aktywna
warstwa”. To ostatnie ogniwo potrafiło pobrać województwo po kliknięciu w przycisk, a przedostatnie
brało obszar z warstwy, której użytkownik nie widział.

Model wysokości (polecenie, nie narzędzie) bierze obszar w tej kolejności:

1. warstwa podana w argumencie `area`,
2. zaznaczenie na aktywnej warstwie,
3. warstwa „Obszar pobierania” z przycisku **Obszar z widoku**,
4. aktywna warstwa w całości.

Zaznaczenie na innej warstwie nie jest brane pod uwagę. Zbyt duży obszar jest odrzucany z liczbami,
zanim poleci jakiekolwiek żądanie.

**Obszar z widoku** to jedno kliknięcie: wtyczka bierze bieżący widok mapy i zapisuje go jako
prostokąt na warstwie „Obszar pobierania”. Prostokąt jest dokładnie tym, co widać — żadnego
rysowania i żadnego zgadywania. Chcesz inny obszar? Przybliż mapę i naciśnij ponownie.

**Narysuj obszar** zostawia kształt taki, jak narysowany: klikaj narożniki i zakończ dwuklikiem
(kliknięcie w pierwszy narożnik nie zamyka obwodu — tak działa narzędzie RustGIS), albo przerwij
Escape. Pobieranie idzie za narysowanym kształtem. Model wysokości jest natomiast zawsze
prostokątem, więc obejmuje prostokąt wokół kształtu i wtyczka mówi wtedy, jaki to prostokąt, a
warstwa obszaru pokazuje go zamiast rysunku.

**Narysuj prostokąt** uzbraja narzędzie prostokąta z aplikacji: przeciągnij po mapie, a RustGIS
narysuje grafikę na warstwie „Rysunek obszaru”. **Prostokąt do obszaru** przenosi ją do „Obszar
pobierania” i usuwa warstwę rysunku. To samo przeniesienie robi każde następne polecenie wtyczki,
więc naciśnięcie dowolnego przycisku w panelu wystarcza. Narzędzie pobierania czyta warstwę obszaru
z chwili startu, dlatego przy nim potrzebne jest jawne **Prostokąt do obszaru**.

**Usuń obszar** usuwa warstwę, więc kolejny prostokąt albo rysunek zaczyna od zera. Bez tego
drugi rysunek dokłada kształt do istniejącego i obszar się powiększa.

To rozróżnienie jest jedyną prawdą, jaką wtyczka może powiedzieć: RustGIS rysuje obiekty
edytowalne punkt po punkcie, a jego narzędzie prostokąta rysuje grafikę, nie obiekt warstwy
wektorowej. Dlatego prostokąt z rysunku wymaga jednego kroku przeniesienia, a prostokąt z widoku
jest zapisywany od razu.

### Przycinanie do granicy

Filtr WFS zwraca **całe** obiekty, które dotykają obszaru i nie przycina ich. Dlatego po pobraniu
uruchom **Przytnij do warstwy**: wywołuje wbudowane narzędzie Clip i zastępuje warstwę wersją
przyciętą (jedna warstwa, nie dwie).

Granicą nie musi być warstwa obszaru. Panel ma na górze statusy (widok, obszar, para
„warstwa → granica”), pod nimi drzewo warstw poligonowych z liczbą obiektów i wyszukiwarką, a pod
drzewem przyciski, katalog i zapisane odpowiedzi. Wybierz w drzewie dowolną warstwę graniczną
(gminę, obręb, działkę, własny poligon), a wiersz **Przytnij** pokaże, co zostanie przycięte i do
czego, na przykład `Adresy → Gmina Poznań`. Przycisk **Przytnij do warstwy** robi wtedy to, co
widać. Bez wyboru granicą jest warstwa „Obszar pobierania”.

Pod drzewem granic stoi drugie drzewo, **Mapa**, też z wyszukiwarką i 304 wierszami. W **Podkładach**
jest każda mapa, którą wtyczka umie pokazać: siedem nakładek własnych, 109 usług katalogu, 30 usług
Wody Polskie i 45 map Cartomatic. W **Pobieraniu** jest każdy zbiór, który umie pobrać, w pięciu
folderach narzędzi. Wiersz niesie źródło w identyfikatorze: `v:<polecenie>` to mapa, a
`d:<narzędzie>:<zbiór>` to pobieranie. Wybór wiersza uruchamia polecenie **Mapa** (`mapa`): `v:`
wjeżdża tym samym poleceniem, które stoi na wstędze, a `d:` startuje narzędzie pobierania z tytułem
zbioru i warstwą obszaru (albo warstwą aktywną, gdy obszaru nie ma). Skrypt robi to samo:

```
run_command  plugin:dane-pl/mapa  {"panel": {"mapa": "v:bdot"}}
run_command  plugin:dane-pl/mapa  {"panel": {"mapa": "d:pobierz-granice:dane-a03-granice-gmin"}}
```

Bez wiersza polecenie odmawia i prosi o wskazanie wiersza. Pobranie z listy trzyma interfejs na czas
transferu: polecenie nie ma tła, więc Anuluj nie działa, a postęp dochodzi do paska statusu dopiero po
zakończeniu. Formularz narzędzia zostaje wygodniejszy, bo działa w tle i ma przycisk przerwania.

**Odśwież panel** czyta od nowa widok, warstwę obszaru, katalog i zapisane odpowiedzi, więc
odświeża panel także wtedy, gdy mapa zmieniła się poza nim.

Cięta jest warstwa aktywna w panelu **Warstwy**, a gdy żadna nie jest aktywna — najnowsza warstwa
wektorowa. Uchwyt `layer` (albo `obszar`) w poleceniu nadpisuje jedno i drugie, więc skrypt nie
zależy od stanu panelu.

Pomiar: 5586 działek przecinało obszar, po przycięciu zostało 5583, dla działek różnica jest
mała, dla dróg i obiektów wielkości gminy duża.

### Model wysokości z obszaru

Grupa **Wysokości** ma trzy pokrycia, w menu **Model**:

| Pozycja | Pokrycie | Format |
| --- | --- | --- |
| Model terenu KRON86 | `DTM_PL-KRON86-NH_TIFF` | GeoTIFF |
| Model terenu EVRF2007 | `DTM_PL-EVRF2007-NH` | siatka ASCII |
| Model pokrycia NMPT | `DSM_PL-KRON86-NH` | siatka ASCII |

Te trzy pozycje to **polecenia**, nie narzędzia: pobierają raster i od razu kładą go na mapie.
To jedyny wyjątek od zasady „polecenie pokazuje, narzędzie pobiera”, bo narzędzie nie umie dodać
warstwy rastrowej.

GeoTIFF pobiera sam RustGIS i bierze z niego układ odniesienia. Siatka ASCII nie niesie układu, więc
wtyczka pobiera ją sama i dopisuje plik `.prj` z EPSG:2180. Model trafia na mapę z cieniowaniem,
a **Ustaw styl modelu** zmienia je na barwy.

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

| Zbiór | Obiekty | Rozmiar odpowiedzi | Czas serwera |
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
- **Nazwy zbiorów danych zostają polskie w obu językach**: „Działki”, „Budynki”, „Ceny
  nieruchomości” to nazwy własne zbiorów GUGiK, nie słowa do tłumaczenia. Tłumaczone jest to, co
  wtyczka mówi od siebie: przyciski, menu, błędy, postęp.
- **`choices`**: wartością jest polska nazwa zbioru albo tytuł skorowidza, a kod przyjmuje też
  identyfikator i nazwę typu, więc zapisane wywołanie działa niezależnie od języka interfejsu.

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
siedzi w `tools/manifest.base.json`. Generator rozwija listy zbiorów w `choices` narzędzi,
dokłada po jednym poleceniu na usługę WMS, na usługę Wody Polskie i na mapę Cartomatic, i składa
menu **Więcej map**: foldery serii Cartomatic biorą angielską nazwę z manifestu, a polską z `i18n`,
a foldery Wody Polskie wybiera prefiks identyfikatora usługi (`wody_folder`), z etykietami łamanymi
co 44 znaki (`wrapped`).

Przynależność do instytucji siedzi w `data/services.json` (`institution`, a dla GUGiK
`institutionMenu`), a nie w manifeście: nowa usługa trafia do właściwego menu sama. Dwie nowe
rodziny mają własne pliki, `data/wody.json` i `data/cartomatic.json`, z atrybucją i głębokością
zbliżenia dla całej rodziny. Klasy menu (nazwa, ikona, podpowiedź) są w `tools/manifest.base.json`.

Tytuły usług Wody Polskie i map Cartomatic są nazwami własnymi, więc zostają polskie w obu
językach i nie mają wpisów w `i18n`; generator pomija je razem z etykietami usług GUGiK.

Ręcznie pisany manifest rozjechał się z katalogami: 162 z 270 typów WFS nie miało żadnego
polecenia, 211 poleceń niosło klucz `command`, którego gospodarz nie czyta (czyta `params`), a dwie
osnowy występowały pod jednym tytułem. Teraz brak zbioru w grupie albo nieznany identyfikator
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
  `g_rect`, `polygon_tool`, `line_tool`, `polygon_layer`, `globe_view`, `table`, `map`, `contents`,
  `package`, `zoom_world`, `history`); nieznana nazwa dawałaby puzzel.
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
- Skorowidz (162 typy) sam nie jest danymi mapowymi: to siatka arkuszy. Wtyczka doda go jako
  warstwę i pobierze pliki, które wskazują jego obiekty, ale nie narysuje arkuszy na mapie.
- Nie pobieramy LAZ ani geobaz z dane.gov.pl (PGW, WORP, PZRP, PPSS, MZP): pobierane jest tylko to,
  co skorowidz wskazuje w polu `url_do_pobrania`.
- Usługa Wody Polskie włącza się w całości: nie ma wyboru pojedynczej warstwy w środku usługi, więc
  jeden przycisk to wszystkie jej warstwy naraz.
- WFS GUGiK nie ma GeoJSON-u, więc konwersja jest lokalna; geometrie inne niż
  Point/LineString/Polygon i ich wersje multi nie są obsługiwane.
- ULDK przyjmuje tylko EPSG:2180, więc wtyczka sama rzutuje współrzędne (Transverse Mercator,
  zgodność z PROJ na poziomie < 1 mm dla punktu testowego).
- Prostokąta nie da się narysować przeciągnięciem: RustGIS nie ma narzędzia prostokąta dla
  warstw edytowalnych. Prostokąt bierze się z widoku (**Obszar z widoku**), a rysowanie zostawia
  kształt, jaki narysujesz.
- Wtyczka nie dodaje pozycji do menu kontekstowego mapy: manifest RustGIS 0.5.0 nie ma na to pola.
  Pobieranie idzie więc przez grupę **Dane › Pobieranie**, a nie spod prawego przycisku.
