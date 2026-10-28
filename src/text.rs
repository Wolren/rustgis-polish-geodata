//! Every sentence the plugin shows, in English and Polish.
//!
//! RustGIS puts the interface language's code on each request (`lang`), so a command or a tool
//! answers in that language. Two languages only: English is the base, Polish is the translation,
//! and any other code falls back to English.

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
    ($lang:expr, $en:expr, $pl:expr) => {
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
    let words = pick!(lang, ["feature", "features", "features"], ["obiekt", "obiekty", "obiektów"]);
    format!("{n} {}", words[form(lang, n)])
}

pub fn files(lang: Lang, n: usize) -> String {
    let words = pick!(lang, ["file", "files", "files"], ["plik", "pliki", "plików"]);
    format!("{n} {}", words[form(lang, n)])
}

pub fn areas(lang: Lang, n: usize) -> String {
    let words = pick!(lang, ["area", "areas", "areas"], ["obszar", "obszary", "obszarów"]);
    format!("{n} {}", words[form(lang, n)])
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
        format!("Dane PL nie ma polecenia '{id}'")
    )
}

pub fn cache_cleared(lang: Lang, count: usize, megabytes: f64) -> String {
    pick!(
        lang,
        format!("WFS cache: removed {} ({megabytes:.0} MB)", files(lang, count)),
        format!("Cache WFS: usunięto {} ({megabytes:.0} MB)", files(lang, count))
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
    pick!(lang, " (clipped)", " (przycięte)")
}

pub fn clipped(lang: Lang, layer: &str, area: &str, kept: usize) -> String {
    pick!(
        lang,
        format!("Clipped \"{layer}\" to \"{area}\": {}", features(lang, kept)),
        format!("Przycięto „{layer}” do „{area}”: {}", features(lang, kept))
    )
}

pub fn removed(lang: Lang, name: &str) -> String {
    pick!(lang, format!("Removed: {name}"), format!("Usunięto: {name}"))
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

pub fn http_status(lang: Lang, code: u16, url: &str) -> String {
    pick!(lang, format!("HTTP {code} from {url}"), format!("HTTP {code} z {url}"))
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

pub fn area_not_passed(lang: Lang, area: &str) -> String {
    pick!(
        lang,
        format!("the area layer \"{area}\" was not passed in"),
        format!("nie otrzymałem warstwy obszaru „{area}”")
    )
}

pub fn area_no_geometry(lang: Lang, area: &str) -> String {
    pick!(
        lang,
        format!("the layer \"{area}\" has no geometry to take an extent from"),
        format!("warstwa „{area}” nie ma geometrii do wyznaczenia obszaru")
    )
}

pub fn area_too_large(lang: Lang, area: &str, width: f64, height: f64, name: &str, max: f64) -> String {
    pick!(
        lang,
        format!(
            "the layer \"{area}\" covers {width:.1}° × {height:.1}°, too much for \"{name}\" \
             (at most {max:.2}°). Select the features you want on the map and run the tool again \
             (a selection narrows the area), or pick a smaller layer."
        ),
        format!(
            "warstwa „{area}” obejmuje {width:.1}° × {height:.1}°, za dużo dla „{name}” \
             (maks. {max:.2}°). Zaznacz na mapie interesujące obiekty i uruchom narzędzie ponownie \
             (zaznaczenie zawęża obszar) albo wskaż mniejszą warstwę."
        )
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

pub fn span_too_large(lang: Lang, name: &str, width: f64, height: f64, max: f64) -> String {
    pick!(
        lang,
        format!(
            "the area is too big for \"{name}\": {width:.2}° × {height:.2}°, the maximum is {max:.2}°, \
             zoom in or choose a smaller area."
        ),
        format!(
            "obszar jest zbyt duży dla „{name}”: {width:.2}° × {height:.2}°, a maksimum to {max:.2}°, \
             przybliż mapę albo wybierz mniejszy obszar."
        )
    )
}

pub fn span_invalid(lang: Lang) -> String {
    pick!(lang, "the area has no valid extent", "obszar nie ma poprawnego zasięgu").to_string()
}

/// The first progress line: how many cells the area was cut into.
pub fn split_areas(lang: Lang, name: &str, n: usize) -> String {
    match (lang, n) {
        (Lang::En, 1) => format!("{name}: one area"),
        (Lang::En, _) => format!("{name}: {} at once", areas(lang, n)),
        (Lang::Pl, 1) => format!("{name}: obszar"),
        (Lang::Pl, _) => format!("{name}: obszary równolegle"),
    }
}

pub fn area_progress(lang: Lang, name: &str, done: usize, total: usize, found: usize) -> String {
    pick!(
        lang,
        format!("{name}: area {done}/{total}, {}", features(lang, found)),
        format!("{name}: obszar {done}/{total}, {}", features(lang, found))
    )
}

pub fn area_progress_error(lang: Lang, name: &str, done: usize, total: usize, error: &str) -> String {
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
        pick!(lang, format!("Parcel {parcel}"), format!("Działka {parcel}"))
    }
}

pub fn uldk_found(lang: Lang, parcel: &str) -> String {
    pick!(lang, format!("ULDK: parcel {parcel}"), format!("ULDK: działka {parcel}"))
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
    pick!(lang, "ULDK returned no geometry", "ULDK nie zwróciło geometrii").to_string()
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
    pick!(lang, "host.view returned no centre", "host.view nie zwróciło środka").to_string()
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

/// What the draw command says, and which gesture finishes it.
pub fn area_armed(lang: Lang, mode: &str, name: &str) -> String {
    let (en, pl) = match mode {
        "linia" => (
            format!("Click two opposite corners, then double-click. {name} is active and the line tool is on. The model covers the rectangle between them."),
            format!("Kliknij dwa przeciwległe narożniki, a na końcu kliknij dwukrotnie. {name} jest aktywna, a narzędzie linii włączone. Model obejmie prostokąt między nimi."),
        ),
        "poligon" => (
            format!("Click the corners, then double-click to close. {name} is active and the polygon tool is on. The shape stays as drawn."),
            format!("Klikaj narożniki, a na końcu kliknij dwukrotnie. {name} jest aktywna, a narzędzie poligonu włączone. Kształt zostaje taki, jak narysowany."),
        ),
        _ => (
            format!("Click the corners, then double-click to close. {name} is active and the polygon tool is on. The model covers the rectangle around them."),
            format!("Klikaj narożniki, a na końcu kliknij dwukrotnie. {name} jest aktywna, a narzędzie poligonu włączone. Model obejmie prostokąt wokół nich."),
        ),
    };
    pick!(lang, en, pl)
}

pub fn area_from_view(lang: Lang, name: &str, size: (f64, f64)) -> String {
    pick!(
        lang,
        format!("View saved to {name}: {:.1} by {:.1} km. Press a model button to build from it.", size.0, size.1),
        format!("Widok zapisany w {name}: {:.1} na {:.1} km. Naciśnij przycisk modelu, aby z niego zbudować.", size.0, size.1)
    )
}

pub fn service_unknown(lang: Lang, wanted: &str, count: usize) -> String {
    pick!(
        lang,
        format!("No service '{wanted}' among the {count} in the catalogue. Pick one on the panel slider, then press Add or remove."),
        format!("Nie ma usługi '{wanted}' wśród {count} w katalogu. Wybierz ją suwakiem w panelu i naciśnij Dodaj lub usuń.")
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
        "Not on the map yet. Press Draw area.",
        "Jeszcze nie ma na mapie. Naciśnij Narysuj obszar."
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
        "{}, {:.1} MB",
        files(lang, count),
        bytes as f64 / 1e6
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
    let look = if colours {
        pick!(lang, "colours", "barwy")
    } else {
        pick!(lang, "shaded", "cieniowany")
    };
    pick!(
        lang,
        format!("{name} added from {source}: {datum}, {width} by {height} cells, {megabytes:.1} MB, {look}."),
        format!("Dodano {name} z {source}: {datum}, {width} na {height} komorek, {megabytes:.1} MB, {look}.")
    )
}

/// The model is built from what the user pointed at, so the view plays no part.
pub fn data_needs_area(lang: Lang) -> String {
    pick!(
        lang,
        "Draw the area first: the download covers the area layer, and a whole country will not fit."
            .to_string(),
        "Najpierw narysuj obszar: pobieranie obejmuje warstwę obszaru, a cały kraj się nie zmieści."
            .to_string()
    )
}

pub fn data_added(lang: Lang, name: &str, n: usize) -> String {
    pick!(
        lang,
        format!("{name}: {n} features on the map."),
        format!("{name}: {n} obiektów na mapie.")
    )
}

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
    pick!(
        lang,
        format!(
            "The area is {cells:.0} cells at 1 m: {megabytes:.0} MB, about {seconds:.0} s to download. One click may hold the window for {budget:.0} s. Select a smaller area."
        ),
        format!(
            "Obszar ma {cells:.0} komorek w 1 m: {megabytes:.0} MB, około {seconds:.0} s pobierania. Jedno kliknięcie może zająć okno na {budget:.0} s. Zaznacz mniejszy obszar."
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
    fn names_the_area_layer_in_both_languages() {
        assert_eq!(area_layer(Lang::En), "Area to download");
        assert_eq!(area_layer(Lang::Pl), "Obszar pobierania");
        assert_eq!(clipped_suffix(Lang::En), " (clipped)");
        assert_eq!(count(Lang::Pl, "Działki (EGiB)", 5), "Działki (EGiB): 5 obiektów");
        assert_eq!(count(Lang::En, "Parcels (EGiB)", 5), "Parcels (EGiB): 5 features");
    }
}
