export {
  TuiError,
  asTuiError,
  isTuiCancelledError,
  isTuiError,
  tuiError,
} from "./api/errors.ts";
export type { TuiErrorCategory } from "./api/errors.ts";
export type {
  AnsiColor,
  ColorSpec,
  RgbColor,
  ThemeColor,
  ThemeColorDefault,
  ThemeColorIndexed,
  ThemeColorNamed,
  ThemeColorReference,
} from "./api/presentation/theme.ts";
export type {
  BorderEdges,
  BorderGlyphs,
  BorderSpec,
  BorderStyle,
  StyleSelectorValue,
  StyleSpecValue,
  TextAttribute,
} from "./api/presentation/style.ts";
export type {
  ComponentAdapter,
  ComponentCapabilities,
  ComponentContext,
  ComponentId,
  InteractionResult,
  KeyEvent,
  PasteEvent,
} from "./api/extensions/traits/component.ts";
export type { ComponentHandle, FrameworkHandle, HandleId } from "./api/controls/framework-handle.ts";
export type {
  GridCell,
  GridRow,
  GridSpec,
  GridTrack,
  HorizontalAlign,
  LayoutChild,
  VerticalAlign,
  ViewChildren,
  WrapMode,
} from "./api/view/view.ts";
export type { InsetsValue } from "./api/view/geometry.ts";
export type { HistoryLayout } from "./api/controls/history.ts";
export type { Output } from "./api/controls/output.ts";
export type { OutputEvent, TerminateEvent, TuiEvent } from "./runtime/events.ts";
export type { Projector } from "./api/extensions/traits/projector.ts";
export type { RenderContext, Renderer } from "./api/extensions/traits/renderer.ts";
export type { SceneProducer } from "./api/view/scene.ts";
export type { StreamAnnotation, StreamSegmentSnapshot, StreamSnapshot } from "./api/content/stream-snapshot.ts";
export type { TextPart, TextRole, TextSelectorValue, TextSpanValue } from "./api/content/text.ts";
export type { TextInputOptions } from "./api/controls/text-input.ts";
export type { TextStreamOptions, TextStreamPacing, TextStreamPresentation } from "./api/controls/text-stream.ts";
export type { StreamingSource } from "./api/extensions/traits/streaming-source.ts";
export type { TextRewriter } from "./api/extensions/traits/text-rewriter.ts";
export type { TextVisitor } from "./api/extensions/traits/text-visitor.ts";
export type { TerminalMetadata, TuiOpenOptions, TuiRuntime } from "./runtime/runtime.ts";
export type { ViewComponent } from "./composition/define-view.ts";
export type { ViewSlot } from "./api/controls/view-slot.ts";
export type { ScrollPane } from "./api/controls/scroll-pane.ts";
export { View, ChildrenBuilder, GridBuilder, GridRowBuilder } from "./api/view/view.ts";
export type { OverflowIndicator } from "./api/view/view.ts";
export { defineView } from "./composition/define-view.ts";
export { state } from "./composition/tracked-state.ts";
export type { State } from "./composition/tracked-state.ts";
export { Insets } from "./api/view/geometry.ts";
export {
  Style,
  StyleRef,
  StyleSelector,
  StyleSpec,
  StyleStateKey,
  StyleStateValue,
} from "./api/presentation/style.ts";
export { TextSelector, TextSpan } from "./api/content/text.ts";
export { History } from "./api/controls/history.ts";
export { TextInput } from "./api/controls/text-input.ts";
export { TextStream } from "./api/controls/text-stream.ts";
export { TextContent, RawText } from "./api/content/text-content.ts";
export { Annotations } from "./api/content/annotations.ts";
export { Projection, ProjectionBuilder, Smooth } from "./api/content/projection.ts";
export { DiffRange, DiffLine, DiffHunk, DiffRenderer } from "./api/content/diff.ts";
export { Theme, ThemeKey, themeColor } from "./api/presentation/theme.ts";
export type { DiffLineKind, DiffLineTermination } from "./api/content/diff.ts";
export type { ProjectionSpan } from "./api/content/projection.ts";
export type { SemanticTag, SemanticValue } from "./api/content/annotations.ts";
export type { TextFormat, TextOrigin } from "./api/content/text-content.ts";
export { PlainTextProjector, MarkdownProjector } from "./api/content/projectors.ts";
export { Scene } from "./api/view/scene.ts";
export { Tui } from "./runtime/runtime.ts";
