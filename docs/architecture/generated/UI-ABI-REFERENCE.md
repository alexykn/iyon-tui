<!-- DO NOT EDIT. Generated from tools/tui-abi/ui_abi.toml. schema_blake3 = aa92c1c46995f6daeab87f08ef493788c1b96ad6cb842a7322220675e9d1691e; generator_blake3 = b24c4f5ded660a7363e3f0d9d7d248d155511bdaf4384d128827ac15e778f08d -->

# Direct UI ABI

## Wire constants

| Name | Value |
|---|---:|
| magic | 1230591305 |
| batch_version | 1 |
| batch_header_words | 16 |
| ack_header_words | 8 |
| ack_words_per_created_handle | 4 |
| handle_words | 4 |
| local_handle_host | 0 |
| local_handle_generation | 0 |
| ack_status_error_bit | 2147483648 |
| ack_reserved | 0 |

## Sections

| Name | Code |
|---|---:|
| structure | 1 |
| state | 2 |
| content_control | 3 |
| events | 4 |
| content_descriptor | 5 |
## Host kinds

| Name | Code | Children |
|---|---:|---|
| Box | 1 | many |
| ContentHost | 2 | none |
| Editor | 3 | none |
| Scroll | 4 | many |
| Animation | 5 | frames |

## Value encodings

| Value kind | Encoding | Min words | Max words | Metadata words | Forms |
|---|---|---:|---:|---:|---|
| U16 | u16 | 1 | 1 | 0 | u16 |
| Insets | u16x4 | 4 | 4 | 0 | u16x4 |
| Alignment | axis_x2 | 2 | 2 | 0 | axis_pair |
| Edges | bool_x4 | 4 | 4 | 0 | bool_x4 |
| Color | ansi_or_rgb_v1 | 1 | 4 | 0 | ansi, rgb |
| BorderStyle | u32_enum | 1 | 1 | 0 | border |
| Glyphs | metadata_pairs_x8 | 16 | 16 | 16 | metadata_pairs_x8 |
| TextAttributes | set_or_clear_bits_v1 | 1 | 2 | 0 | set_or_clear |
| Style | direct_or_themed_style_v1 | 9 | 12 | 2 | direct, themed |
| LayoutMode | u32_enum | 1 | 1 | 0 | layout |
| Dimension | dimension_v1 | 1 | 2 | 0 | fit, fill, auto, length, percent |
| F32 | f32_bits_v1 | 1 | 1 | 0 | scalar |
| Display | u32_enum | 1 | 1 | 0 | display |
| Direction | u32_enum | 1 | 1 | 0 | direction |
| FlexDirection | u32_enum | 1 | 1 | 0 | flex_direction |
| FlexWrap | u32_enum | 1 | 1 | 0 | flex_wrap |
| Position | u32_enum | 1 | 1 | 0 | position |
| AlignmentMode | u32_enum | 1 | 1 | 0 | alignment_mode |
| GridAutoFlow | u32_enum | 1 | 1 | 0 | grid_auto_flow |
| InsetsF32 | dimension_x4_v1 | 8 | 8 | 0 | dimension_x4 |
| TrackList | track_list_v1 | 1 | 321 | 0 | tracks, minmax_min, minmax_max |
| GridPlacement | grid_placement_v1 | 4 | 4 | 0 | placement |

## Control commands

| Name | Code | Control kind | Operands |
|---|---:|---|---|
| EditorInsert | 0x1 | Editor | codepoint_u32 |
| EditorSubmit | 0x2 | Editor |  |
| EditorInsertNewline | 0x3 | Editor |  |
| EditorBackspace | 0x4 | Editor |  |
| EditorDelete | 0x5 | Editor |  |
| EditorDeleteWordBackward | 0x6 | Editor |  |
| EditorDeleteWordForward | 0x7 | Editor |  |
| EditorKillToLineStart | 0x8 | Editor |  |
| EditorYank | 0x9 | Editor |  |
| EditorMoveLeft | 0xa | Editor |  |
| EditorMoveRight | 0xb | Editor |  |
| EditorMoveWordLeft | 0xc | Editor |  |
| EditorMoveWordRight | 0xd | Editor |  |
| EditorMoveLineStart | 0xe | Editor |  |
| EditorMoveLineEnd | 0xf | Editor |  |
| EditorMoveUp | 0x10 | Editor |  |
| EditorMoveDown | 0x11 | Editor |  |
| ScrollLineUp | 0x100 | Scroll |  |
| ScrollLineDown | 0x101 | Scroll |  |
| ScrollPageUp | 0x102 | Scroll |  |
| ScrollPageDown | 0x103 | Scroll |  |
| ScrollStart | 0x104 | Scroll |  |
| ScrollEnd | 0x105 | Scroll |  |
| AnimationStop | 0x200 | Animation |  |

## Configuration fields

| Name | Owner | Kind | Value |
|---|---|---|---|
| EditorMultiline | control | Editor | bool |
| AnimationIntervalMs | control | Animation | u32 |
| HistoryFlowBoundary | root | LegacyHistoryUnit | flow_boundary |
| HistoryUnitIdentity | root | LegacyHistoryUnit | unit_identity |

## Opcodes

| Name | Code | Section | Operands |
|---|---:|---|---|
| CreateNode | 0x01 | structure | local_ordinal, host_kind |
| CreateRoot | 0x02 | structure | local_ordinal, root_role, owner_handle_or_null, config_metadata |
| InsertBefore | 0x03 | structure | parent_handle, child_handle, before_handle_or_null |
| Detach | 0x04 | structure | parent_handle, child_handle |
| RetireSubtree | 0x05 | structure | root_handle |
| AttachPort | 0x06 | structure | node_handle, port_handle_or_null |
| AttachControl | 0x07 | structure | node_handle, control_handle_or_null |
| CreateControl | 0x08 | structure | local_ordinal, control_kind, ownership_mode, owner_handle_or_null, config_metadata |
| DisposeControl | 0x09 | structure | control_handle |
| HistoryAction | 0x0a | structure | unit_root_handle, action_id |
| RetireRoot | 0x0b | structure | root_handle |
| SetDeclared | 0x10 | state | node_handle, property_id, typed_value |
| ResetDeclared | 0x11 | state | node_handle, property_id |
| SetOverride | 0x12 | state | node_handle, property_id, typed_value |
| ClearOverride | 0x13 | state | node_handle, property_id |
| SetHidden | 0x14 | state | node_handle, boolean |
| SetStyleState | 0x15 | state | node_handle, layer, key_metadata, typed_value |
| ClearStyleState | 0x16 | state | node_handle, layer, key_metadata |
| ControlCommand | 0x18 | state | control_handle, command_id, typed_operands |
| CreatePort | 0x20 | content_control | local_ordinal, family, ownership_mode, owner_handle_or_null |
| CreateConnector | 0x21 | content_control | local_ordinal, source_index, port_handle, funnel_metadata, ownership_mode |
| SelectConnector | 0x22 | content_control | port_handle, connector_handle_or_null |
| DisposeConnector | 0x23 | content_control | connector_handle |
| DisposePort | 0x24 | content_control | port_handle |
| SetLiteralFunnel | 0x25 | content_control | port_handle, funnel_metadata |
| SetSubscriptions | 0x30 | events | node_handle, mask_low, mask_high |
| ReplaceLiteral | 0x40 | content_descriptor | port_handle, content_format, content_bytes, annotations_bytes |
| ReplaceEditorContent | 0x41 | content_descriptor | control_handle, content_bytes, expected_edit_revision |

## Property coverage

| ID | Domain | Name | Value | Legal kinds | Effects | Normalizer | Nullable | Clearable |
|---:|---|---|---|---|---|---|---|---|
| 0x00000101 | geometry | width | Dimension | Box, ContentHost, Editor, Scroll, Animation | LayoutInput, ContentProjection | dimension | false | true |
| 0x00000102 | geometry | height | Dimension | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | dimension | false | true |
| 0x00000103 | geometry | padding | Insets | Box, ContentHost, Editor, Scroll, Animation | LayoutInput, ContentProjection | insets | false | true |
| 0x00000104 | geometry | minWidth | U16 | Box, ContentHost, Editor, Scroll, Animation | LayoutInput, ContentProjection | u16 | true | true |
| 0x00000105 | geometry | maxWidth | U16 | Box, ContentHost, Editor, Scroll, Animation | LayoutInput, ContentProjection | u16 | true | true |
| 0x00000106 | geometry | minHeight | U16 | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | u16 | true | true |
| 0x00000107 | geometry | maxHeight | U16 | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | u16 | true | true |
| 0x00000108 | geometry | gap | U16 | Box, Scroll, Animation | LayoutInput | u16 | false | true |
| 0x00000109 | geometry | alignment | Alignment | Box, Scroll, Animation | LayoutInput | alignment | false | true |
| 0x0000010a | geometry | borderEdges | Edges | Box, ContentHost, Editor, Scroll, Animation | LayoutInput, ContentProjection | border_edges | true | true |
| 0x00000201 | presentation | foreground | Color | Box, ContentHost, Editor, Scroll, Animation | Presentation | color | true | true |
| 0x00000202 | presentation | background | Color | Box, ContentHost, Editor, Scroll, Animation | Presentation | color | true | true |
| 0x00000203 | presentation | borderColor | Color | Box, ContentHost, Editor, Scroll, Animation | Presentation | color | true | true |
| 0x00000204 | presentation | borderStyle | BorderStyle | Box, ContentHost, Editor, Scroll, Animation | Presentation, LayoutInput | border_style | true | true |
| 0x00000205 | presentation | borderGlyphs | Glyphs | Box, ContentHost, Editor, Scroll, Animation | Presentation | glyphs | true | true |
| 0x00000206 | presentation | textAttributes | TextAttributes | Box, ContentHost, Editor, Scroll, Animation | Presentation | text_attributes | false | true |
| 0x00000207 | presentation | style | Style | Box, ContentHost, Editor, Scroll, Animation | Presentation, HostEnvironmentDependent | style | true | true |
| 0x0000010b | geometry | layout | LayoutMode | Box | LayoutInput | layout | false | true |
| 0x0000010c | geometry | display | Display | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | display | false | true |
| 0x0000010d | geometry | direction | Direction | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | direction | false | true |
| 0x0000010e | geometry | flexDirection | FlexDirection | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | flex_direction | false | true |
| 0x0000010f | geometry | flexWrap | FlexWrap | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | flex_wrap | false | true |
| 0x00000110 | geometry | flexGrow | F32 | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | float | false | true |
| 0x00000111 | geometry | flexShrink | F32 | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | float | false | true |
| 0x00000112 | geometry | flexBasis | Dimension | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | dimension | false | true |
| 0x00000113 | geometry | margin | InsetsF32 | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | dimensions | false | true |
| 0x00000114 | geometry | alignItems | AlignmentMode | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | alignment_mode | false | true |
| 0x00000115 | geometry | alignSelf | AlignmentMode | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | alignment_mode | false | true |
| 0x00000116 | geometry | alignContent | AlignmentMode | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | alignment_mode | false | true |
| 0x00000117 | geometry | justifyContent | AlignmentMode | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | alignment_mode | false | true |
| 0x00000118 | geometry | justifyItems | AlignmentMode | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | alignment_mode | false | true |
| 0x00000119 | geometry | justifySelf | AlignmentMode | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | alignment_mode | false | true |
| 0x0000011a | geometry | columnGap | Dimension | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | nonnegative_dimension | false | true |
| 0x0000011b | geometry | rowGap | Dimension | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | nonnegative_dimension | false | true |
| 0x0000011c | geometry | gridTemplateColumns | TrackList | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | track_list | false | true |
| 0x0000011d | geometry | gridTemplateRows | TrackList | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | track_list | false | true |
| 0x0000011e | geometry | gridAutoColumns | TrackList | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | track_list | false | true |
| 0x0000011f | geometry | gridAutoRows | TrackList | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | track_list | false | true |
| 0x00000120 | geometry | gridAutoFlow | GridAutoFlow | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | grid_auto_flow | false | true |
| 0x00000121 | geometry | gridColumn | GridPlacement | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | grid_placement | false | true |
| 0x00000122 | geometry | gridRow | GridPlacement | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | grid_placement | false | true |
| 0x00000123 | geometry | position | Position | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | position | false | true |
| 0x00000124 | geometry | inset | InsetsF32 | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | dimensions | false | true |

