//! The committed JSON Schemas of the record types, which must match what
//! `jumpgate::record::schema` generates from the Rust types.
//!
//! `JUMPGATE_SCHEMAS=overwrite cargo nextest run -p jumpgate --test schemas`
//! rewrites the committed files from the code.

#![cfg(test)]

use std::fs;
use std::path::PathBuf;

use jumpgate::RecordType;
use jumpgate::record::schema;

/// Returns the path of the committed schema of `record_type`.
fn committed_path(record_type: RecordType) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join(record_type.schema_file())
}

/// Returns the text that the committed schema of `record_type` must hold.
fn generated(record_type: RecordType) -> String {
    let mut text = serde_json::to_string_pretty(&schema(record_type)).unwrap();
    text.push('\n');
    text
}

/// Reports whether the run was asked to rewrite the committed schemas.
fn overwrite() -> bool {
    #[expect(
        clippy::disallowed_methods,
        reason = "the test reads one variable that asks it to rewrite the committed schemas"
    )]
    let mode = std::env::var("JUMPGATE_SCHEMAS");
    mode.is_ok_and(|mode| mode == "overwrite")
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the test reads the committed schema files and rewrites them when asked"
)]
fn every_committed_schema_matches_the_schema_the_code_generates() {
    let mut stale = Vec::new();
    for record_type in RecordType::ALL {
        let path = committed_path(*record_type);
        let expected = generated(*record_type);
        if overwrite() {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, &expected).unwrap();
            continue;
        }
        let committed = fs::read_to_string(&path).unwrap_or_default();
        if committed != expected {
            stale.push(path.display().to_string());
        }
    }
    assert!(
        stale.is_empty(),
        "these schemas differ from the code; run JUMPGATE_SCHEMAS=overwrite cargo nextest run -p jumpgate --test schemas and review the diff: {stale:?}"
    );
}
