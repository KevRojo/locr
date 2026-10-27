//! Versioned JSON schema shared by the C ABI, CLI, WASM and Python bindings.
//!
//! Result:
//!
//! ```json
//! {"version":1,"text":"HELLO LOCR","score":0.93,"confidence":0.97,
//!  "transform":"none","attempts":1,"width":400,"height":120,
//!  "lines":[{"text":"HELLO LOCR","confidence":0.97,
//!            "bbox":{"x":18,"y":26,"width":197,"height":27},
//!            "words":[{"text":"HELLO","confidence":0.98,"bbox":{...}}, ...]}]}
//! ```
//!
//! Options (every key optional, unknown keys rejected):
//!
//! ```json
//! {"allowed_chars":"0123456789","beam_width":5,
//!  "auto_enhance":true,"min_score":0.55,"max_attempts":7}
//! ```

use serde::Serialize;

use crate::{EnhanceOptions, LocrError, OcrOptions, OcrResult, Result};

/// Version of the JSON result schema. Bumped on breaking changes only;
/// new fields may appear within a version.
pub const JSON_SCHEMA_VERSION: u32 = 1;

impl OcrResult {
    /// Serialize as the versioned JSON schema (see [`JSON_SCHEMA_VERSION`]).
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct Versioned<'a> {
            version: u32,
            #[serde(flatten)]
            result: &'a OcrResult,
        }
        serde_json::to_string(&Versioned {
            version: JSON_SCHEMA_VERSION,
            result: self,
        })
        .expect("OcrResult is always serializable")
    }
}

const OPTION_KEYS: [&str; 5] = [
    "allowed_chars",
    "beam_width",
    "auto_enhance",
    "min_score",
    "max_attempts",
];

impl OcrOptions {
    /// Parse options from JSON. An empty string means defaults; `null`
    /// values mean "not set".
    pub fn from_json(json: &str) -> Result<OcrOptions> {
        let invalid = |msg: String| LocrError::InvalidInput(format!("invalid options JSON: {msg}"));
        if json.trim().is_empty() {
            return Ok(OcrOptions::default());
        }
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        let serde_json::Value::Object(map) = value else {
            return Err(invalid("expected an object".into()));
        };
        if let Some(key) = map.keys().find(|k| !OPTION_KEYS.contains(&k.as_str())) {
            return Err(invalid(format!(
                "unknown option `{key}`, expected one of {}",
                OPTION_KEYS.join(", ")
            )));
        }
        // Field by field, so type errors name the offending option.
        fn field<T: serde::de::DeserializeOwned>(
            map: &serde_json::Map<String, serde_json::Value>,
            key: &str,
        ) -> Result<Option<T>> {
            match map.get(key) {
                None | Some(serde_json::Value::Null) => Ok(None),
                Some(v) => T::deserialize(v).map(Some).map_err(|e| {
                    LocrError::InvalidInput(format!("invalid options JSON: `{key}`: {e}"))
                }),
            }
        }

        let defaults = EnhanceOptions::default();
        let auto_enhance = field::<bool>(&map, "auto_enhance")?.unwrap_or(false);
        let min_score = field::<f32>(&map, "min_score")?.unwrap_or(defaults.min_score);
        let max_attempts = field::<u32>(&map, "max_attempts")?.unwrap_or(defaults.max_attempts);
        Ok(OcrOptions {
            allowed_chars: field(&map, "allowed_chars")?,
            beam_width: field(&map, "beam_width")?.unwrap_or(0),
            enhance: auto_enhance.then_some(EnhanceOptions {
                min_score,
                max_attempts,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoundingBox, Line, TransformUsed, Word};

    #[test]
    fn result_schema() {
        let bbox = BoundingBox {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        };
        let line = Line {
            text: "hi".into(),
            confidence: Some(0.5),
            bbox,
            words: vec![Word {
                text: "hi".into(),
                confidence: None,
                bbox,
            }],
        };
        let result = OcrResult::from_lines(vec![line], 10, 20, TransformUsed::Contrast(30.0), 2);
        let json: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["text"], "hi");
        assert_eq!(json["transform"], "contrast:30");
        assert_eq!(json["attempts"], 2);
        assert_eq!(
            (json["width"].clone(), json["height"].clone()),
            (10.into(), 20.into())
        );
        assert_eq!(json["confidence"], 0.5);
        assert_eq!(
            json["lines"][0]["bbox"],
            serde_json::json!({"x": 1, "y": 2, "width": 3, "height": 4})
        );
        assert_eq!(
            json["lines"][0]["words"][0]["confidence"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn options_from_json() {
        assert_eq!(OcrOptions::from_json("").unwrap(), OcrOptions::default());
        assert_eq!(OcrOptions::from_json("{}").unwrap(), OcrOptions::default());

        let opts = OcrOptions::from_json(
            r#"{"allowed_chars":"0123456789","beam_width":4,"auto_enhance":true,"max_attempts":3}"#,
        )
        .unwrap();
        assert_eq!(opts.allowed_chars.as_deref(), Some("0123456789"));
        assert_eq!(opts.beam_width, 4);
        assert_eq!(
            opts.enhance,
            Some(EnhanceOptions {
                min_score: 0.55,
                max_attempts: 3
            })
        );

        // Enhance settings without `auto_enhance` stay off.
        assert_eq!(
            OcrOptions::from_json(r#"{"min_score":0.9}"#)
                .unwrap()
                .enhance,
            None
        );
    }

    #[test]
    fn options_reject_typos() {
        let err = OcrOptions::from_json(r#"{"allowed_char":"0"}"#).unwrap_err();
        assert!(matches!(err, LocrError::InvalidInput(msg) if msg.contains("`allowed_char`")));
        assert!(OcrOptions::from_json("[1, 2]").is_err());
        assert!(OcrOptions::from_json("{").is_err());
    }

    #[test]
    fn option_type_errors_name_the_option() {
        let err = OcrOptions::from_json(r#"{"beam_width":"wide"}"#).unwrap_err();
        assert!(matches!(err, LocrError::InvalidInput(msg) if msg.contains("`beam_width`")));
        let err = OcrOptions::from_json(r#"{"beam_width":-1}"#).unwrap_err();
        assert!(matches!(err, LocrError::InvalidInput(msg) if msg.contains("`beam_width`")));
    }

    #[test]
    fn null_options_are_unset() {
        let opts = OcrOptions::from_json(
            r#"{"allowed_chars":null,"beam_width":null,"auto_enhance":null}"#,
        )
        .unwrap();
        assert_eq!(opts, OcrOptions::default());
    }
}
