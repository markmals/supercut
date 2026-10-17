//! The config file: JSONC (JSON with comments and trailing commas) read into `Config`, and the
//! JSON Schema that describes it for editors and agents.

use jsonc_parser::ParseOptions;

use super::Config;

/// The config file's name, in the platform config directory or wherever `--config` points.
pub const CONFIG_FILE: &str = "supercut.config.jsonc";
/// The schema's file name, written beside the config so its `$schema` can point at it.
pub const SCHEMA_FILE: &str = "supercut.config.schema.json";

/// JSON plus comments and trailing commas, and nothing looser: a file that parses here parses in
/// any JSONC-aware editor.
fn jsonc() -> ParseOptions {
    ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    }
}

/// The config `text` describes, or what is wrong with it: a syntax error with its line and
/// column, or a setting with its path (`shows.<slug>.chapters.number`).
pub fn parse(text: &str) -> Result<Config, String> {
    let value: serde_json::Value = jsonc_parser::parse_to_serde_value(text, &jsonc()).map_err(|error| error.to_string())?;
    serde_path_to_error::deserialize(value).map_err(|error| match error.path().to_string() {
        path if path == "." => error.inner().to_string(),
        path => format!("`{path}`: {}", error.inner()),
    })
}

/// The JSON Schema of the config file.
pub fn schema() -> serde_json::Value {
    let mut schema = schemars::schema_for!(Config);
    schema.insert("title".into(), "supercut config".into());
    schema.to_value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_starter_config_parses() {
        let config = parse(include_str!("../commands/config/starter-config.jsonc")).unwrap();
        assert!(config.shows.contains_key("example-show"));
    }

    #[test]
    fn comments_and_trailing_commas_are_fine_but_json5_extras_are_not() {
        let commented = "{\n  // where episodes live\n  \"library_dir\": \"/tmp/library\",\n}";
        assert_eq!(parse(commented).unwrap().library_dir.unwrap().to_str(), Some("/tmp/library"));
        let error = parse("{ library_dir: '/tmp/library' }").unwrap_err();
        assert!(error.contains("line 1"), "{error}");
    }

    #[test]
    fn a_wrong_setting_is_named_by_its_path() {
        let error = parse(r#"{ "shows": { "nhl": { "title": "NHL", "feeds": [], "bumper": [] } } }"#).unwrap_err();
        assert!(error.starts_with("`shows.nhl.bumper`: unknown field `bumper`"), "{error}");
    }
}
