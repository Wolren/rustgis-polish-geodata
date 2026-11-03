# Roadmap

_Stan na 2026-11-03, wtyczka 0.6.0._

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
  pobieranie to pięć osobnych przycisków. 10 kolumn zamiast 28; w polskim interfejsie wstęga
  kończy się na 1353 px z 1480. Wymaga RustGIS 0.5.0 (zagnieżdżone menu i kolumny `small`),
  stąd `minRustgisVersion`.
- **Przycinanie do dowolnej granicy.** Panel ma drzewo warstw poligonowych z liczbą obiektów
  i wyszukiwarką; wybór ustawia granicę, a wiersz **Przytnij** pokazuje parę
  „warstwa → granica”. Cięta jest warstwa aktywna, a bez niej najnowsza.
- **Panel bez wypychania przycisków.** Statusy, para „warstwa → granica” i drzewo granicy stoją
  nad przyciskami, a **Odśwież panel** czyta widok, obszar, katalog i zapisane odpowiedzi od nowa.
  Każde narzędzie pobierania ma parametr `clear` zamiast pola wyboru w panelu.

## Następne

### Dane

- **Wody Polskie: ~900 warstw w 32 usługach WMS.** Dziś w katalogu jest 1 z nich (MZP20,
  zagrożenie powodziowe), a rejestr eZiUDP zna 19 zbiorów Wody Polskie, 15 z adresem WMS.
  Jedyna instytucja, przy której model
  „jedna usługa = jedno polecenie" się nie skaluje. Potrzebny wybór warstwy wewnątrz usługi
  (parametr `warstwa` w poleceniu + lista z capabilities), zamiast 900 poleceń.
  Wejście: `wody.isok.gov.pl/gpservices/...`, wtyczka QGIS „Wody Polskie — Baza WMS".
- **Wody Polskie: geobazy do pobrania** (PGW, WORP, PZRP, PPSS, MZP) — pliki, nie WMS.
- **Pozostałe warstwy Cartomatic** — serwer podaje 110 warstw, wciągnięto 11 (WIG i mapy
  niemieckie). Zostały plany miast i mapy katastralne.
- **Wody/geologia/ochrona środowiska** — przejrzeć EZiUDP po instytucjach i dobrać brakujące
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
  dlatego `geokoduj`, `dzialka-id` i `eziudp` są tylko dla skryptów i agentów.

### Interfejs

- **`eziudp` → dodanie usługi z rejestru.** Rejestr zna adresy WMS/WFS; wtyczka mogłaby
  dodać usługę spoza katalogu, gdyby host przyjmował szablon kafli zbudowany w locie.
  To samo dotyczy „Integratora usług danych przestrzennych" z przeglądu wtyczek 2024.
