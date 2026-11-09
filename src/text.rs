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

/// A size in megabytes as a reader expects it: gigabytes above a thousand.
pub fn size(lang: Lang, megabytes: f64) -> String {
    if megabytes >= 1000.0 {
        format!("{} GB", num(lang, megabytes / 1000.0, 1))
    } else {
        format!("{} MB", num(lang, megabytes, 0))
    }
}

/// What a sheet run is about to fetch, before the first byte moves.
pub fn sheets_estimate(lang: Lang, files: usize, megabytes: f64) -> String {
    pick!(
        lang,
        format!(
            "{} to fetch, at least {}",
            self::files(lang, files),
            size(lang, megabytes)
        ),
        format!(
            "{} do pobrania, co najmniej {}",
            self::files(lang, files),
            size(lang, megabytes)
        )
    )
}

/// The run is larger than the form's threshold, so it needs the box that says it is meant.
pub fn sheets_unconfirmed(lang: Lang, files: usize, megabytes: f64, limit: f64) -> String {
    pick!(
        lang,
        format!(
            "{} and at least {}, above the {} mark. Tick \"Download anyway\" and run again.",
            self::files(lang, files),
            size(lang, megabytes),
            size(lang, limit)
        ),
        format!(
            "{} i co najmniej {}, powyżej progu {}. Zaznacz \"Pobierz mimo rozmiaru\" i uruchom ponownie.",
            self::files(lang, files),
            size(lang, megabytes),
            size(lang, limit)
        )
    )
}

/// A `d:` row points at the tool instead of starting it: a command cannot run one in the background.
pub fn map_download(lang: Lang, dataset: &str) -> String {
    pick!(
        lang,
        format!(
            "{dataset} is a dataset to download: pick it in the Downloads group, whose tools run in the background with progress and Cancel"
        ),
        format!(
            "„{dataset}” to zbiór do pobrania: wybierz go w grupie Pobieranie, której narzędzia działają w tle, z postępem i Anuluj"
        )
    )
}

/// The sheet downloader needs a folder to write the files into.
pub fn folder_missing(lang: Lang) -> String {
    pick!(
        lang,
        "give the folder to download into, for example {\"folder\": \"D:/dane\"}",
        "podaj folder do pobrania, np. {\"folder\": \"D:/dane\"}"
    )
    .to_string()
}

/// The folder could not be made, so nothing is downloaded into it.
pub fn folder_failed(lang: Lang, folder: &str, error: &str) -> String {
    pick!(
        lang,
        format!("the folder {folder} cannot be made: {error}"),
        format!("nie można utworzyć folderu {folder}: {error}")
    )
}

/// The layer the tool names as its sheet index is not in the request.
pub fn sheet_layer_missing(lang: Lang, layer: &str) -> String {
    if layer.trim().is_empty() {
        return pick!(
            lang,
            "the sheet index was not passed in",
            "nie przekazano skorowidza"
        )
        .to_string();
    }
    pick!(
        lang,
        format!("the sheet index \"{layer}\" was not passed in"),
        format!("nie przekazano skorowidza „{layer}”")
    )
}

/// The index holds no address at all, so a run would fetch nothing.
pub fn nothing_to_download(lang: Lang, layer: &str, count: usize) -> String {
    pick!(
        lang,
        format!(
            "\"{layer}\" lists no file to download: 0 addresses, the layer has no url_do_pobrania or url ({})",
            features(lang, count)
        ),
        format!(
            "„{layer}” nie wskazuje plików do pobrania: 0 adresów, w warstwie nie ma url_do_pobrania ani url ({})",
            features(lang, count)
        )
    )
}

/// The list is longer than one run takes, and a selection is how the user splits it.
pub fn too_many_files(lang: Lang, count: usize, limit: usize) -> String {
    pick!(
        lang,
        format!(
            "the list holds {}, more than one run takes ({}): select a part of the index on the map and run the tool again",
            files(lang, count),
            files(lang, limit)
        ),
        format!(
            "lista ma {}, a jedno pobranie bierze najwyżej {}: zaznacz na mapie część skorowidza i uruchom narzędzie ponownie",
            files(lang, count),
            files(lang, limit)
        )
    )
}

/// The features that carried no address, counted beside the files that were fetched.
pub fn sheets_shortfall(lang: Lang, count: usize) -> String {
    pick!(
        lang,
        format!("left out {} in the index with no file address", features(lang, count)),
        format!("pominięto {} w skorowidzu bez adresu pliku", features(lang, count))
    )
}

/// What the run did: files fetched, their size, the kept and the failed ones, the report.
pub fn sheets_summary(
    lang: Lang,
    written: usize,
    megabytes: f64,
    skipped: usize,
    failed: usize,
    report: &str,
) -> String {
    let size = num(lang, megabytes, 1);
    let kept_words = pick!(
        lang,
        ["skipped", "skipped", "skipped"],
        ["pominięty", "pominięte", "pominiętych"]
    );
    let failed_words = pick!(
        lang,
        ["failed", "failed", "failed"],
        ["błąd", "błędy", "błędów"]
    );
    let kept = format!("{skipped} {}", kept_words[form(lang, skipped)]);
    let errors = format!("{failed} {}", failed_words[form(lang, failed)]);
    pick!(
        lang,
        format!(
            "Downloaded {} ({size} MB), {kept}, {errors}. Report: {report}",
            files(lang, written)
        ),
        format!(
            "Pobrano {} ({size} MB), {kept}, {errors}. Raport: {report}",
            files(lang, written)
        )
    )
}

/// The per-file progress line: its place in the list, then its name.
pub fn sheet_progress(lang: Lang, done: usize, total: usize, name: &str) -> String {
    pick!(
        lang,
        format!("{done}/{total}: {name}"),
        format!("{done}/{total}: {name}")
    )
}

/// The report could not be written, though the files may have arrived.
pub fn report_failed(lang: Lang, path: &str, error: &str) -> String {
    pick!(
        lang,
        format!("the report {path} could not be written: {error}"),
        format!("nie można zapisać raportu {path}: {error}")
    )
}

/// "Layer: 12 features".
/// A run's count with what the answers took so far, for a run that may grow past the limit.
pub fn count_size(lang: Lang, name: &str, n: usize, bytes: u64) -> String {
    format!("{name}: {}, {}", features(lang, n), size(lang, bytes as f64 / 1e6))
}

/// A download reached the size a run takes without asking.
pub fn download_too_big(lang: Lang, name: &str, megabytes: f64, limit: f64) -> String {
    pick!(
        lang,
        format!(
            "{name}: {} so far, more than the {} a run takes without asking. Tick \u{201e}Download even \
             if large\u{201d} in the tool form to fetch it.",
            size(lang, megabytes),
            size(lang, limit)
        ),
        format!(
            "{name}: już {}, więcej niż {}, ile pobranie bierze bez pytania. Zaznacz \u{201e}Pobierz \
             mimo rozmiaru\u{201d} w formularzu narzędzia, żeby je pobrać.",
            size(lang, megabytes),
            size(lang, limit)
        )
    )
}

/// The panel stored the two download settings.
pub fn settings_saved(lang: Lang, limit_mb: i64, confirm: bool) -> String {
    let asked = pick!(
        lang,
        if confirm { "downloads over the limit run without asking" } else { "downloads over the limit ask first" },
        if confirm { "pobrania powyżej limitu idą bez pytania" } else { "pobrania powyżej limitu pytają najpierw" }
    );
    pick!(
        lang,
        format!("settings saved: limit {limit_mb} MB, {asked}"),
        format!("ustawienia zapisane: limit {limit_mb} MB, {asked}")
    )
}

/// A confirmed run stopped at the hard size cap.
pub fn download_stopped(lang: Lang, name: &str, megabytes: f64) -> String {
    pick!(
        lang,
        format!("{name}: stopped at {}", size(lang, megabytes)),
        format!("{name}: przerwano na {}", size(lang, megabytes))
    )
}

/// The layer the download takes as its area, named in the interface language.
pub fn area_layer(lang: Lang) -> &'static str {
    pick!(lang, "Area to download", "Obszar pobierania")
}

/// Both names, so a layer made in either language is still found.
pub const AREA_LAYERS: [&str; 2] = ["Area to download", "Obszar pobierania"];

/// The layer a rectangle is dragged into, before a command takes it as the area.
pub fn draw_layer(lang: Lang) -> &'static str {
    pick!(lang, "Area drawing", "Rysunek obszaru")
}

/// Both names, so a drawing layer made in either language is found.
pub const DRAW_LAYERS: [&str; 2] = ["Area drawing", "Rysunek obszaru"];

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

/// What the rectangle tool says: the drag, and the button that finishes the job.
pub fn area_rect_armed(lang: Lang) -> String {
    pick!(
        lang,
        "Drag a rectangle on the map, then press Rectangle to area.".to_string(),
        "Przeciągnij prostokąt na mapie, a potem naciśnij Prostokąt do obszaru.".to_string()
    )
}

/// The take command found no rectangle to take.
pub fn area_nothing_drawn(lang: Lang) -> String {
    pick!(
        lang,
        "No rectangle was drawn, so the area did not change.".to_string(),
        "Nie narysowano prostokąta, więc obszar się nie zmienił.".to_string()
    )
}

/// The drawn rectangle the area layer now holds, and how big it is.
pub fn area_from_rectangle(lang: Lang, name: &str, size: (f64, f64)) -> String {
    let (width, height) = (num(lang, size.0, 1), num(lang, size.1, 1));
    pick!(
        lang,
        format!(
            "The drawn rectangle is the area in {name}: {width} × {height} km. It is the area a \
             download and the elevation model use."
        ),
        format!(
            "Narysowany prostokąt jest obszarem w warstwie {name}: {width} × {height} km. To \
             obszar, którego używa pobieranie i model wysokości."
        )
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

/// The two roots of the panel's map list: every map first, then every download.
pub fn map_roots(lang: Lang) -> [&'static str; 3] {
    pick!(
        lang,
        ["Basemaps", "Downloads", "EZiUDP register"],
        ["Podkłady", "Pobieranie", "Rejestr EZiUDP"]
    )
}

/// Nothing chosen in the panel's map list, so the press has no row to act on.
pub fn map_missing(lang: Lang) -> String {
    pick!(
        lang,
        "pick a row in the panel list first: a map to show, or a dataset to download",
        "najpierw wskaż wiersz na liście w panelu: mapę do pokazania albo zbiór do pobrania"
    )
    .to_string()
}

/// The panel's list holds no such row: the value names a map or a dataset this build has not.
pub fn map_unknown(lang: Lang, row: &str) -> String {
    pick!(
        lang,
        format!("the panel list has no map or dataset for \"{row}\""),
        format!("lista w panelu nie ma mapy ani zbioru dla „{row}”")
    )
}

/// A register branch whose reporter the register does not name.
pub fn register_other(lang: Lang) -> &'static str {
    pick!(lang, "Other", "Inne")
}

/// The offline register has no service under this id.
pub fn register_unknown(lang: Lang, id: &str) -> String {
    pick!(
        lang,
        format!("the EZiUDP register in this plugin has no service \"{id}\""),
        format!("rejestr EZiUDP w tej wtyczce nie ma usługi „{id}”")
    )
}

/// A register entry that is not a WMS, so it cannot become an overlay.
pub fn register_not_a_map(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("\"{name}\" is a WFS in the register; download its types through a download tool or QGIS"),
        format!("„{name}” to w rejestrze WFS; pobierz jego typy narzędziem pobierania albo w QGIS")
    )
}

/// The update tool asked the register for its rows.
pub fn register_reading(lang: Lang) -> String {
    pick!(
        lang,
        "EZiUDP: reading the register".to_string(),
        "EZiUDP: czytam rejestr".to_string()
    )
}

/// How many rows the register answered with.
pub fn register_rows(lang: Lang, rows: usize) -> String {
    pick!(
        lang,
        format!("EZiUDP: {rows} rows, grouping the addresses"),
        format!("EZiUDP: {rows} wierszy, grupuję adresy")
    )
}

/// How many addresses the tool is about to prove.
pub fn register_checking(lang: Lang, total: usize) -> String {
    pick!(
        lang,
        format!("EZiUDP: checking {total} new addresses"),
        format!("EZiUDP: sprawdzam {total} nowych adresów")
    )
}

/// One address checked, of the run.
pub fn register_progress(lang: Lang, done: usize, total: usize) -> String {
    pick!(
        lang,
        format!("EZiUDP: {done} of {total}"),
        format!("EZiUDP: {done} z {total}")
    )
}

/// The register was rewritten.
pub fn register_updated(lang: Lang, kept: usize, answered: usize, missed: usize) -> String {
    pick!(
        lang,
        format!("register updated: {kept} services, {answered} new answered, {missed} did not"),
        format!("rejestr zapisany: {kept} usług, {answered} nowych odpowiedziało, {missed} nie")
    )
}

/// How many addresses the run's budget left for the next run.
pub fn register_left(lang: Lang, left: usize) -> String {
    pick!(
        lang,
        format!("{left} addresses left for the next run: the register changed, the file did not"),
        format!("{left} adresów zostało na następny przebieg: rejestr się zmienił, plik nie")
    )
}

/// A script named no usable box, so the area command says what it wants.
pub fn area_needs_box(lang: Lang) -> String {
    pick!(
        lang,
        "give a box: {\"bbox\": [west, south, east, north]} in lon and lat, or a GeoJSON shape in \"geojson\"",
        "podaj prostokąt: {\"bbox\": [zachód, południe, wschód, północ]} w lon i lat, albo kształt GeoJSON w \"geojson\""
    )
    .to_string()
}

/// The area came from a box a script named.
pub fn area_from_box(lang: Lang, name: &str, size: (f64, f64)) -> String {
    let (width, height) = size;
    pick!(
        lang,
        format!("area {name} set from the box: {width:.1} × {height:.1} km"),
        format!("obszar {name} ustawiony z prostokąta: {width:.1} × {height:.1} km")
    )
}

/// The view sits where ULDK names no county.
pub fn register_off_county(lang: Lang) -> String {
    pick!(
        lang,
        "ULDK named no county under the view: point the map inside Poland",
        "ULDK nie podało powiatu pod widokiem: ustaw mapę w Polsce"
    )
    .to_string()
}

/// How many services the county under the view publishes.
pub fn register_county(lang: Lang, county: &str, teryt: &str, kind: &str, count: usize) -> String {
    pick!(
        lang,
        format!("{count} {kind} services for {county} (TERYT {teryt})"),
        format!("{count} usług {kind} dla {county} (TERYT {teryt})")
    )
}

/// A folder row was chosen instead of a map or a dataset inside it.
pub fn map_folder(lang: Lang, label: &str) -> String {
    pick!(
        lang,
        format!("pick a map or a dataset inside \"{label}\", not the folder itself"),
        format!("wybierz mapę albo zbiór wewnątrz „{label}”, nie sam folder")
    )
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
    look: &str,
    terrain: bool,
    heights: Option<(f64, f64)>,
) -> String {
    let (width, height) = size;
    let megabytes = num(lang, megabytes, 1);
    let look = match (lang, look) {
        (Lang::Pl, "shading") => "cieniowany",
        (Lang::Pl, "classes") => "w pasmach",
        (Lang::Pl, _) => "w barwach wysokości",
        (Lang::En, "shading") => "shaded",
        (Lang::En, "classes") => "in bands",
        (Lang::En, _) => "in height colours",
    };
    let terrain = if terrain {
        pick!(lang, ", set as the 3D terrain", ", ustawiony jako teren 3D")
    } else {
        ""
    };
    let heights = match heights {
        Some((min, max)) => pick!(
            lang,
            format!(", heights {min:.1} to {max:.1} m"),
            format!(", wysokości {min:.1} do {max:.1} m")
        ),
        None => String::new(),
    };
    pick!(
        lang,
        format!("{name} added from {source}: {datum}, {width} × {height} cells, {megabytes} MB, {look}{heights}{terrain}."),
        format!("Dodano {name} z {source}: {datum}, {width} × {height} komórek, {megabytes} MB, {look}{heights}{terrain}.")
    )
}

/// The parcel CSV run is partway through the file.
pub fn uldk_progress(lang: Lang, done: usize, total: usize) -> String {
    pick!(
        lang,
        format!("checking parcels: {done} of {total}"),
        format!("sprawdzanie działek: {done} z {total}")
    )
}

/// ULDK found none of the identifiers in the CSV.
pub fn uldk_none_found(lang: Lang, total: usize) -> String {
    pick!(
        lang,
        format!("ULDK found none of the {total} parcels"),
        format!("ULDK nie znalazło żadnej z {total} działek")
    )
}

/// The layer the parcel CSV builds.
pub fn uldk_csv_layer(lang: Lang) -> String {
    pick!(lang, "Parcels from the CSV", "Działki z CSV").to_string()
}

/// How the parcel CSV run went.
pub fn uldk_csv_done(lang: Lang, found: usize, missing: usize) -> String {
    pick!(
        lang,
        format!("{found} parcels found, {missing} not found"),
        format!("znaleziono {found} działek, nie znaleziono {missing}")
    )
}

/// The temporary layer a selected boundary is copied into.
pub fn clip_boundary_layer(lang: Lang) -> String {
    pick!(lang, "Clip boundary", "Granica cięcia").to_string()
}

/// The service counts no feature in the area, so no page is worth a request.
pub fn area_empty(lang: Lang, layer: &str, area: &str) -> String {
    pick!(
        lang,
        format!("{layer} has no feature in {area}: the run stops before a page"),
        format!("{layer} nie ma żadnego obiektu w {area}: przebieg kończy się przed stroną")
    )
}

/// The panel's tree has nothing chosen yet, so no unit sets the area.
pub fn unit_not_chosen(lang: Lang) -> String {
    pick!(
        lang,
        "pick a voivodeship, a powiat or a gmina in the tree first".to_string(),
        "wybierz najpierw województwo, powiat albo gminę w drzewie".to_string()
    )
}

/// The one-step download needs the dataset label it should fetch.
pub fn typed_needs_dataset(lang: Lang) -> String {
    pick!(
        lang,
        "name the dataset: {\"dane\": \"Punkty adresowe\", \"unit\": \"1464011\"}".to_string(),
        "podaj zbiór: {\"dane\": \"Punkty adresowe\", \"unit\": \"1464011\"}".to_string()
    )
}

/// A unit has no feature of the dataset within its boundary.
pub fn typed_empty(lang: Lang, dataset: &str, unit: &str) -> String {
    pick!(
        lang,
        format!("{dataset} has no feature in {unit}"),
        format!("{dataset} nie ma obiektu w {unit}")
    )
}

/// The download reached the run's cap, so the layer holds only the first features.
pub fn typed_capped(lang: Lang, features: usize) -> String {
    pick!(
        lang,
        format!("{features} features took the whole budget: pick a smaller unit for the rest"),
        format!("{features} obiektów zajęło cały budżet: wybierz mniejszą jednostkę dla reszty")
    )
}

/// The layer a typed download lands in: the type, then the unit it covers.
pub fn typed_layer(lang: Lang, dataset: &str, unit: &str, level: &str) -> String {
    let level = match (lang, level) {
        (Lang::Pl, "wojewodztwo") => "województwo",
        (Lang::Pl, "powiat") => "powiat",
        (Lang::Pl, "gmina") => "gmina",
        (_, other) => other,
    };
    let _ = level;
    format!("{dataset} - {unit}")
}

/// The clip refused, so the layer holds the box result and says so.
pub fn typed_clip_failed(lang: Lang, error: &str) -> String {
    pick!(
        lang,
        format!("the clip did not run ({error}): the layer holds the box result"),
        format!("cięcie nie ruszyło ({error}): warstwa ma wynik z prostokąta")
    )
}

/// The panel's checkbox is off, so the layer holds the box result.
pub fn typed_uncut(lang: Lang) -> String {
    pick!(
        lang,
        "cut to the unit is off in the panel: the layer holds every feature in its box".to_string(),
        "cięcie do jednostki jest wyłączone w panelu: warstwa ma wszystkie obiekty z prostokąta".to_string()
    )
}

/// The panel's checkbox label, the setting that decides whether a typed download is cut.
pub fn clip_unit_label(lang: Lang) -> String {
    pick!(
        lang,
        "Clip the download to the unit boundary".to_string(),
        "Przytnij pobranie do granicy jednostki".to_string()
    )
}

/// Features the clip left as lines along the boundary went away.
pub fn typed_shavings(lang: Lang, dropped: u64) -> String {
    pick!(
        lang,
        format!("{dropped} features only touched the boundary and had no area: dropped"),
        format!("{dropped} obiektów tylko dotykało granicy i nie miało powierzchni: usunięte")
    )
}

/// The typed download is on the map, already cut to the unit.
pub fn typed_added(lang: Lang, layer: &str, features: u64, unit: &str, level: &str) -> String {
    let level = match (lang, level) {
        (Lang::Pl, "wojewodztwo") => "województwo",
        (Lang::Pl, "powiat") => "powiat",
        (Lang::Pl, "gmina") => "gmina",
        (_, other) => other,
    };
    pick!(
        lang,
        format!("{layer}: {features} features cut to {level} {unit}"),
        format!("{layer}: {features} obiektów przycięte do {level} {unit}")
    )
}

/// The service refused the boundary as a filter, so the run used the area's box.
pub fn filter_fell_back(lang: Lang) -> String {
    pick!(
        lang,
        "the service refused the boundary as a filter, so the run used its box: clip the layer for the exact set".to_string(),
        "usługa odrzuciła granicę jako filtr, więc przebieg użył prostokąta: przytnij warstwę dla dokładnego zbioru".to_string()
    )
}

/// A code the tree does not hold came in, so the area stays as it was.
pub fn unit_unknown(lang: Lang, code: &str) -> String {
    pick!(
        lang,
        format!("the tree holds no unit with the code {code}"),
        format!("drzewo nie ma jednostki o kodzie {code}")
    )
}

/// The unit chosen in the tree, before PRG answers with its boundary.
pub fn unit_chosen(lang: Lang, name: &str, level: &str) -> String {
    let level = match (lang, level) {
        (Lang::Pl, "wojewodztwo") => "województwo",
        (Lang::Pl, "powiat") => "powiat",
        (Lang::Pl, "gmina") => "gmina",
        (_, other) => other,
    };
    pick!(
        lang,
        format!("{level} {name}: reading its boundary from PRG"),
        format!("{level} {name}: czytam granicę z PRG")
    )
}

/// A named area layer needs a name, since that is what a batch run points at.
pub fn area_needs_name(lang: Lang) -> String {
    pick!(
        lang,
        "give the layer a name: {\"nazwa\": \"Gmina A\", \"bbox\": [w, s, e, n]}".to_string(),
        "podaj nazwę warstwy: {\"nazwa\": \"Gmina A\", \"bbox\": [w, s, e, n]}".to_string()
    )
}

/// A named area layer is on the map, ready for a download.
pub fn area_added(lang: Lang, name: &str, size: (f64, f64)) -> String {
    pick!(
        lang,
        format!("{}: area {:.0} by {:.0} km, ready for a download or a batch", name, size.0, size.1),
        format!("{}: obszar {:.0} na {:.0} km, gotowy do pobrania lub serii", name, size.0, size.1)
    )
}

/// The 3D scene no longer stands on a model.
pub fn nmt_terrain_off(lang: Lang) -> String {
    pick!(
        lang,
        "3D terrain off: the map is flat again.",
        "Teren 3D wyłączony: mapa jest znowu płaska."
    )
    .to_string()
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

/// A big elevation grid waits for a second press, which the first press explains.
pub fn nmt_confirm_box(lang: Lang, cells: f64, megabytes: f64, seconds: f64, wait: u64) -> String {
    let (cells, megabytes, seconds) = (
        num(lang, cells, 0),
        num(lang, megabytes, 0),
        num(lang, seconds, 0),
    );
    pick!(
        lang,
        format!(
            "The area is {cells} cells at 1 m: {megabytes} MB, about {seconds} s, and the window waits for it. Press the same button again within {wait} s to download it."
        ),
        format!(
            "Obszar ma {cells} komórek w rozdzielczości 1 m: {megabytes} MB, około {seconds} s, a okno czeka na pobranie. Naciśnij ten sam przycisk ponownie w ciągu {wait} s, żeby pobrać."
        )
    )
}

/// A `warstwa` value names nothing in the service, so the message counts what is there.
pub fn wody_no_layer(lang: Lang, service: &str, count: usize) -> String {
    pick!(
        lang,
        format!(
            "{service} has no layer like that: it publishes {count}; run wody-warstwy with \"usluga\": \"{service}\" to list them"
        ),
        format!(
            "{service} nie ma takiej warstwy: publikuje {count}; uruchom wody-warstwy z \"usluga\": \"{service}\", żeby je wypisać"
        )
    )
}

/// The obręb listing got no unit code.
pub fn prg_needs_teryt(lang: Lang) -> String {
    pick!(
        lang,
        "give the unit: {\"teryt\": \"306401_1\"}",
        "podaj jednostkę: {\"teryt\": \"306401_1\"}"
    )
    .to_string()
}

/// The unit command got no name, so it says which levels it knows.
pub fn prg_needs_name(lang: Lang) -> String {
    pick!(
        lang,
        "name a unit: {\"nazwa\": \"Poznań\", \"poziom\": \"gmina\"}",
        "podaj jednostkę: {\"nazwa\": \"Poznań\", \"poziom\": \"gmina\"}"
    )
    .to_string()
}

/// The level asked for is not one PRG offers.
pub fn prg_levels(lang: Lang) -> String {
    pick!(
        lang,
        "poziom is missing or unknown: use gmina, powiat or wojewodztwo",
        "poziom jest błędny albo go nie ma: podaj gmina, powiat albo wojewodztwo"
    )
    .to_string()
}

/// PRG answered with no feature for that name and level.
pub fn prg_no_unit(lang: Lang, name: &str, level: &str) -> String {
    pick!(
        lang,
        format!("PRG has no {level} named \"{name}\"; check the spelling"),
        format!("PRG nie ma jednostki „{name}” na poziomie {level}; sprawdź pisownię")
    )
}

/// The area now holds the unit's own boundary.
pub fn prg_area_set(lang: Lang, layer: &str, name: &str, parts: usize) -> String {
    pick!(
        lang,
        format!("area {layer} set from {name}: {parts} polygon(s)"),
        format!("obszar {layer} ustawiony z jednostki {name}: {parts} poligon(y)")
    )
}

/// The county EGiB command found no area to work from.
pub fn egib_needs_area(lang: Lang) -> String {
    pick!(
        lang,
        "make an area first: the county command reads it",
        "najpierw zrób obszar: polecenie powiatowe czyta go"
    )
    .to_string()
}

/// The area layer the request named holds no feature.
pub fn egib_empty_area(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("the area layer \"{name}\" holds no shape"),
        format!("warstwa obszaru „{name}” nie ma żadnego kształtu")
    )
}

/// ULDK named no county for the area's centre.
pub fn egib_no_county(lang: Lang) -> String {
    pick!(
        lang,
        "ULDK named no county under the area: point the map inside Poland",
        "ULDK nie podało powiatu pod obszarem: ustaw mapę w Polsce"
    )
    .to_string()
}

/// The register holds no EGiB WFS for that county.
pub fn egib_no_service(lang: Lang, county: &str, teryt: &str) -> String {
    pick!(
        lang,
        format!("the register holds no EGiB WFS for {county} (TERYT {teryt})"),
        format!("rejestr nie ma WFS EGiB dla {county} (TERYT {teryt})")
    )
}

/// The county's WFS published no parcels or buildings type.
pub fn egib_no_types(lang: Lang, county: &str) -> String {
    pick!(
        lang,
        format!("the WFS of {county} publishes no parcels or buildings type"),
        format!("WFS dla {county} nie publikuje typu działek ani budynków")
    )
}

/// The county's WFS answered with no feature in the area.
pub fn egib_empty(lang: Lang, county: &str) -> String {
    pick!(
        lang,
        format!("the WFS of {county} answered no feature in this area"),
        format!("WFS dla {county} nie zwrócił żadnego obiektu w tym obszarze")
    )
}

/// The layer built from the county's own EGiB.
pub fn egib_layer(lang: Lang, county: &str) -> String {
    pick!(
        lang,
        format!("Parcels and buildings ({county})"),
        format!("Działki i budynki ({county})")
    )
}

/// The county answered more features than one layer takes, so the rest is left out.
pub fn egib_capped(lang: Lang, cap: usize) -> String {
    pick!(
        lang,
        format!("the county answered more than {cap} features: shrink the area for the rest"),
        format!("powiat zwrócił więcej niż {cap} obiektów: zmniejsz obszar, żeby zobaczyć resztę")
    )
}

/// The county run has read part of its pages.
pub fn egib_progress(lang: Lang, count: usize) -> String {
    pick!(
        lang,
        format!("county EGiB: {count} features so far"),
        format!("EGiB powiatu: {count} obiektów na razie")
    )
}

/// The register download got no service id.
pub fn register_download_needs_service(lang: Lang) -> String {
    pick!(
        lang,
        "name the register service: {\"usluga\": \"<id>\"}",
        "podaj usługę z rejestru: {\"usluga\": \"<id>\"}"
    )
    .to_string()
}

/// A register entry is a map service, so it cannot be downloaded.
pub fn register_not_a_download(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("\"{name}\" is a WMS; the download reads a WFS"),
        format!("„{name}” to WMS; pobieranie czyta WFS")
    )
}

/// The service published nothing the filter asked for.
pub fn register_no_types(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("{name} publishes no feature type the request asks for"),
        format!("{name} nie publikuje typu, o który prosi żądanie")
    )
}

/// The service answered no feature in the area.
pub fn register_no_features(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("{name} answered no feature in this area"),
        format!("{name} nie zwrócił żadnego obiektu w tym obszarze")
    )
}

/// The layer a register download builds.
pub fn register_layer(lang: Lang, name: &str) -> String {
    pick!(
        lang,
        format!("{name} (register)"),
        format!("{name} (rejestr)")
    )
}

/// The register layer is on the map.
pub fn register_added(lang: Lang, layer: &str, count: usize) -> String {
    pick!(
        lang,
        format!("{layer}: {count} features from the register"),
        format!("{layer}: {count} obiektów z rejestru")
    )
}

/// The county layer is on the map.
pub fn egib_added(lang: Lang, layer: &str, count: usize, county: &str) -> String {
    pick!(
        lang,
        format!("{layer}: {count} features from the EGiB of {county}"),
        format!("{layer}: {count} obiektów z EGiB {county}")
    )
}

/// The CSV geocoder got no file.
pub fn csv_needs_file(lang: Lang) -> String {
    pick!(
        lang,
        "pick the CSV file with the addresses",
        "wskaż plik CSV z adresami"
    )
    .to_string()
}

/// The CSV the run carried is not readable text.
pub fn csv_bad_content(lang: Lang) -> String {
    pick!(
        lang,
        "the CSV the run carried is not base64 text",
        "CSV przekazany w przebiegu nie jest tekstem base64"
    )
    .to_string()
}

/// The CSV path could not be read.
pub fn csv_read_failed(lang: Lang, path: &str, reason: &str) -> String {
    pick!(
        lang,
        format!("{path} could not be read: {reason}"),
        format!("nie udało się przeczytać {path}: {reason}")
    )
}

/// The CSV holds no usable row.
pub fn csv_empty(lang: Lang, path: &str) -> String {
    pick!(
        lang,
        format!("{path} holds no address row"),
        format!("{path} nie ma żadnego wiersza z adresem")
    )
}

/// The geocoder is partway through the file.
pub fn csv_progress(lang: Lang, done: usize, total: usize) -> String {
    pick!(
        lang,
        format!("geocoding: {done} of {total}"),
        format!("geokodowanie: {done} z {total}")
    )
}

/// No row of the CSV could be geocoded.
pub fn csv_all_failed(lang: Lang, total: usize) -> String {
    pick!(
        lang,
        format!("UUG found none of the {total} addresses"),
        format!("UUG nie znalazło żadnego z {total} adresów")
    )
}

/// The point layer built from the CSV.
pub fn csv_layer(lang: Lang) -> String {
    pick!(lang, "Geocoded addresses", "Geokodowane adresy").to_string()
}

/// How the CSV run went.
pub fn csv_done(lang: Lang, found: usize, failed: usize) -> String {
    pick!(
        lang,
        format!("geocoded {found}, {failed} not found"),
        format!("zgeokodowane {found}, nie znaleziono {failed}")
    )
}

/// The 3D scene no longer stands on a model.
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


pub fn nmt_nothing_to_style(lang: Lang) -> String {
    pick!(
        lang,
        "No elevation model on the map yet. Add one first.",
        "Nie ma jeszcze modelu wysokości na mapie. Najpierw go dodaj."
    )
    .to_string()
}

pub fn nmt_styled(lang: Lang, layer: &str, look: &str) -> String {
    match (lang, look) {
        (Lang::Pl, "shading") => format!("{layer}: cieniowanie."),
        (Lang::Pl, "classes") => format!("{layer}: wysokości w pasmach."),
        (Lang::Pl, _) => format!("{layer}: wysokości w barwach."),
        (Lang::En, "shading") => format!("{layer}: shaded."),
        (Lang::En, "classes") => format!("{layer}: heights in bands."),
        _ => format!("{layer}: heights in colours."),
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

/// No address sits within the service's own radius of the point.
pub fn uug_no_address_here(lang: Lang) -> String {
    pick!(
        lang,
        "no address at that point",
        "brak adresu w tym punkcie",
    )
    .to_string()
}

pub fn uug_reverse_layer(lang: Lang, address: &str) -> String {
    let short: String = address.chars().take(40).collect();
    pick!(
        lang,
        format!("Address at point: {short}"),
        format!("Adres w punkcie: {short}")
    )
}

/// The address the reverse lookup found, and how far the nearest one is.
pub fn uug_here(lang: Lang, address: &str, distance: Option<f64>) -> String {
    match distance {
        Some(distance) => {
            let metres = num(lang, distance, 1);
            pick!(
                lang,
                format!("{address}, {metres} m from the point"),
                format!("{address}, {metres} m od punktu")
            )
        }
        None => pick!(lang, address.to_string(), address.to_string()),
    }
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
            count_size(Lang::Pl, "Działki (EGiB)", 5, 1_500_000),
            "Działki (EGiB): 5 obiektów, 2 MB"
        );
        assert_eq!(
            count_size(Lang::En, "Parcels (EGiB)", 5, 1_500_000),
            "Parcels (EGiB): 5 features, 2 MB"
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
    fn sums_a_sheet_run_in_both_languages() {
        assert_eq!(
            sheets_summary(Lang::Pl, 12, 45.3, 2, 1, "pobrane.csv"),
            "Pobrano 12 plików (45,3 MB), 2 pominięte, 1 błąd. Raport: pobrane.csv"
        );
        assert_eq!(
            sheets_summary(Lang::En, 12, 45.3, 2, 1, "pobrane.csv"),
            "Downloaded 12 files (45.3 MB), 2 skipped, 1 failed. Report: pobrane.csv"
        );
        assert_eq!(
            sheets_summary(Lang::Pl, 1, 0.2, 1, 5, "pobrane.csv"),
            "Pobrano 1 plik (0,2 MB), 1 pominięty, 5 błędów. Raport: pobrane.csv"
        );
        assert_eq!(sheet_progress(Lang::En, 3, 120, "godlo.zip"), "3/120: godlo.zip");
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
