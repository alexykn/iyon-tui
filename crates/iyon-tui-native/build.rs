use std::{collections::BTreeMap, fmt::Write as _, fs, path::PathBuf};

const SCHEMA_PATH: &str =
    "../../packages/iyon-tui/src/transport/abi/structural/schema/bridge-schema.json";

const FIELDS: &[(&str, &str)] = &[
    ("schemaVersion", "VIEW_BRIDGE_SCHEMA_VERSION"),
    ("viewText", "VIEW_KIND_TEXT"),
    ("viewDiff", "VIEW_KIND_DIFF"),
    ("viewSpacer", "VIEW_KIND_SPACER"),
    ("viewRow", "VIEW_KIND_ROW"),
    ("viewColumn", "VIEW_KIND_COLUMN"),
    ("viewHanging", "VIEW_KIND_HANGING"),
    ("viewGrid", "VIEW_KIND_GRID"),
    ("viewContainer", "VIEW_KIND_CONTAINER"),
    ("viewClamp", "VIEW_KIND_CLAMP"),
    ("viewContentMax", "VIEW_KIND_CONTENT_MAX"),
    ("viewComponent", "VIEW_KIND_COMPONENT"),
    ("viewDecorated", "VIEW_KIND_DECORATED"),
    ("layoutNormal", "LAYOUT_CHILD_NORMAL"),
    ("layoutFixed", "LAYOUT_CHILD_FIXED"),
    ("layoutFlex", "LAYOUT_CHILD_FLEX"),
    ("layoutFlexMax", "LAYOUT_CHILD_FLEX_MAX"),
    ("layoutContentMax", "LAYOUT_CHILD_CONTENT_MAX"),
    ("trackContent", "GRID_TRACK_CONTENT"),
    ("trackContentMax", "GRID_TRACK_CONTENT_MAX"),
    ("trackFixed", "GRID_TRACK_FIXED"),
    ("trackFlex", "GRID_TRACK_FLEX"),
    ("trackFlexMax", "GRID_TRACK_FLEX_MAX"),
    ("overflowNone", "OVERFLOW_NONE"),
    ("overflowEllipsis", "OVERFLOW_ELLIPSIS"),
    ("overflowFooter", "OVERFLOW_FOOTER"),
    ("wrapWordThenGrapheme", "WRAP_WORD_THEN_GRAPHEME"),
    ("wrapGrapheme", "WRAP_GRAPHEME"),
    ("wrapNoWrap", "WRAP_NO_WRAP"),
    ("horizontalStart", "ALIGN_START"),
    ("horizontalCenter", "ALIGN_CENTER"),
    ("horizontalEnd", "ALIGN_END"),
    ("verticalTop", "VERTICAL_TOP"),
    ("verticalCenter", "VERTICAL_CENTER"),
    ("verticalBottom", "VERTICAL_BOTTOM"),
    ("diffContext", "DIFF_CONTEXT"),
    ("diffAddition", "DIFF_ADDITION"),
    ("diffDeletion", "DIFF_DELETION"),
    ("terminationTerminated", "DIFF_TERMINATED"),
    ("terminationUnterminated", "DIFF_UNTERMINATED"),
];

type BridgeSchema = BTreeMap<String, u64>;

fn schema_number(schema: &BridgeSchema, field: &str) -> u32 {
    let value = schema
        .get(field)
        .copied()
        .unwrap_or_else(|| panic!("bridge schema field {field} is missing"));
    u32::try_from(value).unwrap_or_else(|_| panic!("bridge schema field {field} does not fit u32"))
}

fn main() {
    napi_build::setup();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let schema_path = manifest_dir.join(SCHEMA_PATH);
    println!("cargo:rerun-if-changed={}", schema_path.display());
    let source = fs::read_to_string(&schema_path)
        .unwrap_or_else(|error| panic!("read bridge schema {}: {error}", schema_path.display()));
    let schema: BridgeSchema = serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("parse bridge schema {}: {error}", schema_path.display()));

    let mut generated = String::new();
    for &(field, constant) in FIELDS {
        writeln!(
            generated,
            "pub const {constant}: u32 = {};",
            schema_number(&schema, field)
        )
        .expect("writing bridge schema constants cannot fail");
    }
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set"))
        .join("tui_bridge_schema.rs");
    fs::write(&output, generated)
        .unwrap_or_else(|error| panic!("write bridge schema {}: {error}", output.display()));
}
