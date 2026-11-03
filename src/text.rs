//! Every sentence the plugin shows, in English and Polish.
//!
//! RustGIS puts the interface language's code on each request (`lang`), so a command or a tool
//! answers in that language. Two languages only: English is the base, Polish is the translation,
//! and any other code falls back to English.

use crate::filter::ShapeProblem;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Pl,
}

impl Lang {
    /// The language of a request's `lang` code, English for anything but Polish.
    pub fn from_code(code: &str) -> Lang {
        let base = code
            .split(['-', '_'])
            .next()
            .unwrap_or(code)
            .to_ascii_lowercase();
        if base == "pl" {
            Lang::Pl
        } else {
            Lang::En
        }
    }
}

/// Picks the English or the Polish sentence.
macro_rules! pick {
    ($lang:expr, $en:expr, $pl:expr $(,)?) => {
        match $lang {
            Lang::En => $en,
            Lang::Pl => $pl,
        }
    };
}

/// 0 for one, 1 for the few of Polish 2 to 4, 2 for the rest.
fn form(lang: Lang, n: usize) -> usize {
    if n == 1 {
        return 0;
    }
    match lang {
        Lang::En => 2,
        Lang::Pl => {
            if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
                1
            } else {
                2
            }
        }
    }
}

/// "1 feature", "3 features", "3 obiekty", "12 obiektów".
pub fn features(lang: Lang, n: usize) -> String {
    let words = pick!(
        lang,
        ["feature", "features", "features"],
        ["obiekt", "obiekty", "obiektów"]
    );
    format!("{n} {}", words[form(lang, n)])
}

pub fn files(lang: Lang, n: usize) -> String {
    let words = pick!(
        lang,
        ["file", "files", "files"],
        ["plik", "pliki", "plików"]
    );
    format!("{n} {}", words[form(lang, n)])
}

pub fn areas(lang: Lang, n: usize) -> String {
    let words = pick!(
        lang,
        ["area", "areas", "areas"],
        ["obszar", "obszary", "obszarów"]
    );
    format!("{n} {}", words[form(lang, n)])
}

pub fn datasets(lang: Lang, n: usize) -> String {
    let words = pick!(
        lang,
        ["dataset", "datasets", "datasets"],
        ["zbiór", "zbiory", "zbiorów"]
    );
    format!("{n} {}", words[form(lang, n)])
}

pub fn sheets(lang: Lang, n: usize) -> String {
    let words = pick!(
        lang,
        ["sheet index", "sheet indexes", "sheet indexes"],
        ["skorowidz", "skorowidze", "skorowidzów"]
    );
    format!("{n} {}", words[form(lang, n)])
}

/// A quantity with the language's decimal separator: `1.5` in English, `1,5` in Polish.
pub fn num(lang: Lang, value: f64, places: usize) -> String {
    let text = format!("{:.1$}", value, places);
    match lang {
        Lang::En => text,
        Lang::Pl => text.replace('.', ","),
    }
}

/// "Layer: 12 features".
pub fn count(lang: Lang, name: &str, n: usize) -> String {
    format!("{name}: {}", features(lang, n))
}

/// The layer the download takes as its area, named in the interface language.
pub fn area_layer(lang: Lang) -> &'static str {
    pick!(lang, "Area to download", "Obszar pobierania")
}

/// Both names, so a layer made in either language is still found.
pub const AREA_LAYERS: [&str; 2] = ["Area to download", "Obszar pobierania"];

pub fn unknown_command(lang: Lang, id: &str) -> String {
    pick!(
        lang,
        format!("Polish Geodata has no command '{id}'"),
        format!("Wtyczka Dane PL nie ma polecenia „{id}”")
    )
}

pub fn cache_cleared(lang: Lang, count: usize, megabytes: f64) -> String {
    let size = num(lang, megabytes, 0);
    pick!(
        lang,
        format!("WFS cache: removed {} ({size} MB)", files(lang, count)),
        format!("Cache WFS: usunięto {} ({size} MB)", files(lang, count))
    )
}

pub fn layers_unexpected(lang: Lang) -> String {
    pick!(
        lang,
        "RustGIS sent the layers in an unexpected shape",
        "RustGIS podało warstwy w nieoczekiwanej postaci"
    )
    .to_string()
}

pub fn nothing_to_clip(lang: Lang) -> String {
    pick!(
        lang,
        "there is no layer to clip, download data first",
        "nie ma warstwy do przycięcia, najpierw pobierz dane"
    )
    .to_string()
}

pub fn no_layer(lang: Lang, layer: &str) -> String {
    pick!(
        lang,
        format!("there is no layer \"{layer}\" to clip"),
        format!("nie ma warstwy „{layer}” do przycięcia")
    )
}

pub fn no_area_layer(lang: Lang, area: &str) -> String {
    pick!(
        lang,
        format!("there is no area layer \"{area}\""),
        format!("nie ma warstwy obszaru „{area}”")
    )
}

pub fn clipped_suffix(lang: Lang) -> &'static str {
    pick!(lang, " (clipped)", " (po przycięciu)")
}

pub fn clipped(lang: Lang, layer: &str, area: &str, kept: usize) -> String {
    pick!(
        lang,
        format!(
            "Clipped \"{layer}\" to \"{area}\": {}",
            features(lang, kept)
        ),
        format!("Przycięto „{layer}” do „{area}”: {}", features(lang, kept))
    )
}

/// The cut kept nothing, so the layer stays as it was and the empty result goes.
pub fn clipped_empty(lang: Lang, layer: &str, area: &str) -> String {
    pick!(
        lang,
        format!(
            "the boundary \"{area}\" covered nothing of \"{layer}\": no feature is kept, so the layer is unchanged"
        ),
        format!(
            "granica „{area}” nie objęła żadnego obiektu warstwy „{layer}”: nic nie zostało zachowane, więc warstwa jest bez zmian"
        )
    )
}

/// The host answered a clip without saying which layer it made.
pub fn clipped_unreported(lang: Lang) -> String {
    pick!(
        lang,
        "RustGIS did not say which layer the clip made",
        "RustGIS nie podało, którą warstwę utworzyło przycięcie"
    )
    .to_string()
}

/// The boundary picked in the panel is no longer on the map, so the press reports it.
pub fn boundary_gone(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("the boundary layer \"{name}\" is no longer on the map, choose another"),
        format!("warstwy granicy „{name}” nie ma już na mapie, wybierz inną")
    )
}

/// What the Clip button will do: the layer first, then the boundary it cuts to.
pub fn clip_text(layer: &str, area: &str) -> String {
    format!("{layer} → {area}")
}

pub fn clip_missing(lang: Lang) -> String {
    pick!(
        lang,
        "nothing to cut yet: download data, or select a layer in Layers",
        "nie ma czego ciąć: pobierz dane albo wskaż warstwę w panelu Warstwy"
    )
    .to_string()
}

pub fn clip_same(lang: Lang, layer: &str) -> String {
    pick!(
        lang,
        format!("the boundary is the layer itself: \"{layer}\""),
        format!("granica to ta sama warstwa: „{layer}”")
    )
}

pub fn clip_no_boundary(lang: Lang) -> String {
    pick!(
        lang,
        "pick a boundary layer below",
        "wybierz warstwę graniczną poniżej"
    )
    .to_string()
}

pub fn removed(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("Removed: {name}"),
        format!("Usunięto: {name}")
    )
}

pub fn added(lang: Lang, name: &str) -> String {
    pick!(lang, format!("Added: {name}"), format!("Dodano: {name}"))
}

pub fn zoomed_to_poland(lang: Lang) -> &'static str {
    pick!(lang, " (zoomed to Poland)", " (przybliżono do Polski)")
}

pub fn detail_hint(lang: Lang) -> &'static str {
    pick!(
        lang,
        "GUGiK draws parcels and addresses only from about 1:5000, zoom the map in",
        "Działki i adresy GUGiK rysuje dopiero od skali ok. 1:5000, przybliż mapę"
    )
}

/// The status, and the exception line the body carried when it carried one.
pub fn http_status(lang: Lang, code: u16, url: &str, detail: &str) -> String {
    if detail.trim().is_empty() {
        return pick!(
            lang,
            format!("HTTP {code} from {url}"),
            format!("HTTP {code} z {url}")
        );
    }
    pick!(
        lang,
        format!("HTTP {code} from {url}: {detail}"),
        format!("HTTP {code} z {url}: {detail}")
    )
}

pub fn http_transport(url: &str, error: &str) -> String {
    format!("{url}: {error}")
}

pub fn http_read_failed(lang: Lang, error: &str) -> String {
    pick!(
        lang,
        format!("the answer could not be read: {error}"),
        format!("nie udało się odczytać odpowiedzi: {error}")
    )
}

pub fn http_too_big(lang: Lang, url: &str) -> String {
    pick!(
        lang,
        format!("the answer from {url} is too big"),
        format!("odpowiedź z {url} jest zbyt duża")
    )
}

pub fn http_not_utf8(lang: Lang, url: &str) -> String {
    pick!(
        lang,
        format!("the answer from {url} is not UTF-8 text"),
        format!("odpowiedź z {url} nie jest tekstem UTF-8")
    )
}

pub fn unknown_dataset(lang: Lang, label: &str) -> String {
    pick!(
        lang,
        format!("unknown dataset: '{label}'"),
        format!("nieznany zestaw danych: '{label}'")
    )
}

pub fn pick_area(lang: Lang) -> String {
    pick!(
        lang,
        "pick an area layer, for example \"Area to download\" drawn with Draw area",
        "wybierz warstwę obszaru, np. „Obszar pobierania” narysowaną przez Narysuj obszar"
    )
    .to_string()
}

pub fn pick_dataset(lang: Lang) -> String {
    pick!(
        lang,
        "pick a dataset from the list",
        "wybierz zestaw danych z listy"
    )
    .to_string()
}

pub fn area_not_passed(lang: Lang, area: &str) -> String {
    pick!(
        lang,
        format!("the area layer \"{area}\" was not passed in"),
        format!("nie przekazano warstwy obszaru „{area}”")
    )
}

pub fn area_no_geometry(lang: Lang, area: &str) -> String {
    pick!(
        lang,
        format!("the layer \"{area}\" has no geometry to take an extent from"),
        format!("warstwa „{area}” nie ma geometrii do wyznaczenia obszaru")
    )
}

pub fn area_too_large(
    lang: Lang,
    area: &str,
    width: f64,
    height: f64,
    name: &str,
    max: f64,
) -> String {
    let (width, height, max) = (num(lang, width, 1), num(lang, height, 1), num(lang, max, 2));
    pick!(
        lang,
        format!(
            "the layer \"{area}\" covers {width}° × {height}°, too much for \"{name}\" \
             (at most {max}°). Select the features you want on the map and run the tool again \
             (a selection narrows the area), or pick a smaller layer."
        ),
        format!(
            "warstwa „{area}” obejmuje {width}° × {height}°, za dużo dla „{name}” \
             (maks. {max}°). Zaznacz na mapie interesujące obiekty i uruchom narzędzie ponownie \
             (zaznaczenie zawęża obszar) albo wskaż mniejszą warstwę."
        )
    )
}

/// A layer the catalogue sets no span for is measured in features instead, before any fetch.
pub fn area_over_budget(lang: Lang, area: &str, needed: usize, budget: usize, name: &str) -> String {
    pick!(
        lang,
        format!(
            "the layer \"{area}\" needs {} from \"{name}\", more than one download takes ({}). \
             Select the features you want on the map and run the tool again (a selection narrows \
             the area), or pick a smaller layer.",
            features(lang, needed),
            features(lang, budget)
        ),
        format!(
            "warstwa „{area}” potrzebuje {} z zestawu „{name}”, a jedno pobranie bierze najwyżej \
             {}. Zaznacz na mapie interesujące obiekty i uruchom narzędzie ponownie (zaznaczenie \
             zawęża obszar) albo wskaż mniejszą warstwę.",
            features(lang, needed),
            features(lang, budget)
        )
    )
}

/// The progress line while the service counts the features the area needs.
pub fn counting(lang: Lang, name: &str, area: &str) -> String {
    pick!(
        lang,
        format!("{name}: counting the features in \"{area}\""),
        format!("{name}: zliczanie obiektów w „{area}”")
    )
}

pub fn no_features(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("no \"{name}\" features in the chosen area"),
        format!("brak obiektów „{name}” w wybranym obszarze")
    )
}

pub fn truncated(lang: Lang) -> &'static str {
    pick!(
        lang,
        "The download stopped at the limit, choose a smaller area.",
        "Pobieranie przerwane limitem, wybierz mniejszy obszar."
    )
}

/// The download could not filter by the drawn shape, so it widened to the shape's bounding box.
pub fn bbox_used(lang: Lang, problem: ShapeProblem) -> String {
    match problem {
        ShapeProblem::NoPolygon | ShapeProblem::SeveralPolygons => pick!(
            lang,
            "the area is not one polygon, so the bounding box was used",
            "obszar nie jest jednym poligonem, więc użyto prostokąta otaczającego"
        ),
        ShapeProblem::TooFewPoints => pick!(
            lang,
            "the area outline has fewer than four points, so the bounding box was used",
            "obrys obszaru ma mniej niż cztery punkty, więc użyto prostokąta otaczającego"
        ),
        ShapeProblem::TooLong => pick!(
            lang,
            "the area outline is too detailed for one request, so the bounding box was used",
            "obrys obszaru jest zbyt szczegółowy dla jednego żądania, więc użyto prostokąta otaczającego"
        ),
    }
    .to_string()
}

/// Requests that never answered, so the layer is short of the whole area.
pub fn shortfall(lang: Lang, failed: usize) -> String {
    // Polish wants the genitive after "brak", so 2 and 5 share one form.
    let words = pick!(
        lang,
        ["request", "requests", "requests"],
        ["żądania", "żądań", "żądań"]
    );
    let count = format!("{failed} {}", words[form(lang, failed)]);
    pick!(
        lang,
        format!("{count} failed, so the layer holds only what arrived"),
        format!("brak {count}, więc warstwa zawiera tylko to, co dotarło")
    )
}

pub fn span_invalid(lang: Lang) -> String {
    pick!(
        lang,
        "the area has no valid extent",
        "obszar nie ma poprawnego zasięgu"
    )
    .to_string()
}

/// The first progress line: how many cells the area was cut into.
pub fn split_areas(lang: Lang, name: &str, n: usize) -> String {
    match (lang, n) {
        (Lang::En, 1) => format!("{name}: one area"),
        (Lang::En, _) => format!("{name}: {} at once", areas(lang, n)),
        (Lang::Pl, 1) => format!("{name}: obszar"),
        (Lang::Pl, _) => format!("{name}: {}, równolegle", areas(lang, n)),
    }
}

pub fn area_progress(lang: Lang, name: &str, done: usize, total: usize, found: usize) -> String {
    pick!(
        lang,
        format!("{name}: area {done}/{total}, {}", features(lang, found)),
        format!("{name}: obszar {done}/{total}, {}", features(lang, found))
    )
}

pub fn area_progress_error(
    lang: Lang,
    name: &str,
    done: usize,
    total: usize,
    error: &str,
) -> String {
    pick!(
        lang,
        format!("{name}: area {done}/{total}, {error}"),
        format!("{name}: obszar {done}/{total}, {error}")
    )
}

pub fn uldk_layer(lang: Lang, parcel: &str) -> String {
    if parcel.is_empty() {
        pick!(lang, "Parcel (ULDK)", "Działka (ULDK)").to_string()
    } else {
        pick!(
            lang,
            format!("Parcel {parcel}"),
            format!("Działka {parcel}")
        )
    }
}

pub fn uldk_found(lang: Lang, parcel: &str) -> String {
    pick!(
        lang,
        format!("ULDK: parcel {parcel}"),
        format!("ULDK: działka {parcel}")
    )
}

pub fn uldk_bad_geometry(lang: Lang) -> String {
    pick!(
        lang,
        "ULDK returned a geometry that could not be read",
        "ULDK zwróciło geometrię, której nie da się odczytać"
    )
    .to_string()
}

pub fn uldk_empty(lang: Lang) -> String {
    pick!(lang, "ULDK returned nothing", "ULDK nic nie zwróciło").to_string()
}

pub fn uldk_no_parcel(lang: Lang) -> String {
    pick!(
        lang,
        "ULDK returned no parcel at this point",
        "ULDK nie znalazło działki w tym punkcie"
    )
    .to_string()
}

pub fn uldk_no_geometry(lang: Lang) -> String {
    pick!(
        lang,
        "ULDK returned no geometry",
        "ULDK nie zwróciło geometrii"
    )
    .to_string()
}

pub fn uldk_message(message: &str) -> String {
    format!("ULDK: {message}")
}

pub fn host_closed(lang: Lang) -> String {
    pick!(
        lang,
        "RustGIS closed the connection",
        "RustGIS zamknęło połączenie"
    )
    .to_string()
}

pub fn host_unreadable_reply(lang: Lang, error: &str) -> String {
    pick!(
        lang,
        format!("unreadable reply from RustGIS: {error}"),
        format!("nieczytelna odpowiedź RustGIS: {error}")
    )
}

pub fn host_reported_error(lang: Lang) -> &'static str {
    pick!(lang, "RustGIS reported an error", "RustGIS zgłosiło błąd")
}

pub fn host_no_answer(lang: Lang) -> String {
    pick!(
        lang,
        "RustGIS sent a reply without an answer",
        "RustGIS przysłało odpowiedź bez wyniku"
    )
    .to_string()
}

pub fn extent_missing(lang: Lang) -> String {
    pick!(
        lang,
        "map.extent returned no extent",
        "map.extent nie zwróciło zasięgu"
    )
    .to_string()
}

pub fn extent_shape(lang: Lang) -> String {
    pick!(
        lang,
        "map.extent returned an unexpected extent",
        "map.extent zwróciło zasięg w nieoczekiwanej postaci"
    )
    .to_string()
}

pub fn extent_non_numeric(lang: Lang) -> String {
    pick!(
        lang,
        "map.extent returned a non-numeric extent",
        "map.extent zwróciło zasięg z wartością nieliczbową"
    )
    .to_string()
}

pub fn view_no_centre(lang: Lang) -> String {
    pick!(
        lang,
        "host.view returned no centre",
        "host.view nie zwróciło środka"
    )
    .to_string()
}

pub fn no_longitude(lang: Lang) -> String {
    pick!(lang, "no longitude", "brak długości geograficznej").to_string()
}

pub fn no_latitude(lang: Lang) -> String {
    pick!(lang, "no latitude", "brak szerokości geograficznej").to_string()
}

pub fn gml_unreadable(lang: Lang, error: &str) -> String {
    pick!(
        lang,
        format!("GML could not be read: {error}"),
        format!("nie udało się odczytać GML: {error}")
    )
}

/// The service answered an OGC exception instead of features, so its own text is the answer.
pub fn service_exception(lang: Lang, detail: &str) -> String {
    pick!(
        lang,
        format!("the service answered with an exception: {detail}"),
        format!("usługa odpowiedziała wyjątkiem: {detail}")
    )
}

pub fn gml_unclosed(lang: Lang) -> String {
    pick!(
        lang,
        "the GML document ends before its last element closes",
        "dokument GML kończy się przed zamknięciem ostatniego elementu"
    )
    .to_string()
}

/// The view line: the box, then its scale.
pub fn view_text(lang: Lang, extent: [f64; 4], scale: f64) -> String {
    let [west, south, east, north] = extent;
    let box_line = format!("{west:.3}°–{east:.3}° E, {south:.3}°–{north:.3}° N");
    if scale <= 0.0 {
        return box_line;
    }
    pick!(
        lang,
        format!("{box_line}\nScale 1:{scale:.0}"),
        format!("{box_line}\nSkala 1:{scale:.0}")
    )
}

/// No extent came back, so the panel says so and hides the row instead of showing 0°.
pub fn view_missing(lang: Lang) -> String {
    pick!(
        lang,
        "The map did not report a view.",
        "Mapa nie podała widoku."
    )
    .to_string()
}

/// What the draw command says, and which gesture finishes it.
pub fn area_armed(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!(
            "Click the corners of the area, then double-click to close it — or press Escape to \
             stop. {name} is active and the polygon tool is on. The shape stays as drawn; a \
             download follows it, and the elevation model covers the rectangle around it."
        ),
        format!(
            "Klikaj narożniki obszaru, a na końcu kliknij dwukrotnie, aby go zamknąć — albo \
             naciśnij Escape, aby przerwać. Warstwa {name} jest aktywna, a narzędzie poligonu \
             włączone. Kształt zostaje taki, jak narysowany; pobieranie idzie za nim, a model \
             wysokości obejmuje prostokąt wokół niego."
        )
    )
}

/// The rectangle the area layer now holds, and how big it is.
pub fn area_from_view(lang: Lang, name: &str, size: (f64, f64)) -> String {
    let (width, height) = (num(lang, size.0, 1), num(lang, size.1, 1));
    pick!(
        lang,
        format!(
            "Rectangle from the view written to {name}: {width} × {height} km. It is the area a \
             download and the elevation model use."
        ),
        format!(
            "Prostokąt z widoku zapisany w warstwie {name}: {width} × {height} km. To obszar, \
             którego używa pobieranie i model wysokości."
        )
    )
}

/// The rectangle replaced a drawn shape, so the shape did not vanish without a word.
pub fn area_replaced(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("The rectangle replaced the shape drawn in {name}."),
        format!("Prostokąt zastąpił kształt narysowany w warstwie {name}.")
    )
}

/// The host made no area layer for the ring.
pub fn area_failed(lang: Lang) -> String {
    pick!(
        lang,
        "RustGIS did not make the area layer",
        "RustGIS nie utworzyło warstwy obszaru"
    )
    .to_string()
}

/// The area layer is gone, and the next command makes a new one.
pub fn area_cleared(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("Removed {name}. The next rectangle or drawn shape starts empty."),
        format!("Usunięto {name}. Następny prostokąt albo rysowany kształt zaczyna od zera.")
    )
}

/// Nothing to clear: no area layer on the map.
pub fn area_none(lang: Lang) -> String {
    pick!(
        lang,
        "There is no area layer to remove. Press Area from view or Draw area to make one.",
        "Nie ma warstwy obszaru do usunięcia. Naciśnij przycisk Obszar z widoku albo Narysuj obszar."
    )
    .to_string()
}

/// The model is a rectangle, and the area drawn was not one.
pub fn nmt_box_note(lang: Lang, name: &str, box_lon: [f64; 4]) -> String {
    let [west, south, east, north] = box_lon;
    pick!(
        lang,
        format!(
            "The model is a rectangle, so it covers {west:.4}°–{east:.4}° E, {south:.4}°–{north:.4}° N \
             rather than the shape drawn. {name} now shows that rectangle."
        ),
        format!(
            "Model jest prostokątem, więc obejmuje {west:.4}°–{east:.4}° E, {south:.4}°–{north:.4}° N, \
             a nie narysowany kształt. {name} pokazuje teraz ten prostokąt."
        )
    )
}

pub fn service_unknown(lang: Lang, wanted: &str, count: usize) -> String {
    pick!(
        lang,
        format!("No service '{wanted}' among the {count} services in the catalogue. Pick one from the WMS (view only) group, or pass its name in the service parameter."),
        format!("Nie ma usługi '{wanted}' wśród {count} usług w katalogu. Wskaż ją w grupie WMS (podgląd) albo podaj jej nazwę w parametrze service.")
    )
}

pub fn service_added(lang: Lang, name: &str, layer: &str) -> String {
    pick!(
        lang,
        format!("{name} on the map, layers {layer}."),
        format!("{name} na mapie, warstwy {layer}.")
    )
}

pub fn area_aborted(lang: Lang) -> String {
    pick!(
        lang,
        "Drawing cancelled, the tool is back to explore.",
        "Rysowanie anulowane, narzędzie wróciło do przeglądania."
    )
    .to_string()
}

pub fn area_no_view(lang: Lang) -> String {
    pick!(
        lang,
        "RustGIS reported no view to copy.",
        "RustGIS nie podało widoku do skopiowania."
    )
    .to_string()
}

pub fn area_missing(lang: Lang) -> String {
    pick!(
        lang,
        "Not on the map yet. Press Area from view, or draw one.",
        "Jeszcze nie ma na mapie. Naciśnij przycisk Obszar z widoku albo go narysuj."
    )
    .to_string()
}

pub fn area_present(lang: Lang, features_found: usize) -> String {
    pick!(
        lang,
        format!("On the map, {}.", features(lang, features_found)),
        format!("Jest na mapie, {}.", features(lang, features_found))
    )
}

pub fn cache_text(lang: Lang, count: usize, bytes: u64) -> String {
    format!(
        "{}, {} MB",
        files(lang, count),
        num(lang, bytes as f64 / 1e6, 1)
    )
}

/// What the download tools offer, and what the catalogue holds beyond them.
pub fn catalogue_text(lang: Lang, offered: usize, all: usize) -> String {
    let (offered, sheets) = (datasets(lang, offered), sheets(lang, all.saturating_sub(offered)));
    pick!(
        lang,
        format!("{offered} in the download tools, {sheets} beside them (a script names those by id)."),
        format!("{offered} w narzędziach pobierania, {sheets} obok (skrypt wskazuje je po identyfikatorze).")
    )
}

pub fn zoomed(lang: Lang) -> String {
    pick!(lang, "Zoomed to Poland", "Przybliżono do Polski").to_string()
}

/// The raster's name: the product and its grid, the same in both languages.
pub fn nmt_layer_name(surface: bool, datum: &str) -> String {
    let product = if surface { "NMPT" } else { "NMT" };
    format!("{product} 1 m {datum}")
}

pub fn nmt_added(
    lang: Lang,
    name: &str,
    datum: &str,
    size: (u32, u32),
    megabytes: f64,
    source: &str,
    colours: bool,
) -> String {
    let (width, height) = size;
    let megabytes = num(lang, megabytes, 1);
    let look = if colours {
        pick!(lang, "colours", "barwy")
    } else {
        pick!(lang, "shaded", "cieniowany")
    };
    pick!(
        lang,
        format!("{name} added from {source}: {datum}, {width} × {height} cells, {megabytes} MB, {look}."),
        format!("Dodano {name} z {source}: {datum}, {width} × {height} komórek, {megabytes} MB, {look}.")
    )
}

/// The GeoTIFF is left to the app, which fetches it off the frame: nothing is styled yet.
pub fn nmt_pending(lang: Lang, name: &str, source: &str) -> String {
    pick!(
        lang,
        format!(
            "{name} is on its way from {source}: RustGIS downloads the GeoTIFF in the background and shades it when it lands."
        ),
        format!(
            "{name} jest w drodze z warstwy {source}: RustGIS pobiera GeoTIFF w tle i cieniuje go po pobraniu."
        )
    )
}

/// The model is built from what the user pointed at, so the view plays no part.
pub fn nmt_needs_area(lang: Lang) -> String {
    pick!(
        lang,
        "The elevation model needs an area: select features on a layer, or draw a polygon and keep it active.",
        "Model wysokości potrzebuje obszaru: zaznacz obiekty na warstwie albo narysuj poligon i zostaw go aktywnego."
    )
    .to_string()
}

pub fn nmt_outside_poland(lang: Lang) -> String {
    pick!(
        lang,
        "The area box is outside Poland, where GUGiK has no data.",
        "Obszar jest poza Polską, gdzie GUGiK nie ma danych."
    )
    .to_string()
}

/// The download holds the window, so the area is measured against the measured speed first.
pub fn nmt_budget(lang: Lang, cells: f64, megabytes: f64, seconds: f64, budget: f64) -> String {
    let (cells, megabytes, seconds, budget) = (
        num(lang, cells, 0),
        num(lang, megabytes, 0),
        num(lang, seconds, 0),
        num(lang, budget, 0),
    );
    pick!(
        lang,
        format!(
            "The area is {cells} cells at 1 m: {megabytes} MB, about {seconds} s to download. One click may hold the window for {budget} s. Select a smaller area."
        ),
        format!(
            "Obszar ma {cells} komórek w rozdzielczości 1 m: {megabytes} MB, około {seconds} s pobierania. Jedno kliknięcie może zablokować okno na {budget} s. Zaznacz mniejszy obszar."
        )
    )
}

pub fn nmt_nothing_to_style(lang: Lang) -> String {
    pick!(
        lang,
        "No elevation model on the map yet. Add one first.",
        "Nie ma jeszcze modelu wysokości na mapie. Najpierw go dodaj."
    )
    .to_string()
}

pub fn nmt_styled(lang: Lang, layer: &str, colours: bool) -> String {
    if !colours {
        pick!(
            lang,
            format!("{layer}: shaded."),
            format!("{layer}: cieniowanie.")
        )
    } else {
        pick!(
            lang,
            format!("{layer}: heights in colours."),
            format!("{layer}: wysokości w barwach.")
        )
    }
}

pub fn file_failed(lang: Lang, path: &str, error: &str) -> String {
    pick!(
        lang,
        format!("{path} could not be written: {error}"),
        format!("Nie można zapisać {path}: {error}")
    )
}

pub fn download_empty(lang: Lang, url: &str) -> String {
    pick!(
        lang,
        format!("{url} sent no data."),
        format!("{url} nie przysłał danych.")
    )
}

pub fn uug_needs_address(lang: Lang) -> String {
    pick!(
        lang,
        "give an address: {\"adres\": \"Poznań, Głogowska 10\"}",
        "podaj adres: {\"adres\": \"Poznań, Głogowska 10\"}"
    )
    .to_string()
}

pub fn uug_unreadable(lang: Lang) -> String {
    pick!(
        lang,
        "the address service answered something that is not an address",
        "usługa adresowa odpowiedziała czymś, co nie jest adresem",
    )
    .to_string()
}

pub fn uug_no_address(lang: Lang, address: &str) -> String {
    pick!(
        lang,
        format!("no address found for \"{address}\""),
        format!("nie znaleziono adresu „{address}”"),
    )
}

pub fn uug_layer(lang: Lang, address: &str) -> String {
    let short: String = address.chars().take(40).collect();
    pick!(lang, format!("Address: {short}"), format!("Adres: {short}"))
}

pub fn uug_found(lang: Lang, address: &str, lon: f64, lat: f64) -> String {
    pick!(
        lang,
        format!("{address} at {lat:.6}, {lon:.6}"),
        format!("{address} o współrzędnych {lat:.6}, {lon:.6}"),
    )
}

/// More than one address matched the query; the tooltip names how many were left out.
pub fn uug_candidates(lang: Lang, count: usize) -> String {
    pick!(
        lang,
        format!("{count} addresses match, the closest one was added"),
        format!("pasuje {count} adresów, dodano najbliższy"),
    )
}

pub fn height_failed(lang: Lang, detail: &str) -> String {
    pick!(
        lang,
        format!("the terrain model answered \"{detail}\", not a height"),
        format!("model terenu odpowiedział „{detail}”, a nie wysokością"),
    )
}

/// The model covers Poland; elsewhere the service answers 0, which would read as sea level.
pub fn height_outside(lang: Lang) -> String {
    pick!(
        lang,
        "the terrain model covers Poland only, and the point is outside it",
        "model terenu pokrywa tylko Polskę, a punkt jest poza nią",
    )
    .to_string()
}

pub fn height_at(lang: Lang, metres: f64, x: f64, y: f64) -> String {
    pick!(
        lang,
        format!("{metres:.1} m above sea level at PL-1992 {x:.0}, {y:.0}"),
        format!("{metres:.1} m n.p.m. w PL-1992 {x:.0}, {y:.0}"),
    )
}

pub fn eziudp_needs_query(lang: Lang) -> String {
    pick!(
        lang,
        "narrow the register: {\"szukaj\": \"ortofoto\"}, {\"instytucja\": \"Geolog\"} or {\"teryt\": \"3064\"}. Without a filter it is 12 000 records and 9 MB",
        "zawęź ewidencję: {\"szukaj\": \"ortofoto\"}, {\"instytucja\": \"Geolog\"} albo {\"teryt\": \"3064\"}. Bez filtra to 12 000 rekordów i 9 MB",
    )
    .to_string()
}

/// The narrowed register matched nothing, so the empty answer says it rather than reading as a bug.
pub fn eziudp_nothing(lang: Lang, query: &str) -> String {
    pick!(
        lang,
        format!("the register has nothing matching \"{query}\""),
        format!("ewidencja nie ma nic dla „{query}”"),
    )
}

pub fn eziudp_unreadable(lang: Lang) -> String {
    pick!(
        lang,
        "the register answered something that is not JSON",
        "rejestr odpowiedział czymś, co nie jest JSON-em",
    )
    .to_string()
}

pub fn uldk_needs_id(lang: Lang) -> String {
    pick!(
        lang,
        "give the parcel as {\"id\": \"306401_1.0001.14/4\"}, or as \"teryt\" and \"numer\" (with \"obreb\", \"arkusz\")",
        "podaj działkę jako {\"id\": \"306401_1.0001.14/4\"} albo przez \"teryt\" i \"numer\" (z \"obreb\", \"arkusz\")",
    )
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polish_counts_follow_its_plural_rules() {
        assert_eq!(features(Lang::Pl, 1), "1 obiekt");
        assert_eq!(features(Lang::Pl, 3), "3 obiekty");
        assert_eq!(features(Lang::Pl, 12), "12 obiektów");
        assert_eq!(features(Lang::Pl, 22), "22 obiekty");
        assert_eq!(features(Lang::En, 1), "1 feature");
        assert_eq!(features(Lang::En, 3), "3 features");
    }

    #[test]
    fn reads_a_language_code() {
        assert_eq!(Lang::from_code("pl"), Lang::Pl);
        assert_eq!(Lang::from_code("pl-PL"), Lang::Pl);
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("de"), Lang::En);
        assert_eq!(Lang::from_code(""), Lang::En);
    }

    #[test]
    fn names_the_clip_pair_in_both_languages() {
        assert_eq!(
            clip_text("Parcels (EGiB)", "Area to download"),
            "Parcels (EGiB) → Area to download"
        );
        assert!(clip_missing(Lang::Pl).contains("pobierz dane"));
        assert!(clip_same(Lang::En, "Parcels").contains("Parcels"));
    }

    #[test]
    fn names_the_area_layer_in_both_languages() {
        assert_eq!(area_layer(Lang::En), "Area to download");
        assert_eq!(area_layer(Lang::Pl), "Obszar pobierania");
        assert_eq!(clipped_suffix(Lang::En), " (clipped)");
        assert_eq!(clipped_suffix(Lang::Pl), " (po przycięciu)");
        assert_eq!(
            count(Lang::Pl, "Działki (EGiB)", 5),
            "Działki (EGiB): 5 obiektów"
        );
        assert_eq!(
            count(Lang::En, "Parcels (EGiB)", 5),
            "Parcels (EGiB): 5 features"
        );
    }

    #[test]
    fn polish_writes_a_decimal_comma() {
        assert_eq!(num(Lang::Pl, 1.5, 1), "1,5");
        assert_eq!(num(Lang::En, 1.5, 1), "1.5");
        assert_eq!(num(Lang::Pl, 12.3, 1), "12,3");
        assert_eq!(num(Lang::Pl, 54_267.0, 0), "54267");
        assert!(area_from_view(Lang::Pl, "Obszar", (1.5, 2.0)).contains("1,5 × 2,0 km"));
        assert!(catalogue_text(Lang::Pl, 106, 268).contains("106 zbiorów"));
    }

    #[test]
    fn a_count_of_areas_keeps_the_number_in_polish() {
        assert_eq!(split_areas(Lang::Pl, "granice", 1), "granice: obszar");
        assert_eq!(
            split_areas(Lang::Pl, "granice", 4),
            "granice: 4 obszary, równolegle"
        );
        assert_eq!(split_areas(Lang::En, "gminy", 4), "gminy: 4 areas at once");
    }

    #[test]
    fn names_the_bbox_fallback_and_the_shortfall() {
        assert!(bbox_used(Lang::En, ShapeProblem::SeveralPolygons).contains("not one polygon"));
        assert!(bbox_used(Lang::Pl, ShapeProblem::TooLong).contains("prostokąta"));
        assert_eq!(shortfall(Lang::En, 1), "1 request failed, so the layer holds only what arrived");
        assert_eq!(shortfall(Lang::Pl, 5), "brak 5 żądań, więc warstwa zawiera tylko to, co dotarło");
        assert!(service_exception(Lang::Pl, "błąd serwera").contains("błąd serwera"));
        assert!(http_status(Lang::En, 400, "https://x", "no such type").contains("no such type"));
        assert_eq!(http_status(Lang::En, 400, "https://x", ""), "HTTP 400 from https://x");
    }
}
