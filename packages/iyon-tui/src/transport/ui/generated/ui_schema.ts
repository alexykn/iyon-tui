// DO NOT EDIT. Generated from tools/tui-abi/ui_abi.toml.
// schema_blake3 = aa92c1c46995f6daeab87f08ef493788c1b96ad6cb842a7322220675e9d1691e
// generator_blake3 = b24c4f5ded660a7363e3f0d9d7d248d155511bdaf4384d128827ac15e778f08d

/** Generated direct-occurrence UI schema; do not edit. */
export const UI_ABI_NAME = "iyon_tui_ui" as const;
export const UI_ABI_VERSION = 1 as const;
export const UI_SEMANTIC_SCHEMA_VERSION = 1 as const;
export const UI_BATCH_MAGIC = 0x49595549 as const;
export const UI_BATCH_VERSION = 1 as const;
export const UI_BATCH_HEADER_WORDS = 16 as const;
export const UI_ACK_HEADER_WORDS = 8 as const;
export const UI_ACK_WORDS_PER_CREATED_HANDLE = 4 as const;
export const UI_HANDLE_WORDS = 4 as const;
export const UI_LOCAL_HANDLE_HOST = 0 as const;
export const UI_LOCAL_HANDLE_GENERATION = 0 as const;
export const UI_ACK_STATUS_ERROR_BIT = 0x80000000 as const;

export const HOST_KINDS = {
  box: 1,
  contentHost: 2,
  editor: 3,
  scroll: 4,
  animation: 5,
} as const;
export type HostKind = typeof HOST_KINDS[keyof typeof HOST_KINDS];


export type HostKindName = "Box" | "ContentHost" | "Editor" | "Scroll" | "Animation";

export const HOST_KIND_CHILDREN = {
  box: "many",
  contentHost: "none",
  editor: "none",
  scroll: "many",
  animation: "frames",
} as const;

export const CONTROL_KINDS = {
  editor: 1,
  scroll: 2,
  animation: 3,
} as const;
export type ControlKind = typeof CONTROL_KINDS[keyof typeof CONTROL_KINDS];


export type ControlKindName = "Editor" | "Scroll" | "Animation";

export interface UiControlCommandDescriptor {
  readonly name: string;
  readonly code: number;
  readonly controlKind: ControlKindName;
  readonly operands: readonly string[];
}

export const UI_CONTROL_COMMAND_DESCRIPTORS: readonly UiControlCommandDescriptor[] = [
  { name: "EditorInsert", code: 1, controlKind: "Editor", operands: ["codepoint_u32"] },
  { name: "EditorSubmit", code: 2, controlKind: "Editor", operands: [] },
  { name: "EditorInsertNewline", code: 3, controlKind: "Editor", operands: [] },
  { name: "EditorBackspace", code: 4, controlKind: "Editor", operands: [] },
  { name: "EditorDelete", code: 5, controlKind: "Editor", operands: [] },
  { name: "EditorDeleteWordBackward", code: 6, controlKind: "Editor", operands: [] },
  { name: "EditorDeleteWordForward", code: 7, controlKind: "Editor", operands: [] },
  { name: "EditorKillToLineStart", code: 8, controlKind: "Editor", operands: [] },
  { name: "EditorYank", code: 9, controlKind: "Editor", operands: [] },
  { name: "EditorMoveLeft", code: 10, controlKind: "Editor", operands: [] },
  { name: "EditorMoveRight", code: 11, controlKind: "Editor", operands: [] },
  { name: "EditorMoveWordLeft", code: 12, controlKind: "Editor", operands: [] },
  { name: "EditorMoveWordRight", code: 13, controlKind: "Editor", operands: [] },
  { name: "EditorMoveLineStart", code: 14, controlKind: "Editor", operands: [] },
  { name: "EditorMoveLineEnd", code: 15, controlKind: "Editor", operands: [] },
  { name: "EditorMoveUp", code: 16, controlKind: "Editor", operands: [] },
  { name: "EditorMoveDown", code: 17, controlKind: "Editor", operands: [] },
  { name: "ScrollLineUp", code: 256, controlKind: "Scroll", operands: [] },
  { name: "ScrollLineDown", code: 257, controlKind: "Scroll", operands: [] },
  { name: "ScrollPageUp", code: 258, controlKind: "Scroll", operands: [] },
  { name: "ScrollPageDown", code: 259, controlKind: "Scroll", operands: [] },
  { name: "ScrollStart", code: 260, controlKind: "Scroll", operands: [] },
  { name: "ScrollEnd", code: 261, controlKind: "Scroll", operands: [] },
  { name: "AnimationStop", code: 512, controlKind: "Animation", operands: [] },
];

export interface UiConfigDescriptor {
  readonly name: string;
  readonly owner: "control" | "root";
  readonly kind: string;
  readonly value: string;
}

export const UI_CONFIG_DESCRIPTORS: readonly UiConfigDescriptor[] = [
  { name: "EditorMultiline", owner: "control", kind: "Editor", value: "bool" },
  { name: "AnimationIntervalMs", owner: "control", kind: "Animation", value: "u32" },
  { name: "HistoryFlowBoundary", owner: "root", kind: "LegacyHistoryUnit", value: "flow_boundary" },
  { name: "HistoryUnitIdentity", owner: "root", kind: "LegacyHistoryUnit", value: "unit_identity" },
];

export const ROOT_ROLES = {
  body: 1,
  portal: 2,
  legacyHistoryUnit: 3,
} as const;
export type RootRole = typeof ROOT_ROLES[keyof typeof ROOT_ROLES];


export type RootRoleName = "Body" | "Portal" | "LegacyHistoryUnit";

export const HANDLE_KINDS = {
  node: 1,
  port: 2,
  connector: 3,
  control: 4,
} as const;
export type HandleKind = typeof HANDLE_KINDS[keyof typeof HANDLE_KINDS];


export type HandleKindName = "Node" | "Port" | "Connector" | "Control";

export const OWNERSHIP_MODES = {
  occurrenceOwned: 1,
  explicit: 2,
} as const;
export type OwnershipMode = typeof OWNERSHIP_MODES[keyof typeof OWNERSHIP_MODES];


export type OwnershipModeName = "OccurrenceOwned" | "Explicit";

export const VALUE_KINDS = {
  u16: 2,
  insets: 3,
  alignment: 4,
  edges: 5,
  color: 6,
  borderStyle: 7,
  glyphs: 8,
  textAttributes: 9,
  style: 10,
  layoutMode: 11,
  dimension: 12,
  f32: 13,
  display: 14,
  direction: 15,
  flexDirection: 16,
  flexWrap: 17,
  position: 18,
  alignmentMode: 19,
  gridAutoFlow: 20,
  insetsF32: 21,
  trackList: 22,
  gridPlacement: 23,
} as const;
export type ValueKind = typeof VALUE_KINDS[keyof typeof VALUE_KINDS];


export type ValueKindName = "U16" | "Insets" | "Alignment" | "Edges" | "Color" | "BorderStyle" | "Glyphs" | "TextAttributes" | "Style" | "LayoutMode" | "Dimension" | "F32" | "Display" | "Direction" | "FlexDirection" | "FlexWrap" | "Position" | "AlignmentMode" | "GridAutoFlow" | "InsetsF32" | "TrackList" | "GridPlacement";

export interface UiValueEncodingDescriptor {
  readonly valueKind: ValueKindName;
  readonly encoding: string;
  readonly minWords: number;
  readonly maxWords: number;
  readonly metadataWords: number;
  readonly forms: readonly UiValueEncodingForm[];
}

export interface UiValueEncodingForm {
  readonly name: string;
  readonly wordCount: number;
  readonly tags: readonly number[];
  readonly names: readonly string[];
  readonly values: readonly number[];
  readonly mask: number | undefined;
  readonly maxValue: number | undefined;
}

export const UI_VALUE_ENCODING_DESCRIPTORS: readonly UiValueEncodingDescriptor[] = [
  { valueKind: "U16", encoding: "u16", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "u16", wordCount: 1, tags: [], names: [], values: [], mask: undefined, maxValue: 65535 }] },
  { valueKind: "Insets", encoding: "u16x4", minWords: 4, maxWords: 4, metadataWords: 0, forms: [{ name: "u16x4", wordCount: 4, tags: [], names: [], values: [], mask: undefined, maxValue: undefined }] },
  { valueKind: "Alignment", encoding: "axis_x2", minWords: 2, maxWords: 2, metadataWords: 0, forms: [{ name: "axis_pair", wordCount: 2, tags: [], names: [], values: [0, 1, 2, 3, 4], mask: undefined, maxValue: undefined }] },
  { valueKind: "Edges", encoding: "bool_x4", minWords: 4, maxWords: 4, metadataWords: 0, forms: [{ name: "bool_x4", wordCount: 4, tags: [], names: [], values: [0, 1], mask: undefined, maxValue: undefined }] },
  { valueKind: "Color", encoding: "ansi_or_rgb_v1", minWords: 1, maxWords: 4, metadataWords: 0, forms: [{ name: "ansi", wordCount: 1, tags: [], names: [], values: [], mask: undefined, maxValue: 255 }, { name: "rgb", wordCount: 4, tags: [2147483649], names: [], values: [], mask: undefined, maxValue: undefined }] },
  { valueKind: "BorderStyle", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "border", wordCount: 1, tags: [], names: ["plain", "rounded", "double"], values: [0, 1, 2], mask: undefined, maxValue: undefined }] },
  { valueKind: "Glyphs", encoding: "metadata_pairs_x8", minWords: 16, maxWords: 16, metadataWords: 16, forms: [{ name: "metadata_pairs_x8", wordCount: 16, tags: [], names: [], values: [], mask: undefined, maxValue: undefined }] },
  { valueKind: "TextAttributes", encoding: "set_or_clear_bits_v1", minWords: 1, maxWords: 2, metadataWords: 0, forms: [{ name: "set_or_clear", wordCount: 2, tags: [], names: [], values: [], mask: 63, maxValue: undefined }] },
  { valueKind: "Style", encoding: "direct_or_themed_style_v1", minWords: 9, maxWords: 12, metadataWords: 2, forms: [{ name: "direct", wordCount: 9, tags: [0, 1, 2], names: [], values: [], mask: 63, maxValue: undefined }, { name: "themed", wordCount: 12, tags: [0, 1, 2], names: [], values: [], mask: 63, maxValue: undefined }] },
  { valueKind: "LayoutMode", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "layout", wordCount: 1, tags: [], names: ["box", "row", "column", "grid"], values: [0, 1, 2, 3], mask: undefined, maxValue: undefined }] },
  { valueKind: "Dimension", encoding: "dimension_v1", minWords: 1, maxWords: 2, metadataWords: 0, forms: [{ name: "fit", wordCount: 1, tags: [0], names: [], values: [0], mask: undefined, maxValue: undefined }, { name: "fill", wordCount: 1, tags: [1], names: [], values: [1], mask: undefined, maxValue: undefined }, { name: "auto", wordCount: 1, tags: [2], names: [], values: [2], mask: undefined, maxValue: undefined }, { name: "length", wordCount: 2, tags: [3], names: [], values: [], mask: undefined, maxValue: undefined }, { name: "percent", wordCount: 2, tags: [4], names: [], values: [], mask: undefined, maxValue: undefined }] },
  { valueKind: "F32", encoding: "f32_bits_v1", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "scalar", wordCount: 1, tags: [], names: [], values: [], mask: undefined, maxValue: undefined }] },
  { valueKind: "Display", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "display", wordCount: 1, tags: [], names: ["flex", "grid", "none"], values: [0, 1, 2], mask: undefined, maxValue: undefined }] },
  { valueKind: "Direction", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "direction", wordCount: 1, tags: [], names: ["ltr", "rtl"], values: [0, 1], mask: undefined, maxValue: undefined }] },
  { valueKind: "FlexDirection", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "flex_direction", wordCount: 1, tags: [], names: ["row", "column", "rowReverse", "columnReverse"], values: [0, 1, 2, 3], mask: undefined, maxValue: undefined }] },
  { valueKind: "FlexWrap", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "flex_wrap", wordCount: 1, tags: [], names: ["nowrap", "wrap", "wrapReverse"], values: [0, 1, 2], mask: undefined, maxValue: undefined }] },
  { valueKind: "Position", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "position", wordCount: 1, tags: [], names: ["relative", "absolute"], values: [0, 1], mask: undefined, maxValue: undefined }] },
  { valueKind: "AlignmentMode", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "alignment_mode", wordCount: 1, tags: [], names: ["start", "end", "center", "stretch", "baseline", "spaceBetween", "spaceEvenly", "spaceAround"], values: [0, 1, 2, 3, 4, 5, 6, 7], mask: undefined, maxValue: undefined }] },
  { valueKind: "GridAutoFlow", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "grid_auto_flow", wordCount: 1, tags: [], names: ["row", "column", "rowDense", "columnDense"], values: [0, 1, 2, 3], mask: undefined, maxValue: undefined }] },
  { valueKind: "InsetsF32", encoding: "dimension_x4_v1", minWords: 8, maxWords: 8, metadataWords: 0, forms: [{ name: "dimension_x4", wordCount: 8, tags: [2, 3, 4], names: [], values: [], mask: undefined, maxValue: undefined }] },
  { valueKind: "TrackList", encoding: "track_list_v1", minWords: 1, maxWords: 321, metadataWords: 0, forms: [{ name: "tracks", wordCount: 1, tags: [0, 1, 2, 3, 4, 5, 6], names: [], values: [], mask: undefined, maxValue: undefined }, { name: "minmax_min", wordCount: 1, tags: [0, 1, 2, 3, 4], names: [], values: [], mask: undefined, maxValue: undefined }, { name: "minmax_max", wordCount: 1, tags: [0, 1, 2, 3, 4, 5], names: [], values: [], mask: undefined, maxValue: undefined }] },
  { valueKind: "GridPlacement", encoding: "grid_placement_v1", minWords: 4, maxWords: 4, metadataWords: 0, forms: [{ name: "placement", wordCount: 4, tags: [0, 1, 2], names: [], values: [], mask: undefined, maxValue: undefined }] },
];

export function uiValueEncoding(valueKind: ValueKindName): UiValueEncodingDescriptor {
  const descriptor = UI_VALUE_ENCODING_DESCRIPTORS.find((value) => value.valueKind === valueKind);
  if (!descriptor) throw new Error("unknown UI value encoding: " + valueKind);
  return descriptor;
}

export function uiValueEncodingForm(valueKind: ValueKindName, name: string): UiValueEncodingForm {
  const descriptor = uiValueEncoding(valueKind).forms.find((value) => value.name === name);
  if (!descriptor) throw new Error("unknown UI value encoding form: " + valueKind + "/" + name);
  return descriptor;
}

export const EFFECTS = {
  presentation: 0,
  contentProjection: 1,
  layoutInput: 2,
  interactionRuntime: 3,
  hostEnvironmentDependent: 4,
  structureGuard: 5,
} as const;
export type Effect = typeof EFFECTS[keyof typeof EFFECTS];


export type EffectName = "Presentation" | "ContentProjection" | "LayoutInput" | "InteractionRuntime" | "HostEnvironmentDependent" | "StructureGuard";

export const UI_SECTIONS = {
  structure: 1,
  state: 2,
  contentControl: 3,
  events: 4,
  contentDescriptor: 5,
} as const;

export const UI_OPCODES = {
  createNode: 0x01,
  createRoot: 0x02,
  insertBefore: 0x03,
  detach: 0x04,
  retireSubtree: 0x05,
  attachPort: 0x06,
  attachControl: 0x07,
  createControl: 0x08,
  disposeControl: 0x09,
  historyAction: 0x0a,
  retireRoot: 0x0b,
  setDeclared: 0x10,
  resetDeclared: 0x11,
  setOverride: 0x12,
  clearOverride: 0x13,
  setHidden: 0x14,
  setStyleState: 0x15,
  clearStyleState: 0x16,
  controlCommand: 0x18,
  createPort: 0x20,
  createConnector: 0x21,
  selectConnector: 0x22,
  disposeConnector: 0x23,
  disposePort: 0x24,
  setLiteralFunnel: 0x25,
  setSubscriptions: 0x30,
  replaceLiteral: 0x40,
  replaceEditorContent: 0x41,
} as const;

export interface UiOpcodeDescriptor {
  readonly name: string;
  readonly code: number;
  readonly section: keyof typeof UI_SECTIONS;
  readonly operands: readonly string[];
}

export const UI_OPCODE_DESCRIPTORS: readonly UiOpcodeDescriptor[] = [
  { name: "CreateNode", code: 0x01, section: "structure", operands: ["local_ordinal", "host_kind"] },
  { name: "CreateRoot", code: 0x02, section: "structure", operands: ["local_ordinal", "root_role", "owner_handle_or_null", "config_metadata"] },
  { name: "InsertBefore", code: 0x03, section: "structure", operands: ["parent_handle", "child_handle", "before_handle_or_null"] },
  { name: "Detach", code: 0x04, section: "structure", operands: ["parent_handle", "child_handle"] },
  { name: "RetireSubtree", code: 0x05, section: "structure", operands: ["root_handle"] },
  { name: "AttachPort", code: 0x06, section: "structure", operands: ["node_handle", "port_handle_or_null"] },
  { name: "AttachControl", code: 0x07, section: "structure", operands: ["node_handle", "control_handle_or_null"] },
  { name: "CreateControl", code: 0x08, section: "structure", operands: ["local_ordinal", "control_kind", "ownership_mode", "owner_handle_or_null", "config_metadata"] },
  { name: "DisposeControl", code: 0x09, section: "structure", operands: ["control_handle"] },
  { name: "HistoryAction", code: 0x0a, section: "structure", operands: ["unit_root_handle", "action_id"] },
  { name: "RetireRoot", code: 0x0b, section: "structure", operands: ["root_handle"] },
  { name: "SetDeclared", code: 0x10, section: "state", operands: ["node_handle", "property_id", "typed_value"] },
  { name: "ResetDeclared", code: 0x11, section: "state", operands: ["node_handle", "property_id"] },
  { name: "SetOverride", code: 0x12, section: "state", operands: ["node_handle", "property_id", "typed_value"] },
  { name: "ClearOverride", code: 0x13, section: "state", operands: ["node_handle", "property_id"] },
  { name: "SetHidden", code: 0x14, section: "state", operands: ["node_handle", "boolean"] },
  { name: "SetStyleState", code: 0x15, section: "state", operands: ["node_handle", "layer", "key_metadata", "typed_value"] },
  { name: "ClearStyleState", code: 0x16, section: "state", operands: ["node_handle", "layer", "key_metadata"] },
  { name: "ControlCommand", code: 0x18, section: "state", operands: ["control_handle", "command_id", "typed_operands"] },
  { name: "CreatePort", code: 0x20, section: "contentControl", operands: ["local_ordinal", "family", "ownership_mode", "owner_handle_or_null"] },
  { name: "CreateConnector", code: 0x21, section: "contentControl", operands: ["local_ordinal", "source_index", "port_handle", "funnel_metadata", "ownership_mode"] },
  { name: "SelectConnector", code: 0x22, section: "contentControl", operands: ["port_handle", "connector_handle_or_null"] },
  { name: "DisposeConnector", code: 0x23, section: "contentControl", operands: ["connector_handle"] },
  { name: "DisposePort", code: 0x24, section: "contentControl", operands: ["port_handle"] },
  { name: "SetLiteralFunnel", code: 0x25, section: "contentControl", operands: ["port_handle", "funnel_metadata"] },
  { name: "SetSubscriptions", code: 0x30, section: "events", operands: ["node_handle", "mask_low", "mask_high"] },
  { name: "ReplaceLiteral", code: 0x40, section: "contentDescriptor", operands: ["port_handle", "content_format", "content_bytes", "annotations_bytes"] },
  { name: "ReplaceEditorContent", code: 0x41, section: "contentDescriptor", operands: ["control_handle", "content_bytes", "expected_edit_revision"] },
];

export const UI_PROPERTIES = {
  width: 0x0101,
  height: 0x0102,
  padding: 0x0103,
  minWidth: 0x0104,
  maxWidth: 0x0105,
  minHeight: 0x0106,
  maxHeight: 0x0107,
  gap: 0x0108,
  alignment: 0x0109,
  borderEdges: 0x010a,
  foreground: 0x0201,
  background: 0x0202,
  borderColor: 0x0203,
  borderStyle: 0x0204,
  borderGlyphs: 0x0205,
  textAttributes: 0x0206,
  style: 0x0207,
  layout: 0x010b,
  display: 0x010c,
  direction: 0x010d,
  flexDirection: 0x010e,
  flexWrap: 0x010f,
  flexGrow: 0x0110,
  flexShrink: 0x0111,
  flexBasis: 0x0112,
  margin: 0x0113,
  alignItems: 0x0114,
  alignSelf: 0x0115,
  alignContent: 0x0116,
  justifyContent: 0x0117,
  justifyItems: 0x0118,
  justifySelf: 0x0119,
  columnGap: 0x011a,
  rowGap: 0x011b,
  gridTemplateColumns: 0x011c,
  gridTemplateRows: 0x011d,
  gridAutoColumns: 0x011e,
  gridAutoRows: 0x011f,
  gridAutoFlow: 0x0120,
  gridColumn: 0x0121,
  gridRow: 0x0122,
  position: 0x0123,
  inset: 0x0124,
} as const;
export type UiPropertyName = keyof typeof UI_PROPERTIES;
export type UiPropertyId = typeof UI_PROPERTIES[UiPropertyName];

export interface UiPropertyDescriptor {
  readonly id: number;
  readonly name: UiPropertyName;
  readonly domain: string;
  readonly valueKind: ValueKindName;
  readonly legalKinds: readonly HostKindName[];
  readonly normalizer: string;
  readonly default: string;
  readonly reset: string;
  readonly overrideBehavior: string;
  readonly inheritance: string;
  readonly effects: readonly EffectName[];
  readonly realization: string;
  readonly nullable: boolean;
  readonly clearable: boolean;
  readonly allowedValues: readonly string[];
}

export const UI_PROPERTY_DESCRIPTORS: readonly UiPropertyDescriptor[] = [
  { id: 0x0101, name: "width", domain: "geometry", valueKind: "Dimension", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "dimension", default: "unset", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput", "ContentProjection"], realization: "layout", nullable: false, clearable: true, allowedValues: ["fit", "fill", "length", "percent"] },
  { id: 0x0102, name: "height", domain: "geometry", valueKind: "Dimension", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "dimension", default: "unset", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: false, clearable: true, allowedValues: ["fit", "fill", "length", "percent"] },
  { id: 0x0103, name: "padding", domain: "geometry", valueKind: "Insets", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "insets", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput", "ContentProjection"], realization: "layout", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0104, name: "minWidth", domain: "geometry", valueKind: "U16", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "u16", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput", "ContentProjection"], realization: "layout", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x0105, name: "maxWidth", domain: "geometry", valueKind: "U16", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "u16", default: "unbounded", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput", "ContentProjection"], realization: "layout", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x0106, name: "minHeight", domain: "geometry", valueKind: "U16", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "u16", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x0107, name: "maxHeight", domain: "geometry", valueKind: "U16", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "u16", default: "unbounded", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x0108, name: "gap", domain: "geometry", valueKind: "U16", legalKinds: ["Box", "Scroll", "Animation"], normalizer: "u16", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0109, name: "alignment", domain: "geometry", valueKind: "Alignment", legalKinds: ["Box", "Scroll", "Animation"], normalizer: "alignment", default: "unset", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x010a, name: "borderEdges", domain: "geometry", valueKind: "Edges", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "border_edges", default: "unset", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput", "ContentProjection"], realization: "layout", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x0201, name: "foreground", domain: "presentation", valueKind: "Color", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "color", default: "inherit", reset: "unset", overrideBehavior: "explicit", inheritance: "theme", effects: ["Presentation"], realization: "paint", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x0202, name: "background", domain: "presentation", valueKind: "Color", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "color", default: "inherit", reset: "unset", overrideBehavior: "explicit", inheritance: "theme", effects: ["Presentation"], realization: "paint", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x0203, name: "borderColor", domain: "presentation", valueKind: "Color", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "color", default: "inherit", reset: "unset", overrideBehavior: "explicit", inheritance: "theme", effects: ["Presentation"], realization: "paint", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x0204, name: "borderStyle", domain: "presentation", valueKind: "BorderStyle", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "border_style", default: "plain", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["Presentation", "LayoutInput"], realization: "paint", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x0205, name: "borderGlyphs", domain: "presentation", valueKind: "Glyphs", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "glyphs", default: "style_default", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["Presentation"], realization: "paint", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x0206, name: "textAttributes", domain: "presentation", valueKind: "TextAttributes", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "text_attributes", default: "inherit", reset: "unset", overrideBehavior: "sparse", inheritance: "theme", effects: ["Presentation"], realization: "paint", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0207, name: "style", domain: "presentation", valueKind: "Style", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "style", default: "inherit", reset: "unset", overrideBehavior: "explicit", inheritance: "theme", effects: ["Presentation", "HostEnvironmentDependent"], realization: "paint", nullable: true, clearable: true, allowedValues: [] },
  { id: 0x010b, name: "layout", domain: "geometry", valueKind: "LayoutMode", legalKinds: ["Box"], normalizer: "layout", default: "box", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x010c, name: "display", domain: "geometry", valueKind: "Display", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "display", default: "flex", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_display", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x010d, name: "direction", domain: "geometry", valueKind: "Direction", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "direction", default: "ltr", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_direction", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x010e, name: "flexDirection", domain: "geometry", valueKind: "FlexDirection", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "flex_direction", default: "row", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_flex_direction", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x010f, name: "flexWrap", domain: "geometry", valueKind: "FlexWrap", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "flex_wrap", default: "nowrap", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_flex_wrap", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0110, name: "flexGrow", domain: "geometry", valueKind: "F32", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "float", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_flex_grow", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0111, name: "flexShrink", domain: "geometry", valueKind: "F32", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "float", default: "one", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_flex_shrink", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0112, name: "flexBasis", domain: "geometry", valueKind: "Dimension", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "dimension", default: "auto", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_flex_basis", nullable: false, clearable: true, allowedValues: ["auto", "length", "percent"] },
  { id: 0x0113, name: "margin", domain: "geometry", valueKind: "InsetsF32", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "dimensions", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_margin", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0114, name: "alignItems", domain: "geometry", valueKind: "AlignmentMode", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "alignment_mode", default: "stretch", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_align_items", nullable: false, clearable: true, allowedValues: ["start", "end", "center", "stretch", "baseline"] },
  { id: 0x0115, name: "alignSelf", domain: "geometry", valueKind: "AlignmentMode", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "alignment_mode", default: "auto", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_align_self", nullable: false, clearable: true, allowedValues: ["start", "end", "center", "stretch", "baseline"] },
  { id: 0x0116, name: "alignContent", domain: "geometry", valueKind: "AlignmentMode", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "alignment_mode", default: "stretch", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_align_content", nullable: false, clearable: true, allowedValues: ["start", "end", "center", "stretch", "spaceBetween", "spaceEvenly", "spaceAround"] },
  { id: 0x0117, name: "justifyContent", domain: "geometry", valueKind: "AlignmentMode", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "alignment_mode", default: "start", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_justify_content", nullable: false, clearable: true, allowedValues: ["start", "end", "center", "stretch", "spaceBetween", "spaceEvenly", "spaceAround"] },
  { id: 0x0118, name: "justifyItems", domain: "geometry", valueKind: "AlignmentMode", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "alignment_mode", default: "stretch", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_justify_items", nullable: false, clearable: true, allowedValues: ["start", "end", "center", "stretch", "baseline"] },
  { id: 0x0119, name: "justifySelf", domain: "geometry", valueKind: "AlignmentMode", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "alignment_mode", default: "auto", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_justify_self", nullable: false, clearable: true, allowedValues: ["start", "end", "center", "stretch", "baseline"] },
  { id: 0x011a, name: "columnGap", domain: "geometry", valueKind: "Dimension", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "nonnegative_dimension", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_column_gap", nullable: false, clearable: true, allowedValues: ["length", "percent"] },
  { id: 0x011b, name: "rowGap", domain: "geometry", valueKind: "Dimension", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "nonnegative_dimension", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_row_gap", nullable: false, clearable: true, allowedValues: ["length", "percent"] },
  { id: 0x011c, name: "gridTemplateColumns", domain: "geometry", valueKind: "TrackList", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "track_list", default: "empty", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_grid_template_columns", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x011d, name: "gridTemplateRows", domain: "geometry", valueKind: "TrackList", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "track_list", default: "empty", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_grid_template_rows", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x011e, name: "gridAutoColumns", domain: "geometry", valueKind: "TrackList", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "track_list", default: "empty", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_grid_auto_columns", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x011f, name: "gridAutoRows", domain: "geometry", valueKind: "TrackList", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "track_list", default: "empty", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_grid_auto_rows", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0120, name: "gridAutoFlow", domain: "geometry", valueKind: "GridAutoFlow", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "grid_auto_flow", default: "row", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_grid_auto_flow", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0121, name: "gridColumn", domain: "geometry", valueKind: "GridPlacement", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "grid_placement", default: "auto", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_grid_column", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0122, name: "gridRow", domain: "geometry", valueKind: "GridPlacement", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "grid_placement", default: "auto", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_grid_row", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0123, name: "position", domain: "geometry", valueKind: "Position", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "position", default: "relative", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_position", nullable: false, clearable: true, allowedValues: [] },
  { id: 0x0124, name: "inset", domain: "geometry", valueKind: "InsetsF32", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "dimensions", default: "auto", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "taffy_inset", nullable: false, clearable: true, allowedValues: [] },
];

export function uiPropertyDescriptor(id: number): UiPropertyDescriptor | undefined {
  return UI_PROPERTY_DESCRIPTORS.find((property) => property.id === id);
}

export function uiPropertyDescriptorByName(name: UiPropertyName): UiPropertyDescriptor {
  const descriptor = UI_PROPERTY_DESCRIPTORS.find((property) => property.name === name);
  if (!descriptor) throw new Error("unknown UI property: " + name);
  return descriptor;
}

function uiCanonicalField(value: string): string {
  return value.length + ":" + value;
}

function uiCanonicalFields(values: readonly string[]): string {
  return values.map(uiCanonicalField).join("|");
}

function uiColorValueKey(value: unknown): string {
  if (value === undefined) return "unset";
  if (typeof value !== "object" || value === null) throw new TypeError("finite color value must be an object");
  const color = value as { readonly type?: string; readonly value?: string | number; readonly r?: number; readonly g?: number; readonly b?: number };
  switch (color.type) {
    case "named":
    case "indexed": return uiCanonicalFields([color.type, String(color.value)]);
    case "rgb": return uiCanonicalFields(["rgb", String(color.r), String(color.g), String(color.b)]);
    case "theme": return uiCanonicalFields(["theme", String(color.value)]);
    default: throw new TypeError("unknown finite color form");
  }
}

function uiF32Key(value: unknown, name: string): string {
  if (typeof value !== "number" || !Number.isFinite(value)) throw new TypeError(name + " must be finite");
  const rounded = Math.fround(value);
  if (!Number.isFinite(rounded)) throw new RangeError(name + " is outside f32 range");
  const canonical = rounded === 0 ? 0 : rounded;
  return String(new Uint32Array(new Float32Array([canonical]).buffer)[0] ?? 0);
}

function uiDimensionKey(value: unknown): string {
  if (value === "fit" || value === "fill" || value === "auto") return String(value);
  if (typeof value !== "object" || value === null) throw new TypeError("finite dimension must be an object");
  const item = value as { readonly unit?: string; readonly value?: number };
  return uiCanonicalFields([String(item.unit), uiF32Key(item.value, "dimension.value")]);
}

function uiDimensionsKey(value: unknown): string {
  if (typeof value !== "object" || value === null) throw new TypeError("finite dimensions must be an object");
  const item = value as Record<string, unknown>;
  return uiCanonicalFields(["top", "right", "bottom", "left"].map((side) => uiDimensionKey(item[side])));
}

function uiTrackBoundKey(value: unknown, allowFr: boolean): string {
  if (value === "auto" || value === "minContent" || value === "maxContent") return value;
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new TypeError("typed track must be an object");
  const item = value as Record<string, unknown>;
  if (item.type === "minmax" || (item.type !== "length" && item.type !== "percent" && item.type !== "fr")) throw new TypeError("invalid track bound type");
  if (item.type === "fr" && !allowFr) throw new RangeError("fr minimum track is invalid");
  if (Object.keys(item).some((key) => key !== "type" && key !== "value")) throw new RangeError("simple track has unknown fields");
  return uiCanonicalFields([item.type, uiF32Key(item.value, "track.value")]);
}

function uiTrackKey(value: unknown): string {
  if (value === "auto" || value === "minContent" || value === "maxContent") return value;
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new TypeError("typed track must be an object");
  const item = value as Record<string, unknown>;
  if (item.type === "minmax") {
    if (Object.keys(item).some((key) => key !== "type" && key !== "min" && key !== "max")) throw new RangeError("minmax track has unknown fields");
    return uiCanonicalFields(["minmax", uiTrackBoundKey(item.min, false), uiTrackBoundKey(item.max, true)]);
  }
  return uiTrackBoundKey(value, true);
}
function uiTracksKey(value: unknown): string {
  if (!Array.isArray(value)) throw new TypeError("track list must be an array");
  return uiCanonicalFields(value.map(uiTrackKey));
}

function uiPlacementLineKey(value: unknown): string {
  if (value === undefined || value === "auto") return "auto";
  if (typeof value === "number") return "line:" + String(value);
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    const item = value as Record<string, unknown>;
    if (Object.keys(item).some((key) => key !== "span")) throw new RangeError("grid placement span has unknown fields");
    if (typeof item.span !== "number" || !Number.isSafeInteger(item.span) || item.span <= 0 || item.span > 65535) throw new RangeError("grid placement span is invalid");
    return "span:" + String(item.span);
  }
  throw new TypeError("grid placement line is invalid");
}
function uiPlacementKey(value: unknown): string {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new TypeError("grid placement must be an object");
  const item = value as Record<string, unknown>;
  if (Object.keys(item).some((key) => key !== "start" && key !== "end")) throw new RangeError("grid placement has unknown fields");
  return uiCanonicalFields([uiPlacementLineKey(item.start), uiPlacementLineKey(item.end)]);
}
function uiFiniteValueKey(normalizer: string, value: unknown): string {
  switch (normalizer) {
    case "dimension":
    case "nonnegative_dimension": return uiDimensionKey(value);
    case "dimensions": return uiDimensionsKey(value);
    case "float": return uiF32Key(value, "float");
    case "display":
    case "direction":
    case "flex_direction":
    case "flex_wrap":
    case "position":
    case "alignment_mode":
    case "grid_auto_flow": return String(value);
    case "grid_placement": return uiPlacementKey(value);
    case "track_list": return uiTracksKey(value);
    case "layout":
    case "u16":
    case "border_style": return String(value);
    case "insets": { const item = value as { readonly top: number; readonly right: number; readonly bottom: number; readonly left: number }; return uiCanonicalFields([String(item.top), String(item.right), String(item.bottom), String(item.left)]); }
    case "alignment": { const item = value as { readonly horizontal: number; readonly vertical: number }; return uiCanonicalFields([String(item.horizontal), String(item.vertical)]); }
    case "border_edges": return uiCanonicalFields((value as readonly boolean[]).map((part) => part ? "1" : "0"));
    case "color": return uiColorValueKey(value);
    case "glyphs": { const item = value as Record<string, string>; return uiCanonicalFields(["top", "right", "bottom", "left", "topLeft", "topRight", "bottomLeft", "bottomRight"].map((key) => item[key])); }
    case "text_attributes": { const item = value as Record<string, boolean>; return uiCanonicalFields(["bold", "dim", "italic", "underline", "reversed", "strikethrough"].map((key) => item[key] === undefined ? "-" : item[key] ? "1" : "0")); }
    case "style": { const item = value as { readonly foreground?: unknown; readonly background?: unknown; readonly attributes: Record<string, boolean>; readonly theme?: string }; return uiCanonicalFields([uiColorValueKey(item.foreground), uiColorValueKey(item.background), item.theme ?? "", uiFiniteValueKey("text_attributes", item.attributes)]); }
    default: throw new TypeError("unknown generated UI property normalizer: " + normalizer);
  }
}

/** Generated finite normalization/equality contract consumed by the React adapter. */
export function uiPropertyValueKey(name: UiPropertyName, value: unknown): string {
  const descriptor = uiPropertyDescriptorByName(name);
  return uiCanonicalFields([descriptor.normalizer, uiFiniteValueKey(descriptor.normalizer, value)]);
}

function uiF32Equal(left: unknown, right: unknown): boolean {
  if (typeof left !== "number" || typeof right !== "number") return false;
  const leftValue = Math.fround(left);
  const rightValue = Math.fround(right);
  if (!Number.isFinite(leftValue) || !Number.isFinite(rightValue)) return false;
  const leftBits = new Uint32Array(new Float32Array([leftValue === 0 ? 0 : leftValue]).buffer)[0];
  const rightBits = new Uint32Array(new Float32Array([rightValue === 0 ? 0 : rightValue]).buffer)[0];
  return leftBits === rightBits;
}

function uiDimensionEqual(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (typeof left === "string" || typeof right === "string") return false;
  if (typeof left !== "object" || left === null || typeof right !== "object" || right === null) return false;
  const leftValue = left as { readonly unit?: unknown; readonly value?: unknown };
  const rightValue = right as { readonly unit?: unknown; readonly value?: unknown };
  return leftValue.unit === rightValue.unit && uiF32Equal(leftValue.value, rightValue.value);
}

function uiDimensionsEqual(left: unknown, right: unknown): boolean {
  if (typeof left !== "object" || left === null || typeof right !== "object" || right === null) return false;
  const leftValue = left as Record<string, unknown>;
  const rightValue = right as Record<string, unknown>;
  return ["top", "right", "bottom", "left"].every((side) => uiDimensionEqual(leftValue[side], rightValue[side]));
}

function uiTrackEqual(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (typeof left === "string" || typeof right === "string") return false;
  if (typeof left !== "object" || left === null || typeof right !== "object" || right === null) return false;
  const leftValue = left as { readonly type?: unknown; readonly value?: unknown; readonly min?: unknown; readonly max?: unknown };
  const rightValue = right as { readonly type?: unknown; readonly value?: unknown; readonly min?: unknown; readonly max?: unknown };
  if (leftValue.type !== rightValue.type) return false;
  if (leftValue.type === "minmax") return uiTrackEqual(leftValue.min, rightValue.min) && uiTrackEqual(leftValue.max, rightValue.max);
  return uiF32Equal(leftValue.value, rightValue.value);
}

function uiTracksEqual(left: unknown, right: unknown): boolean {
  if (!Array.isArray(left) || !Array.isArray(right) || left.length !== right.length) return false;
  return left.every((value, index) => uiTrackKey(value) === uiTrackKey(right[index]));
}
function uiPlacementLineEqual(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (typeof left === "object" && left !== null && typeof right === "object" && right !== null) return (left as { readonly span?: unknown }).span === (right as { readonly span?: unknown }).span;
  return false;
}

function uiPlacementEqual(left: unknown, right: unknown): boolean {
  try {
    return uiPlacementKey(left) === uiPlacementKey(right);
  } catch {
    return false;
  }
}
export function uiPropertyValuesEqual(name: UiPropertyName, left: unknown, right: unknown): boolean {
  const descriptor = uiPropertyDescriptorByName(name);
  switch (descriptor.normalizer) {
    case "dimension":
    case "nonnegative_dimension": return uiDimensionEqual(left, right);
    case "dimensions": return uiDimensionsEqual(left, right);
    case "float": return uiF32Equal(left, right);
    case "track_list": return uiTracksEqual(left, right);
    case "grid_placement": return uiPlacementEqual(left, right);
    default: return uiPropertyValueKey(name, left) === uiPropertyValueKey(name, right);
  }
}

export function uiPropertyEncoding(name: UiPropertyName): UiValueEncodingDescriptor {
  return uiValueEncoding(uiPropertyDescriptorByName(name).valueKind);
}


export type UiMetadataWriter = (value: string) => readonly [number, number];

const UI_ANSI_NAMES = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "gray", "darkGray", "lightRed", "lightGreen", "lightYellow", "lightBlue", "lightMagenta", "lightCyan", "white"] as const;

function uiRequiredNumber(value: unknown, name: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value)) throw new TypeError(name + " must be an integer");
  return value;
}

function uiEncodingTag(valueKind: ValueKindName, formName: string, index: number): number {
  const tag = uiValueEncodingForm(valueKind, formName).tags[index];
  if (tag === undefined) throw new Error(valueKind + " / " + formName + " is missing generated tag " + index);
  return tag;
}
function uiEnumValue(value: unknown, valueKind: ValueKindName, formName: string, name: string): number {
  const form = uiValueEncodingForm(valueKind, formName);
  const index = form.names.indexOf(String(value));
  const encoded = index < 0 ? undefined : form.values[index];
  if (encoded === undefined) throw new RangeError(name + " has an unknown generated enum value");
  return encoded;
}

function uiFiniteScalar(value: unknown, name: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) throw new TypeError(name + " must be finite");
  const rounded = Math.fround(value);
  if (!Number.isFinite(rounded)) throw new RangeError(name + " is outside f32 range");
  const canonical = rounded === 0 ? 0 : rounded;
  return new Uint32Array(new Float32Array([canonical]).buffer)[0] ?? 0;
}

function uiDimensionWords(value: unknown, name: string, valueKind: ValueKindName): number[] {
  if (value === "auto") return [uiEncodingTag(valueKind, "auto", 0), 0];
  if (typeof value !== "object" || value === null) throw new TypeError(name + " must be a finite dimension");
  const item = value as { readonly unit?: string; readonly value?: number };
  const bits = uiFiniteScalar(item.value, name + ".value");
  if (item.unit === "length") return [uiEncodingTag(valueKind, "length", 0), bits];
  if (item.unit === "percent") return [uiEncodingTag(valueKind, "percent", 0), bits];
  throw new RangeError(name + " has an unknown dimension unit");
}

function uiPackDimensions(value: unknown, name: string, valueKind: ValueKindName): number[] {
  if (typeof value !== "object" || value === null) throw new TypeError(name + " must be an object");
  const item = value as Record<string, unknown>;
  return ["top", "right", "bottom", "left"].flatMap((key) => uiDimensionWords(item[key], name + "." + key, valueKind));
}

function uiTrackObject(value: unknown, name: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new TypeError(name + " must be a typed track");
  const track = value as Record<string, unknown>;
  if (Object.keys(track).some((key) => !["type", "value", "min", "max"].includes(key))) throw new RangeError(name + " has unknown fields");
  return track;
}

function uiPackSimpleTrack(value: unknown, name: string, valueKind: ValueKindName, formName = "tracks", allowFr = true): number[] {
  const track = uiTrackObject(value, name);
  if (Object.keys(track).some((key) => key !== "type" && key !== "value")) throw new RangeError(name + " simple track has unknown fields");
  const bits = uiFiniteScalar(track.value, name + ".value");
  const offset = 1;
  const tag = track.type === "length" ? uiEncodingTag(valueKind, formName, offset) : track.type === "percent" ? uiEncodingTag(valueKind, formName, offset + 1) : track.type === "fr" && allowFr ? uiEncodingTag(valueKind, formName, offset + 2) : -1;
  if (tag < 0) throw new RangeError(name + " has an invalid track bound type");
  return [tag, bits];
}

function uiPackTrackBound(value: unknown, name: string, valueKind: ValueKindName, formName: string, allowFr: boolean): number[] {
  if (value === "auto") return [uiEncodingTag(valueKind, formName, 0), 0];
  if (value === "minContent") return [uiEncodingTag(valueKind, formName, allowFr ? 4 : 3), 0];
  if (value === "maxContent") return [uiEncodingTag(valueKind, formName, allowFr ? 5 : 4), 0];
  return uiPackSimpleTrack(value, name, valueKind, formName, allowFr);
}

function uiPackTrack(value: unknown, name: string, valueKind: ValueKindName): number[] {
  if (value === "auto") return [uiEncodingTag(valueKind, "tracks", 0), 0];
  if (value === "minContent") return [uiEncodingTag(valueKind, "tracks", 4), 0];
  if (value === "maxContent") return [uiEncodingTag(valueKind, "tracks", 5), 0];
  const track = uiTrackObject(value, name);
  if (track.type === "minmax") {
    if (Object.keys(track).some((key) => key !== "type" && key !== "min" && key !== "max")) throw new RangeError(name + " minmax has unknown fields");
    return [uiEncodingTag(valueKind, "tracks", 6), ...uiPackTrackBound(track.min, name + ".min", valueKind, "minmax_min", false), ...uiPackTrackBound(track.max, name + ".max", valueKind, "minmax_max", true)];
  }
  return uiPackSimpleTrack(value, name, valueKind);
}

function uiPackTracks(value: unknown, name: string, valueKind: ValueKindName): number[] {
  if (!Array.isArray(value) || value.length > 64) throw new RangeError(name + " must contain at most 64 tracks");
  return [value.length, ...value.flatMap((track, index) => uiPackTrack(track, name + "[" + index + "]", valueKind))];
}

function uiGridLineWords(value: unknown, name: string, valueKind: ValueKindName): number[] {
  if (value === undefined || value === "auto") return [uiEncodingTag(valueKind, "placement", 0), 0];
  if (typeof value === "number" && Number.isSafeInteger(value) && value !== 0) return [uiEncodingTag(valueKind, "placement", 1), value >>> 0];
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    const item = value as Record<string, unknown>;
    if (Object.keys(item).some((key) => key !== "span")) throw new RangeError(name + " span has unknown fields");
    const span = item.span;
    if (typeof span === "number" && Number.isSafeInteger(span) && span > 0 && span <= 65535) return [uiEncodingTag(valueKind, "placement", 2), span];
  }
  throw new RangeError(name + " must be auto, a nonzero line, or a positive span");
}

function uiPackPlacement(value: unknown, name: string, valueKind: ValueKindName): number[] {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new TypeError(name + " must be an object");
  const item = value as Record<string, unknown>;
  if (Object.keys(item).some((key) => key !== "start" && key !== "end")) throw new RangeError(name + " has unknown fields");
  return [...uiGridLineWords(item.start, name + ".start", valueKind), ...uiGridLineWords(item.end, name + ".end", valueKind)];
}


function uiPackColor(value: unknown, valueKind: ValueKindName): number[] {
  if (typeof value !== "object" || value === null) throw new TypeError("finite color value must be an object");
  const color = value as { readonly type?: string; readonly value?: string | number; readonly r?: number; readonly g?: number; readonly b?: number };
  if (color.type === "rgb") return [uiEncodingTag(valueKind, valueKind === "Color" ? "rgb" : "direct", valueKind === "Color" ? 0 : 2), uiRequiredNumber(color.r, "color.r"), uiRequiredNumber(color.g, "color.g"), uiRequiredNumber(color.b, "color.b")];
  if (color.type === "indexed") return [uiRequiredNumber(color.value, "color.value")];
  if (color.type === "named") {
    const index = UI_ANSI_NAMES.indexOf(color.value as typeof UI_ANSI_NAMES[number]);
    if (index < 0) throw new RangeError("unknown ANSI color " + String(color.value));
    return [index];
  }
  throw new TypeError("theme colors require a themed Style value");
}

function uiPackAttributes(value: unknown, valueKind: ValueKindName, formName: string): [number, number] {
  if (typeof value !== "object" || value === null) throw new TypeError("finite text attributes must be an object");
  const bits: Record<string, number> = { bold: 1, dim: 2, italic: 4, underline: 8, reversed: 16, strikethrough: 32 };
  const mask = uiValueEncodingForm(valueKind, formName).mask;
  if (mask === undefined) throw new Error(valueKind + " / " + formName + " is missing generated attribute mask");
  let set = 0;
  let clear = 0;
  for (const [name, enabled] of Object.entries(value as Record<string, boolean>)) {
    const bit = bits[name];
    if (bit === undefined || typeof enabled !== "boolean") throw new RangeError("unknown finite text attribute " + name);
    if (enabled) set |= bit & mask;
    else clear |= bit & mask;
  }
  return [set, clear];
}

function uiPackStyle(value: unknown, metadata: UiMetadataWriter, valueKind: ValueKindName): number[] {
  if (typeof value !== "object" || value === null) throw new TypeError("finite Style value must be an object");
  const style = value as { readonly foreground?: unknown; readonly background?: unknown; readonly attributes: Record<string, boolean>; readonly theme?: string };
  const words = [...uiPackStyleColorImpl(style.foreground, valueKind), ...uiPackStyleColorImpl(style.background, valueKind)];
  const attributes = uiPackAttributes(style.attributes, valueKind, "direct");
  if (style.theme === undefined) return [...words, attributes[0]];
  return [...words, ...attributes, ...metadata(style.theme)];
}

function uiPackStyleColorImpl(value: unknown, valueKind: ValueKindName): number[] {
  const unset = uiEncodingTag(valueKind, "direct", 0);
  if (value === undefined) return [unset, 0, 0, 0];
  const color = value as { readonly type?: string; readonly value?: string | number; readonly r?: number; readonly g?: number; readonly b?: number };
  if (color.type === "rgb") return [uiEncodingTag(valueKind, "direct", 2), uiRequiredNumber(color.r, "style.r"), uiRequiredNumber(color.g, "style.g"), uiRequiredNumber(color.b, "style.b")];
  const ansi = uiEncodingTag(valueKind, "direct", 1);
  if (color.type === "indexed") return [ansi, uiRequiredNumber(color.value, "style.value"), 0, 0];
  if (color.type === "named") {
    const index = UI_ANSI_NAMES.indexOf(color.value as typeof UI_ANSI_NAMES[number]);
    if (index < 0) throw new RangeError("unknown ANSI color " + String(color.value));
    return [ansi, index, 0, 0];
  }
  throw new TypeError("unsupported Style color form");
}

/** Generated finite property packing; metadata is the sole sidecar writer. */
export function uiEncodePropertyValue(name: UiPropertyName, value: unknown, metadata: UiMetadataWriter): number[] {
  const descriptor = uiPropertyDescriptorByName(name);
  const valueKind = descriptor.valueKind;
  switch (descriptor.normalizer) {
    case "dimension":
    case "nonnegative_dimension": {
      if (value === "auto") return [uiEncodingTag(valueKind, "auto", 0)];
      if (value === "fit") return [uiEncodingTag(valueKind, "fit", 0)];
      if (value === "fill") return [uiEncodingTag(valueKind, "fill", 0)];
      if (typeof value !== "object" || value === null) throw new TypeError(name + " must be a finite dimension");
      const dimension = value as { readonly unit?: string; readonly value?: number };
      const scalar = uiFiniteScalar(dimension.value, name + ".value");
      if (dimension.unit === "length") return [uiEncodingTag(valueKind, "length", 0), scalar];
      if (dimension.unit === "percent") return [uiEncodingTag(valueKind, "percent", 0), scalar];
      throw new RangeError(name + " has an unknown dimension unit");
    }
    case "float": return [uiFiniteScalar(value, name)];
    case "display": return [uiEnumValue(value, valueKind, "display", name)];
    case "direction": return [uiEnumValue(value, valueKind, "direction", name)];
    case "flex_direction": return [uiEnumValue(value, valueKind, "flex_direction", name)];
    case "flex_wrap": return [uiEnumValue(value, valueKind, "flex_wrap", name)];
    case "position": return [uiEnumValue(value, valueKind, "position", name)];
    case "alignment_mode": return [uiEnumValue(value, valueKind, "alignment_mode", name)];
    case "grid_auto_flow": return [uiEnumValue(value, valueKind, "grid_auto_flow", name)];
    case "dimensions": return uiPackDimensions(value, name, valueKind);
    case "track_list": return uiPackTracks(value, name, valueKind);
    case "grid_placement": return uiPackPlacement(value, name, valueKind);
    case "layout": return [uiEnumValue(value, valueKind, "layout", name)];
    case "u16": return [uiRequiredNumber(value, name)];
    case "insets": { const item = value as { readonly top: number; readonly right: number; readonly bottom: number; readonly left: number }; return [uiRequiredNumber(item.top, name + ".top"), uiRequiredNumber(item.right, name + ".right"), uiRequiredNumber(item.bottom, name + ".bottom"), uiRequiredNumber(item.left, name + ".left")]; }
    case "alignment": { const item = value as { readonly horizontal: number; readonly vertical: number }; return [uiRequiredNumber(item.horizontal, name + ".horizontal"), uiRequiredNumber(item.vertical, name + ".vertical")]; }
    case "border_edges": return (value as readonly boolean[]).map((part) => part ? 1 : 0);
    case "color": return uiPackColor(value, valueKind);
    case "border_style": return [uiEnumValue(value, valueKind, "border", name)];
    case "glyphs": { const item = value as Record<string, string>; const words: number[] = []; for (const key of ["top", "right", "bottom", "left", "topLeft", "topRight", "bottomLeft", "bottomRight"]) words.push(...metadata(item[key])); return words; }
    case "text_attributes": return uiPackAttributes(value, valueKind, "set_or_clear");
    case "style": return uiPackStyle(value, metadata, valueKind);
    default: throw new TypeError("unknown generated UI property normalizer: " + descriptor.normalizer);
  }
}
