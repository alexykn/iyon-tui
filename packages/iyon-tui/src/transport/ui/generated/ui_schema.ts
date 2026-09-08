// DO NOT EDIT. Generated from tools/tui-abi/ui_abi.toml.
// schema_blake3 = 8ff1e9e2b87f0439ccdbd9f487068d190302196bd8545dd90d1cd4dc2f9daee0
// generator_blake3 = da9e330405afd1424c48eada9c868113caf08ee8009f434e893a0f6f028b4ba3

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
} as const;
export type ValueKind = typeof VALUE_KINDS[keyof typeof VALUE_KINDS];


export type ValueKindName = "SizeMode" | "U16" | "Insets" | "Alignment" | "Edges" | "Color" | "BorderStyle" | "Glyphs" | "TextAttributes" | "Style";

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
];

export function uiPropertyDescriptor(id: number): UiPropertyDescriptor | undefined {
  return UI_PROPERTY_DESCRIPTORS.find((property) => property.id === id);
}
