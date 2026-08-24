use std::{collections::BTreeMap, fmt::Write as _, fs, path::PathBuf};

const SCHEMA_PATH: &str = "../../packages/iyon-runtime/src/tui/bridge-schema.json";

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
    ("packedMagic", "PACKED_VIEW_MAGIC"),
    ("packedProtocolVersion", "PACKED_VIEW_PROTOCOL_VERSION"),
    ("packedRef", "PACKED_VIEW_REF"),
    ("packedDef", "PACKED_VIEW_DEF"),
    ("packedColorNone", "PACKED_COLOR_NONE"),
    ("packedColorString", "PACKED_COLOR_STRING"),
    ("packedColorAnsi", "PACKED_COLOR_ANSI"),
    ("packedOverflowNone", "PACKED_OVERFLOW_NONE"),
    ("packedOverflowEllipsis", "PACKED_OVERFLOW_ELLIPSIS"),
    ("packedOverflowFooter", "PACKED_OVERFLOW_FOOTER"),
    ("packedRuleAbsent", "PACKED_RULE_ABSENT"),
    ("packedRuleFit", "PACKED_RULE_FIT"),
    ("packedRuleFill", "PACKED_RULE_FILL"),
    ("packedBorderStyleAbsent", "PACKED_BORDER_STYLE_ABSENT"),
    ("packedBorderStylePlain", "PACKED_BORDER_STYLE_PLAIN"),
    ("packedBorderStyleRounded", "PACKED_BORDER_STYLE_ROUNDED"),
    ("packedBorderStyleDouble", "PACKED_BORDER_STYLE_DOUBLE"),
    ("packedBorderEdgesAbsent", "PACKED_BORDER_EDGES_ABSENT"),
    ("packedBorderEdgesAll", "PACKED_BORDER_EDGES_ALL"),
    (
        "packedBorderEdgesTopBottom",
        "PACKED_BORDER_EDGES_TOP_BOTTOM",
    ),
    ("packedStyleTheme", "PACKED_STYLE_THEME"),
    ("packedStyleForeground", "PACKED_STYLE_FOREGROUND"),
    ("packedStyleBackground", "PACKED_STYLE_BACKGROUND"),
    ("packedDecorationPadding", "PACKED_DECORATION_PADDING"),
    ("packedDecorationBackground", "PACKED_DECORATION_BACKGROUND"),
    ("packedDecorationForeground", "PACKED_DECORATION_FOREGROUND"),
    ("packedDecorationBorder", "PACKED_DECORATION_BORDER"),
    ("packedDecorationStyle", "PACKED_DECORATION_STYLE"),
    ("packedDecorationStates", "PACKED_DECORATION_STATES"),
    ("packedDecorationWidth", "PACKED_DECORATION_WIDTH"),
    ("packedDecorationHeight", "PACKED_DECORATION_HEIGHT"),
    ("packedDecorationMinWidth", "PACKED_DECORATION_MIN_WIDTH"),
    ("packedDecorationMaxWidth", "PACKED_DECORATION_MAX_WIDTH"),
    ("packedDecorationMinHeight", "PACKED_DECORATION_MIN_HEIGHT"),
    ("packedDecorationMaxHeight", "PACKED_DECORATION_MAX_HEIGHT"),
    ("packedBorderGlyphs", "PACKED_BORDER_GLYPHS"),
    ("packedBorderColor", "PACKED_BORDER_COLOR"),
    ("packedBorderStyle", "PACKED_BORDER_STYLE"),
    ("packedBorderEdges", "PACKED_BORDER_EDGES"),
    ("packedV3ProtocolVersion", "PACKED_V3_PROTOCOL_VERSION"),
    ("packedV3ResetGeneration", "PACKED_V3_RESET_GENERATION"),
    ("packedV3ColdClosure", "PACKED_V3_COLD_CLOSURE"),
    ("packedV3HasByteLane", "PACKED_V3_HAS_BYTE_LANE"),
    ("packedV3HasStringLane", "PACKED_V3_HAS_STRING_LANE"),
    ("packedV3DefViewFull", "PACKED_V3_DEF_VIEW_FULL"),
    ("packedV3PatchView", "PACKED_V3_PATCH_VIEW"),
    ("packedV3DefSeqLeaf", "PACKED_V3_DEF_SEQ_LEAF"),
    ("packedV3DefSeqBranch", "PACKED_V3_DEF_SEQ_BRANCH"),
    ("packedV3DefGridCellLeaf", "PACKED_V3_DEF_GRID_CELL_LEAF"),
    (
        "packedV3DefGridCellBranch",
        "PACKED_V3_DEF_GRID_CELL_BRANCH",
    ),
    ("packedV3OpRender", "PACKED_V3_OP_RENDER"),
    ("packedV3OpRenderForest", "PACKED_V3_OP_RENDER_FOREST"),
    ("packedV3PatchText", "PACKED_V3_PATCH_TEXT"),
    ("packedV3PatchDecoration", "PACKED_V3_PATCH_DECORATION"),
    ("packedV3PatchAxis", "PACKED_V3_PATCH_AXIS"),
    ("packedV3PatchGrid", "PACKED_V3_PATCH_GRID"),
    ("packedV3PatchWrap", "PACKED_V3_PATCH_WRAP"),
    ("packedV3PatchAlign", "PACKED_V3_PATCH_ALIGN"),
    ("packedV3PatchPadding", "PACKED_V3_PATCH_PADDING"),
    ("packedV3PatchWidth", "PACKED_V3_PATCH_WIDTH"),
    ("packedV3PatchHeight", "PACKED_V3_PATCH_HEIGHT"),
    ("packedV3PatchMinWidth", "PACKED_V3_PATCH_MIN_WIDTH"),
    ("packedV3PatchMaxWidth", "PACKED_V3_PATCH_MAX_WIDTH"),
    ("packedV3PatchMinHeight", "PACKED_V3_PATCH_MIN_HEIGHT"),
    ("packedV3PatchMaxHeight", "PACKED_V3_PATCH_MAX_HEIGHT"),
    ("packedV3PatchGap", "PACKED_V3_PATCH_GAP"),
    ("packedV3PatchSequence", "PACKED_V3_PATCH_SEQUENCE"),
    ("packedV3PatchGridCells", "PACKED_V3_PATCH_GRID_CELLS"),
    ("packedV3SeqColumn", "PACKED_V3_SEQ_COLUMN"),
    ("packedV3SeqRow", "PACKED_V3_SEQ_ROW"),
    ("packedV3SeqGrid", "PACKED_V3_SEQ_GRID"),
    ("packedV3WireLocalBit", "PACKED_V3_WIRE_LOCAL_BIT"),
    ("packedV3SeqBranchFactor", "PACKED_V3_SEQ_BRANCH_FACTOR"),
    ("packedV3SeqPageShift", "PACKED_V3_SEQ_PAGE_SHIFT"),
    ("packedV4ProtocolVersion", "PACKED_V4_PROTOCOL_VERSION"),
    ("packedV4ResetGeneration", "PACKED_V4_RESET_GENERATION"),
    ("packedV4ColdClosure", "PACKED_V4_COLD_CLOSURE"),
    ("packedV4HasUtf8", "PACKED_V4_HAS_UTF8"),
    ("packedV4DefViewFull", "PACKED_V4_DEF_VIEW_FULL"),
    ("packedV4PatchView", "PACKED_V4_PATCH_VIEW"),
    ("packedV4DefSeqLeaf", "PACKED_V4_DEF_SEQ_LEAF"),
    ("packedV4DefSeqBranch", "PACKED_V4_DEF_SEQ_BRANCH"),
    ("packedV4DefGridCellLeaf", "PACKED_V4_DEF_GRID_CELL_LEAF"),
    (
        "packedV4DefGridCellBranch",
        "PACKED_V4_DEF_GRID_CELL_BRANCH",
    ),
    ("packedV4OpRender", "PACKED_V4_OP_RENDER"),
    ("packedV4OpRenderForest", "PACKED_V4_OP_RENDER_FOREST"),
    ("packedV4PatchText", "PACKED_V4_PATCH_TEXT"),
    ("packedV4PatchDecoration", "PACKED_V4_PATCH_DECORATION"),
    ("packedV4PatchAxis", "PACKED_V4_PATCH_AXIS"),
    ("packedV4PatchGrid", "PACKED_V4_PATCH_GRID"),
    ("packedV4PatchWrap", "PACKED_V4_PATCH_WRAP"),
    ("packedV4PatchAlign", "PACKED_V4_PATCH_ALIGN"),
    ("packedV4PatchPadding", "PACKED_V4_PATCH_PADDING"),
    ("packedV4PatchWidth", "PACKED_V4_PATCH_WIDTH"),
    ("packedV4PatchHeight", "PACKED_V4_PATCH_HEIGHT"),
    ("packedV4PatchMinWidth", "PACKED_V4_PATCH_MIN_WIDTH"),
    ("packedV4PatchMaxWidth", "PACKED_V4_PATCH_MAX_WIDTH"),
    ("packedV4PatchMinHeight", "PACKED_V4_PATCH_MIN_HEIGHT"),
    ("packedV4PatchMaxHeight", "PACKED_V4_PATCH_MAX_HEIGHT"),
    ("packedV4PatchGap", "PACKED_V4_PATCH_GAP"),
    ("packedV4PatchSequence", "PACKED_V4_PATCH_SEQUENCE"),
    ("packedV4PatchGridCells", "PACKED_V4_PATCH_GRID_CELLS"),
    ("packedV4SeqColumn", "PACKED_V4_SEQ_COLUMN"),
    ("packedV4SeqRow", "PACKED_V4_SEQ_ROW"),
    ("packedV4SeqGrid", "PACKED_V4_SEQ_GRID"),
    ("packedV4WireLocalBit", "PACKED_V4_WIRE_LOCAL_BIT"),
    ("packedV4SeqBranchFactor", "PACKED_V4_SEQ_BRANCH_FACTOR"),
    ("packedV4SeqPageShift", "PACKED_V4_SEQ_PAGE_SHIFT"),
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
