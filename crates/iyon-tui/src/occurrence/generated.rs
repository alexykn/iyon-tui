// DO NOT EDIT. Generated from tools/tui-abi/ui_abi.toml.
// schema_blake3 = 8ff1e9e2b87f0439ccdbd9f487068d190302196bd8545dd90d1cd4dc2f9daee0
// generator_blake3 = da9e330405afd1424c48eada9c868113caf08ee8009f434e893a0f6f028b4ba3

#![allow(dead_code)]

pub const UI_ABI_NAME: &str = "iyon_tui_ui";
pub const UI_ABI_VERSION: u32 = 1;
pub const UI_SEMANTIC_SCHEMA_VERSION: u32 = 1;
pub const UI_MINIMUM_BUN: &str = "1.4.0";
pub const UI_QUALIFIED_BUN: &str = "1.4.0";
pub const UI_BATCH_MAGIC: u32 = 0x4959_5549;
pub const UI_BATCH_VERSION: u32 = 1;
pub const UI_BATCH_HEADER_WORDS: usize = 16;
pub const UI_ACK_HEADER_WORDS: usize = 8;
pub const UI_ACK_WORDS_PER_CREATED_HANDLE: usize = 4;
pub const UI_HANDLE_WORDS: usize = 4;
pub const UI_LOCAL_HANDLE_HOST: u32 = 0;
pub const UI_LOCAL_HANDLE_GENERATION: u32 = 0;
pub const UI_ACK_STATUS_ERROR_BIT: u32 = 0x8000_0000;
pub const UI_ACK_RESERVED: u32 = 0;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HostKind {
    Box = 1,
    ContentHost = 2,
    Editor = 3,
    Scroll = 4,
    Animation = 5,
}

impl HostKind {
    pub const fn from_code(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Box),
            2 => Some(Self::ContentHost),
            3 => Some(Self::Editor),
            4 => Some(Self::Scroll),
            5 => Some(Self::Animation),
            _ => None,
        }
    }

    pub const fn code(self) -> u32 {
        self as u32
    }
}

impl HostKind {
    pub const fn accepts_children(self) -> bool {
        match self {
            Self::Box => true,
            Self::ContentHost => false,
            Self::Editor => false,
            Self::Scroll => true,
            Self::Animation => true,
        }
    }

    pub const fn child_cardinality(self) -> &'static str {
        match self {
            Self::Box => "many",
            Self::ContentHost => "none",
            Self::Editor => "none",
            Self::Scroll => "many",
            Self::Animation => "frames",
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ControlKind {
    Editor = 1,
    Scroll = 2,
    Animation = 3,
}

impl ControlKind {
    pub const fn from_code(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Editor),
            2 => Some(Self::Scroll),
            3 => Some(Self::Animation),
            _ => None,
        }
    }

    pub const fn code(self) -> u32 {
        self as u32
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RootRole {
    Body = 1,
    Portal = 2,
    LegacyHistoryUnit = 3,
}

impl RootRole {
    pub const fn from_code(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Body),
            2 => Some(Self::Portal),
            3 => Some(Self::LegacyHistoryUnit),
            _ => None,
        }
    }

    pub const fn code(self) -> u32 {
        self as u32
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HandleKind {
    Node = 1,
    Port = 2,
    Connector = 3,
    Control = 4,
}

impl HandleKind {
    pub const fn from_code(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Node),
            2 => Some(Self::Port),
            3 => Some(Self::Connector),
            4 => Some(Self::Control),
            _ => None,
        }
    }

    pub const fn code(self) -> u32 {
        self as u32
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OwnershipMode {
    OccurrenceOwned = 1,
    Explicit = 2,
}

impl OwnershipMode {
    pub const fn from_code(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::OccurrenceOwned),
            2 => Some(Self::Explicit),
            _ => None,
        }
    }

    pub const fn code(self) -> u32 {
        self as u32
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ValueKind {
    SizeMode = 1,
    U16 = 2,
    Insets = 3,
    Alignment = 4,
    Edges = 5,
    Color = 6,
    BorderStyle = 7,
    Glyphs = 8,
    TextAttributes = 9,
    Style = 10,
}

impl ValueKind {
    pub const fn from_code(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::SizeMode),
            2 => Some(Self::U16),
            3 => Some(Self::Insets),
            4 => Some(Self::Alignment),
            5 => Some(Self::Edges),
            6 => Some(Self::Color),
            7 => Some(Self::BorderStyle),
            8 => Some(Self::Glyphs),
            9 => Some(Self::TextAttributes),
            10 => Some(Self::Style),
            _ => None,
        }
    }

    pub const fn code(self) -> u32 {
        self as u32
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Presentation = 0,
    ContentProjection = 1,
    LayoutInput = 2,
    InteractionRuntime = 3,
    HostEnvironmentDependent = 4,
    StructureGuard = 5,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectMask(pub u32);

impl EffectMask {
    pub const NONE: Self = Self(0);
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

pub const EFFECT_PRESENTATION: EffectMask = EffectMask(1 << 0);
pub const EFFECT_CONTENT_PROJECTION: EffectMask = EffectMask(1 << 1);
pub const EFFECT_LAYOUT_INPUT: EffectMask = EffectMask(1 << 2);
pub const EFFECT_INTERACTION_RUNTIME: EffectMask = EffectMask(1 << 3);
pub const EFFECT_HOST_ENVIRONMENT_DEPENDENT: EffectMask = EffectMask(1 << 4);
pub const EFFECT_STRUCTURE_GUARD: EffectMask = EffectMask(1 << 5);

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiSection {
    Structure = 1,
    State = 2,
    ContentControl = 3,
    Events = 4,
    ContentDescriptor = 5,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiOpcode {
    CreateNode = 0x01,
    CreateRoot = 0x02,
    InsertBefore = 0x03,
    Detach = 0x04,
    RetireSubtree = 0x05,
    AttachPort = 0x06,
    AttachControl = 0x07,
    CreateControl = 0x08,
    DisposeControl = 0x09,
    HistoryAction = 0x0a,
    RetireRoot = 0x0b,
    SetDeclared = 0x10,
    ResetDeclared = 0x11,
    SetOverride = 0x12,
    ClearOverride = 0x13,
    SetHidden = 0x14,
    SetStyleState = 0x15,
    ClearStyleState = 0x16,
    ControlCommand = 0x18,
    CreatePort = 0x20,
    CreateConnector = 0x21,
    SelectConnector = 0x22,
    DisposeConnector = 0x23,
    DisposePort = 0x24,
    SetLiteralFunnel = 0x25,
    SetSubscriptions = 0x30,
    ReplaceLiteral = 0x40,
    ReplaceEditorContent = 0x41,
}

#[derive(Clone, Copy, Debug)]
pub struct OpcodeDescriptor {
    pub opcode: UiOpcode,
    pub section: UiSection,
    pub name: &'static str,
    pub operands: &'static [&'static str],
}

pub const OPCODE_DESCRIPTORS: &[OpcodeDescriptor] = &[
    OpcodeDescriptor {
        opcode: UiOpcode::CreateNode,
        section: UiSection::Structure,
        name: "CreateNode",
        operands: &["local_ordinal", "host_kind"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::CreateRoot,
        section: UiSection::Structure,
        name: "CreateRoot",
        operands: &[
            "local_ordinal",
            "root_role",
            "owner_handle_or_null",
            "config_metadata",
        ],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::InsertBefore,
        section: UiSection::Structure,
        name: "InsertBefore",
        operands: &["parent_handle", "child_handle", "before_handle_or_null"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::Detach,
        section: UiSection::Structure,
        name: "Detach",
        operands: &["parent_handle", "child_handle"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::RetireSubtree,
        section: UiSection::Structure,
        name: "RetireSubtree",
        operands: &["root_handle"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::AttachPort,
        section: UiSection::Structure,
        name: "AttachPort",
        operands: &["node_handle", "port_handle_or_null"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::AttachControl,
        section: UiSection::Structure,
        name: "AttachControl",
        operands: &["node_handle", "control_handle_or_null"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::CreateControl,
        section: UiSection::Structure,
        name: "CreateControl",
        operands: &[
            "local_ordinal",
            "control_kind",
            "ownership_mode",
            "owner_handle_or_null",
            "config_metadata",
        ],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::DisposeControl,
        section: UiSection::Structure,
        name: "DisposeControl",
        operands: &["control_handle"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::HistoryAction,
        section: UiSection::Structure,
        name: "HistoryAction",
        operands: &["unit_root_handle", "action_id"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::RetireRoot,
        section: UiSection::Structure,
        name: "RetireRoot",
        operands: &["root_handle"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::SetDeclared,
        section: UiSection::State,
        name: "SetDeclared",
        operands: &["node_handle", "property_id", "typed_value"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::ResetDeclared,
        section: UiSection::State,
        name: "ResetDeclared",
        operands: &["node_handle", "property_id"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::SetOverride,
        section: UiSection::State,
        name: "SetOverride",
        operands: &["node_handle", "property_id", "typed_value"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::ClearOverride,
        section: UiSection::State,
        name: "ClearOverride",
        operands: &["node_handle", "property_id"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::SetHidden,
        section: UiSection::State,
        name: "SetHidden",
        operands: &["node_handle", "boolean"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::SetStyleState,
        section: UiSection::State,
        name: "SetStyleState",
        operands: &["node_handle", "layer", "key_metadata", "typed_value"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::ClearStyleState,
        section: UiSection::State,
        name: "ClearStyleState",
        operands: &["node_handle", "layer", "key_metadata"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::ControlCommand,
        section: UiSection::State,
        name: "ControlCommand",
        operands: &["control_handle", "command_id", "typed_operands"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::CreatePort,
        section: UiSection::ContentControl,
        name: "CreatePort",
        operands: &[
            "local_ordinal",
            "family",
            "ownership_mode",
            "owner_handle_or_null",
        ],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::CreateConnector,
        section: UiSection::ContentControl,
        name: "CreateConnector",
        operands: &[
            "local_ordinal",
            "source_index",
            "port_handle",
            "funnel_metadata",
            "ownership_mode",
        ],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::SelectConnector,
        section: UiSection::ContentControl,
        name: "SelectConnector",
        operands: &["port_handle", "connector_handle_or_null"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::DisposeConnector,
        section: UiSection::ContentControl,
        name: "DisposeConnector",
        operands: &["connector_handle"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::DisposePort,
        section: UiSection::ContentControl,
        name: "DisposePort",
        operands: &["port_handle"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::SetLiteralFunnel,
        section: UiSection::ContentControl,
        name: "SetLiteralFunnel",
        operands: &["port_handle", "funnel_metadata"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::SetSubscriptions,
        section: UiSection::Events,
        name: "SetSubscriptions",
        operands: &["node_handle", "mask_low", "mask_high"],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::ReplaceLiteral,
        section: UiSection::ContentDescriptor,
        name: "ReplaceLiteral",
        operands: &[
            "port_handle",
            "content_format",
            "content_bytes",
            "annotations_bytes",
        ],
    },
    OpcodeDescriptor {
        opcode: UiOpcode::ReplaceEditorContent,
        section: UiSection::ContentDescriptor,
        name: "ReplaceEditorContent",
        operands: &["control_handle", "content_bytes", "expected_edit_revision"],
    },
];

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyId {
    Width = 0x0101,
    Height = 0x0102,
    Padding = 0x0103,
    MinWidth = 0x0104,
    MaxWidth = 0x0105,
    MinHeight = 0x0106,
    MaxHeight = 0x0107,
    Gap = 0x0108,
    Alignment = 0x0109,
    BorderEdges = 0x010a,
    Foreground = 0x0201,
    Background = 0x0202,
    BorderColor = 0x0203,
    BorderStyle = 0x0204,
    BorderGlyphs = 0x0205,
    TextAttributes = 0x0206,
    Style = 0x0207,
}

impl PropertyId {
    pub const ALL: &[Self] = &[
        Self::Width,
        Self::Height,
        Self::Padding,
        Self::MinWidth,
        Self::MaxWidth,
        Self::MinHeight,
        Self::MaxHeight,
        Self::Gap,
        Self::Alignment,
        Self::BorderEdges,
        Self::Foreground,
        Self::Background,
        Self::BorderColor,
        Self::BorderStyle,
        Self::BorderGlyphs,
        Self::TextAttributes,
        Self::Style,
    ];

    pub const fn from_raw(value: u32) -> Option<Self> {
        match value {
            0x0101 => Some(Self::Width),
            0x0102 => Some(Self::Height),
            0x0103 => Some(Self::Padding),
            0x0104 => Some(Self::MinWidth),
            0x0105 => Some(Self::MaxWidth),
            0x0106 => Some(Self::MinHeight),
            0x0107 => Some(Self::MaxHeight),
            0x0108 => Some(Self::Gap),
            0x0109 => Some(Self::Alignment),
            0x010a => Some(Self::BorderEdges),
            0x0201 => Some(Self::Foreground),
            0x0202 => Some(Self::Background),
            0x0203 => Some(Self::BorderColor),
            0x0204 => Some(Self::BorderStyle),
            0x0205 => Some(Self::BorderGlyphs),
            0x0206 => Some(Self::TextAttributes),
            0x0207 => Some(Self::Style),
            _ => None,
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Width => 0,
            Self::Height => 1,
            Self::Padding => 2,
            Self::MinWidth => 3,
            Self::MaxWidth => 4,
            Self::MinHeight => 5,
            Self::MaxHeight => 6,
            Self::Gap => 7,
            Self::Alignment => 8,
            Self::BorderEdges => 9,
            Self::Foreground => 10,
            Self::Background => 11,
            Self::BorderColor => 12,
            Self::BorderStyle => 13,
            Self::BorderGlyphs => 14,
            Self::TextAttributes => 15,
            Self::Style => 16,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PropertyDescriptor {
    pub id: PropertyId,
    pub name: &'static str,
    pub domain: &'static str,
    pub value_kind: ValueKind,
    pub legal_kinds: &'static [HostKind],
    pub normalizer: &'static str,
    pub default: &'static str,
    pub reset: &'static str,
    pub override_behavior: &'static str,
    pub inheritance: &'static str,
    pub effects: EffectMask,
    pub realization: &'static str,
    pub nullable: bool,
    pub clearable: bool,
}

pub const PROPERTY_DESCRIPTORS: &[PropertyDescriptor] = &[
    PropertyDescriptor {
        id: PropertyId::Width,
        name: "width",
        domain: "geometry",
        value_kind: ValueKind::SizeMode,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "size_mode",
        default: "unset",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE
            .union(EffectMask(1 << 2))
            .union(EffectMask(1 << 1)),
        realization: "layout",
        nullable: false,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::Height,
        name: "height",
        domain: "geometry",
        value_kind: ValueKind::SizeMode,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "size_mode",
        default: "unset",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE.union(EffectMask(1 << 2)),
        realization: "layout",
        nullable: false,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::Padding,
        name: "padding",
        domain: "geometry",
        value_kind: ValueKind::Insets,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "insets",
        default: "zero",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE
            .union(EffectMask(1 << 2))
            .union(EffectMask(1 << 1)),
        realization: "layout",
        nullable: false,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::MinWidth,
        name: "minWidth",
        domain: "geometry",
        value_kind: ValueKind::U16,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "u16",
        default: "zero",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE
            .union(EffectMask(1 << 2))
            .union(EffectMask(1 << 1)),
        realization: "layout",
        nullable: true,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::MaxWidth,
        name: "maxWidth",
        domain: "geometry",
        value_kind: ValueKind::U16,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "u16",
        default: "unbounded",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE
            .union(EffectMask(1 << 2))
            .union(EffectMask(1 << 1)),
        realization: "layout",
        nullable: true,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::MinHeight,
        name: "minHeight",
        domain: "geometry",
        value_kind: ValueKind::U16,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "u16",
        default: "zero",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE.union(EffectMask(1 << 2)),
        realization: "layout",
        nullable: true,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::MaxHeight,
        name: "maxHeight",
        domain: "geometry",
        value_kind: ValueKind::U16,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "u16",
        default: "unbounded",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE.union(EffectMask(1 << 2)),
        realization: "layout",
        nullable: true,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::Gap,
        name: "gap",
        domain: "geometry",
        value_kind: ValueKind::U16,
        legal_kinds: &[HostKind::Box, HostKind::Scroll, HostKind::Animation],
        normalizer: "u16",
        default: "zero",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE.union(EffectMask(1 << 2)),
        realization: "layout",
        nullable: false,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::Alignment,
        name: "alignment",
        domain: "geometry",
        value_kind: ValueKind::Alignment,
        legal_kinds: &[HostKind::Box, HostKind::Scroll, HostKind::Animation],
        normalizer: "alignment",
        default: "unset",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE.union(EffectMask(1 << 2)),
        realization: "layout",
        nullable: false,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::BorderEdges,
        name: "borderEdges",
        domain: "geometry",
        value_kind: ValueKind::Edges,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "border_edges",
        default: "unset",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE
            .union(EffectMask(1 << 2))
            .union(EffectMask(1 << 1)),
        realization: "layout",
        nullable: true,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::Foreground,
        name: "foreground",
        domain: "presentation",
        value_kind: ValueKind::Color,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "color",
        default: "inherit",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "theme",
        effects: EffectMask::NONE.union(EffectMask(1 << 0)),
        realization: "paint",
        nullable: true,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::Background,
        name: "background",
        domain: "presentation",
        value_kind: ValueKind::Color,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "color",
        default: "inherit",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "theme",
        effects: EffectMask::NONE.union(EffectMask(1 << 0)),
        realization: "paint",
        nullable: true,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::BorderColor,
        name: "borderColor",
        domain: "presentation",
        value_kind: ValueKind::Color,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "color",
        default: "inherit",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "theme",
        effects: EffectMask::NONE.union(EffectMask(1 << 0)),
        realization: "paint",
        nullable: true,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::BorderStyle,
        name: "borderStyle",
        domain: "presentation",
        value_kind: ValueKind::BorderStyle,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "border_style",
        default: "plain",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE
            .union(EffectMask(1 << 0))
            .union(EffectMask(1 << 2)),
        realization: "paint",
        nullable: true,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::BorderGlyphs,
        name: "borderGlyphs",
        domain: "presentation",
        value_kind: ValueKind::Glyphs,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "glyphs",
        default: "style_default",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "none",
        effects: EffectMask::NONE.union(EffectMask(1 << 0)),
        realization: "paint",
        nullable: true,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::TextAttributes,
        name: "textAttributes",
        domain: "presentation",
        value_kind: ValueKind::TextAttributes,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "text_attributes",
        default: "inherit",
        reset: "unset",
        override_behavior: "sparse",
        inheritance: "theme",
        effects: EffectMask::NONE.union(EffectMask(1 << 0)),
        realization: "paint",
        nullable: false,
        clearable: true,
    },
    PropertyDescriptor {
        id: PropertyId::Style,
        name: "style",
        domain: "presentation",
        value_kind: ValueKind::Style,
        legal_kinds: &[
            HostKind::Box,
            HostKind::ContentHost,
            HostKind::Editor,
            HostKind::Scroll,
            HostKind::Animation,
        ],
        normalizer: "style",
        default: "inherit",
        reset: "unset",
        override_behavior: "explicit",
        inheritance: "theme",
        effects: EffectMask::NONE
            .union(EffectMask(1 << 0))
            .union(EffectMask(1 << 4)),
        realization: "paint",
        nullable: true,
        clearable: true,
    },
];

pub const PROPERTY_COUNT: usize = PROPERTY_DESCRIPTORS.len();

pub const fn property_descriptor(id: PropertyId) -> &'static PropertyDescriptor {
    &PROPERTY_DESCRIPTORS[id.index()]
}
