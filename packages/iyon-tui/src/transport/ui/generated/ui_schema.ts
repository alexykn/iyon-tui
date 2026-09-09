// DO NOT EDIT. Generated from tools/tui-abi/ui_abi.toml.
// schema_blake3 = b5d1fe98d102d16d7f9533ff2044e36b675ef993fda62b8866583a45b77376e0
// generator_blake3 = 46129672d7e8216a8601be2bb6e665542a8cc397cb6eeffd96ba5f87bf06663a

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
  sizeMode: 1,
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
} as const;
export type ValueKind = typeof VALUE_KINDS[keyof typeof VALUE_KINDS];


export type ValueKindName = "SizeMode" | "U16" | "Insets" | "Alignment" | "Edges" | "Color" | "BorderStyle" | "Glyphs" | "TextAttributes" | "Style" | "LayoutMode";

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
  readonly values: readonly number[];
  readonly mask: number | undefined;
  readonly maxValue: number | undefined;
}

export const UI_VALUE_ENCODING_DESCRIPTORS: readonly UiValueEncodingDescriptor[] = [
  { valueKind: "SizeMode", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "mode", wordCount: 1, tags: [], values: [0, 1], mask: undefined, maxValue: undefined }] },
  { valueKind: "U16", encoding: "u16", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "u16", wordCount: 1, tags: [], values: [], mask: undefined, maxValue: 65535 }] },
  { valueKind: "Insets", encoding: "u16x4", minWords: 4, maxWords: 4, metadataWords: 0, forms: [{ name: "u16x4", wordCount: 4, tags: [], values: [], mask: undefined, maxValue: undefined }] },
  { valueKind: "Alignment", encoding: "axis_x2", minWords: 2, maxWords: 2, metadataWords: 0, forms: [{ name: "axis_pair", wordCount: 2, tags: [], values: [0, 1, 2, 3, 4], mask: undefined, maxValue: undefined }] },
  { valueKind: "Edges", encoding: "bool_x4", minWords: 4, maxWords: 4, metadataWords: 0, forms: [{ name: "bool_x4", wordCount: 4, tags: [], values: [0, 1], mask: undefined, maxValue: undefined }] },
  { valueKind: "Color", encoding: "ansi_or_rgb_v1", minWords: 1, maxWords: 4, metadataWords: 0, forms: [{ name: "ansi", wordCount: 1, tags: [], values: [], mask: undefined, maxValue: 255 }, { name: "rgb", wordCount: 4, tags: [2147483649], values: [], mask: undefined, maxValue: undefined }] },
  { valueKind: "BorderStyle", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "border", wordCount: 1, tags: [], values: [0, 1, 2], mask: undefined, maxValue: undefined }] },
  { valueKind: "Glyphs", encoding: "metadata_pairs_x8", minWords: 16, maxWords: 16, metadataWords: 16, forms: [{ name: "metadata_pairs_x8", wordCount: 16, tags: [], values: [], mask: undefined, maxValue: undefined }] },
  { valueKind: "TextAttributes", encoding: "set_or_clear_bits_v1", minWords: 1, maxWords: 2, metadataWords: 0, forms: [{ name: "set_or_clear", wordCount: 2, tags: [], values: [], mask: 63, maxValue: undefined }] },
  { valueKind: "Style", encoding: "direct_or_themed_style_v1", minWords: 9, maxWords: 12, metadataWords: 2, forms: [{ name: "direct", wordCount: 9, tags: [0, 1, 2], values: [], mask: 63, maxValue: undefined }, { name: "themed", wordCount: 12, tags: [0, 1, 2], values: [], mask: 63, maxValue: undefined }] },
  { valueKind: "LayoutMode", encoding: "u32_enum", minWords: 1, maxWords: 1, metadataWords: 0, forms: [{ name: "layout", wordCount: 1, tags: [], values: [0, 1, 2, 3], mask: undefined, maxValue: undefined }] },
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
}

export const UI_PROPERTY_DESCRIPTORS: readonly UiPropertyDescriptor[] = [
  { id: 0x0101, name: "width", domain: "geometry", valueKind: "SizeMode", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "size_mode", default: "unset", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput", "ContentProjection"], realization: "layout", nullable: false, clearable: true },
  { id: 0x0102, name: "height", domain: "geometry", valueKind: "SizeMode", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "size_mode", default: "unset", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: false, clearable: true },
  { id: 0x0103, name: "padding", domain: "geometry", valueKind: "Insets", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "insets", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput", "ContentProjection"], realization: "layout", nullable: false, clearable: true },
  { id: 0x0104, name: "minWidth", domain: "geometry", valueKind: "U16", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "u16", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput", "ContentProjection"], realization: "layout", nullable: true, clearable: true },
  { id: 0x0105, name: "maxWidth", domain: "geometry", valueKind: "U16", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "u16", default: "unbounded", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput", "ContentProjection"], realization: "layout", nullable: true, clearable: true },
  { id: 0x0106, name: "minHeight", domain: "geometry", valueKind: "U16", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "u16", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: true, clearable: true },
  { id: 0x0107, name: "maxHeight", domain: "geometry", valueKind: "U16", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "u16", default: "unbounded", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: true, clearable: true },
  { id: 0x0108, name: "gap", domain: "geometry", valueKind: "U16", legalKinds: ["Box", "Scroll", "Animation"], normalizer: "u16", default: "zero", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: false, clearable: true },
  { id: 0x0109, name: "alignment", domain: "geometry", valueKind: "Alignment", legalKinds: ["Box", "Scroll", "Animation"], normalizer: "alignment", default: "unset", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: false, clearable: true },
  { id: 0x010a, name: "borderEdges", domain: "geometry", valueKind: "Edges", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "border_edges", default: "unset", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput", "ContentProjection"], realization: "layout", nullable: true, clearable: true },
  { id: 0x0201, name: "foreground", domain: "presentation", valueKind: "Color", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "color", default: "inherit", reset: "unset", overrideBehavior: "explicit", inheritance: "theme", effects: ["Presentation"], realization: "paint", nullable: true, clearable: true },
  { id: 0x0202, name: "background", domain: "presentation", valueKind: "Color", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "color", default: "inherit", reset: "unset", overrideBehavior: "explicit", inheritance: "theme", effects: ["Presentation"], realization: "paint", nullable: true, clearable: true },
  { id: 0x0203, name: "borderColor", domain: "presentation", valueKind: "Color", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "color", default: "inherit", reset: "unset", overrideBehavior: "explicit", inheritance: "theme", effects: ["Presentation"], realization: "paint", nullable: true, clearable: true },
  { id: 0x0204, name: "borderStyle", domain: "presentation", valueKind: "BorderStyle", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "border_style", default: "plain", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["Presentation", "LayoutInput"], realization: "paint", nullable: true, clearable: true },
  { id: 0x0205, name: "borderGlyphs", domain: "presentation", valueKind: "Glyphs", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "glyphs", default: "style_default", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["Presentation"], realization: "paint", nullable: true, clearable: true },
  { id: 0x0206, name: "textAttributes", domain: "presentation", valueKind: "TextAttributes", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "text_attributes", default: "inherit", reset: "unset", overrideBehavior: "sparse", inheritance: "theme", effects: ["Presentation"], realization: "paint", nullable: false, clearable: true },
  { id: 0x0207, name: "style", domain: "presentation", valueKind: "Style", legalKinds: ["Box", "ContentHost", "Editor", "Scroll", "Animation"], normalizer: "style", default: "inherit", reset: "unset", overrideBehavior: "explicit", inheritance: "theme", effects: ["Presentation", "HostEnvironmentDependent"], realization: "paint", nullable: true, clearable: true },
  { id: 0x010b, name: "layout", domain: "geometry", valueKind: "LayoutMode", legalKinds: ["Box"], normalizer: "layout", default: "box", reset: "unset", overrideBehavior: "explicit", inheritance: "none", effects: ["LayoutInput"], realization: "layout", nullable: false, clearable: true },
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

function uiFiniteValueKey(normalizer: string, value: unknown): string {
  switch (normalizer) {
    case "size_mode":
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

export function uiPropertyValuesEqual(name: UiPropertyName, left: unknown, right: unknown): boolean {
  return uiPropertyValueKey(name, left) === uiPropertyValueKey(name, right);
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
    case "size_mode": return [value === "fill" ? 1 : 0];
    case "layout": { const values = uiValueEncodingForm(valueKind, "layout").values; const index = ["box", "row", "column", "grid"].indexOf(String(value)); const encoded = values[index]; if (encoded === undefined) throw new RangeError("unknown layout mode"); return [encoded]; }
    case "u16": return [uiRequiredNumber(value, name)];
    case "insets": { const item = value as { readonly top: number; readonly right: number; readonly bottom: number; readonly left: number }; return [uiRequiredNumber(item.top, name + ".top"), uiRequiredNumber(item.right, name + ".right"), uiRequiredNumber(item.bottom, name + ".bottom"), uiRequiredNumber(item.left, name + ".left")]; }
    case "alignment": { const item = value as { readonly horizontal: number; readonly vertical: number }; return [uiRequiredNumber(item.horizontal, name + ".horizontal"), uiRequiredNumber(item.vertical, name + ".vertical")]; }
    case "border_edges": return (value as readonly boolean[]).map((part) => part ? 1 : 0);
    case "color": return uiPackColor(value, valueKind);
    case "border_style": { const values = uiValueEncodingForm(valueKind, "border").values; const index = ["plain", "rounded", "double"].indexOf(String(value)); const encoded = values[index]; if (encoded === undefined) throw new RangeError("unknown border style"); return [encoded]; }
    case "glyphs": { const item = value as Record<string, string>; const words: number[] = []; for (const key of ["top", "right", "bottom", "left", "topLeft", "topRight", "bottomLeft", "bottomRight"]) words.push(...metadata(item[key])); return words; }
    case "text_attributes": return uiPackAttributes(value, valueKind, "set_or_clear");
    case "style": return uiPackStyle(value, metadata, valueKind);
    default: throw new TypeError("unknown generated UI property normalizer: " + descriptor.normalizer);
  }
}
