<!-- DO NOT EDIT. Generated from tools/tui-abi/ui_abi.toml. schema_blake3 = 8ff1e9e2b87f0439ccdbd9f487068d190302196bd8545dd90d1cd4dc2f9daee0; generator_blake3 = da9e330405afd1424c48eada9c868113caf08ee8009f434e893a0f6f028b4ba3 -->

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
| 0x00000101 | geometry | width | SizeMode | Box, ContentHost, Editor, Scroll, Animation | LayoutInput, ContentProjection | size_mode | false | true |
| 0x00000102 | geometry | height | SizeMode | Box, ContentHost, Editor, Scroll, Animation | LayoutInput | size_mode | false | true |
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

