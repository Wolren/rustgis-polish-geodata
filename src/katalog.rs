//! The whole catalogue as JSON: what a script or an agent can ask for, and by which id.
//!
//! The ribbon shows what fits on it; this command answers with everything, so an agent does not
//! have to read `plugin.json` to learn that `usluga-pig-szczegolowa-mapa-geologiczna` exists. The
//! ids it returns are the ones `run_command` takes, and the tool names are the ones `gp.run` takes.

use crate::catalog;
use crate::services;
use crate::text::{self, Lang};
use crate::wcs;
use serde_json::{json, Value};

/// The catalogue, optionally narrowed by institution and by a word of the name.
///
/// `{"instytucja"?: "gugik"|"pig"|…, "szukaj"?: "geolog"}`; both are case-insensitive and either
/// may be left out.
pub fn run(params: &Value, lang: Lang) -> Result<Value, String> {
    let search = params
        .get("szukaj")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let institution = params
        .get("instytucja")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_lowercase();

    let service_list: Vec<Value> = services::all()
        .iter()
        .filter(|service| {
            institution.is_empty() || service.institution.eq_ignore_ascii_case(&institution)
        })
        .filter(|service| {
            search.is_empty()
                || service.name.to_lowercase().contains(&search)
                || service.cmd.contains(&search)
        })
        .map(|service| {
            let menu = if service.institution_menu.is_empty() {
                service.institution.as_str()
            } else {
                service.institution_menu.as_str()
            };
            json!({
                "command": service.cmd,
                "name": service.display(),
                "title": service.name,
                "institution": service.institution,
                "menu": menu,
            })
        })
        .collect();

    // Every dataset here is GUGiK's: another institution's filter leaves them out.
    let datasets_here = institution.is_empty() || institution == "gugik";
    let found: Vec<&catalog::WfsLayer> = catalog::wfs_layers()
        .iter()
        .filter(|layer| {
            datasets_here
                && (search.is_empty()
                    || layer.name.to_lowercase().contains(&search)
                    || layer.id.contains(&search))
        })
        .collect();
    let dataset_list: Vec<Value> = found
        .iter()
        .filter(|layer| !layer.index)
        .map(|layer| {
            json!({
                "id": layer.id,
                "title": layer.name,
                "tool": layer.tool,
                "max_span": layer.max_span,
            })
        })
        .collect();
    let index_list: Vec<Value> = found
        .iter()
        .filter(|layer| layer.index)
        .map(|layer| json!({ "id": layer.id, "title": layer.name }))
        .collect();

    // A coverage is a raster of the area, so its command takes an area like the elevation model.
    let coverage_list: Vec<Value> = wcs::all()
        .iter()
        .filter(|_| institution.is_empty() || institution == "gugik")
        .filter(|coverage| {
            search.is_empty()
                || coverage.label.to_lowercase().contains(&search)
                || coverage.coverage.to_lowercase().contains(&search)
                || coverage.cmd.contains(&search)
        })
        .map(|coverage| {
            json!({
                "command": coverage.cmd,
                "coverage": coverage.coverage,
                "title": coverage.label,
                "kind": coverage.kind,
                "crs": coverage.crs,
                "format": coverage.format,
            })
        })
        .collect();

    let counts = json!({
        "services": service_list.len(),
        "datasets": dataset_list.len(),
        "indexes": index_list.len(),
        "coverages": coverage_list.len(),
    });

    Ok(json!({
        "services": service_list,
        "datasets": dataset_list,
        "indexes": index_list,
        "coverages": coverage_list,
        "counts": counts,
        "how": {
            "service": "run_command  plugin:polish-geodata/<command>",
            "download": "gp.run  {\"tool\": \"plugin:polish-geodata/<tool>\", \"params\": {\"dane\": \"<title>\", \"obszar\": \"<area layer>\"}}",
            "coverage": "run_command  plugin:polish-geodata/<command>  {\"service\": \"<title>\", \"rozdzielczosc\": 1}",
            "area": text::area_layer(lang),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_the_whole_catalogue() {
        let answer = run(&json!({}), Lang::En).unwrap();
        assert_eq!(
            answer["services"].as_array().unwrap().len(),
            services::all().len()
        );
        assert_eq!(
            answer["datasets"].as_array().unwrap().len(),
            catalog::offered().count()
        );
        assert_eq!(
            answer["indexes"].as_array().unwrap().len(),
            catalog::wfs_layers().len() - catalog::offered().count()
        );
        assert_eq!(
            answer["coverages"].as_array().unwrap().len(),
            wcs::all().len()
        );
        counts_describe_the_lists(&answer);
    }

    /// The counts say what the lists beside them hold, also after a filter.
    fn counts_describe_the_lists(answer: &Value) {
        for (key, list) in [
            ("services", "services"),
            ("datasets", "datasets"),
            ("indexes", "indexes"),
            ("coverages", "coverages"),
        ] {
            assert_eq!(
                answer["counts"][key].as_u64().unwrap() as usize,
                answer[list].as_array().unwrap().len(),
                "{key}"
            );
        }
    }

    #[test]
    fn narrows_by_institution_and_word() {
        let pig = run(&json!({"instytucja": "pig"}), Lang::En).unwrap();
        let services = pig["services"].as_array().unwrap();
        assert!(!services.is_empty());
        assert!(services
            .iter()
            .all(|s| s["institution"].as_str() == Some("pig")));
        // The datasets and the coverages are GUGiK's, so another institution's filter leaves them out.
        assert!(pig["datasets"].as_array().unwrap().is_empty());
        assert!(pig["coverages"].as_array().unwrap().is_empty());
        counts_describe_the_lists(&pig);

        let search = run(&json!({"szukaj": "geolog"}), Lang::En).unwrap();
        assert!(search["services"].as_array().unwrap().len() < services::all().len());
        counts_describe_the_lists(&search);
    }

    #[test]
    fn every_dataset_says_which_tool_offers_it() {
        let answer = run(&json!({}), Lang::En).unwrap();
        for dataset in answer["datasets"].as_array().unwrap() {
            let tool = dataset["tool"].as_str().unwrap_or_default();
            assert!(tool.starts_with("pobierz-"), "{dataset}");
        }
    }
}
