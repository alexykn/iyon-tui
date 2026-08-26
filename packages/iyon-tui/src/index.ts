export {
  TuiError,
  asTuiError,
  isTuiCancelledError,
  isTuiError,
  tuiError,
} from "./errors.ts";
export type {
  AnsiColor,
  BorderEdges,
  BorderGlyphs,
  BorderSpec,
  BorderStyle,
  ColorSpec,
  RgbColor,
  ThemeColor,
  ThemeColorDefault,
  ThemeColorIndexed,
  ThemeColorNamed,
  ThemeColorReference,
  ComponentAdapter,
  ComponentCapabilities,
  ComponentContext,
  GridCell,
  GridRow,
  GridSpec,
  GridTrack,
  HorizontalAlign,
  InsetsValue,
  InteractionResult,
  LayoutChild,
  HistoryLayout,
  KeyEvent,
  NativeHandle,
  NativeHandleId,
  OutputEvent,
  Output,
  OutputHandle,
  PasteEvent,
  Projector,
  RenderContext,
  Renderer,
  ResizeEvent,
  SceneProducer,
  StreamAnnotation,
  StreamSegmentSnapshot,
  StreamSnapshot,
  TextInputOptions,
  TextSelectorValue,
  TextSpanValue,
  StyleSelectorValue,
  StyleSpecValue,
  TextStreamOptions,
  TextStreamPacing,
  TextStreamPresentation,
  StreamingSource,
  TerminateEvent,
  TextRewriter,
  TextVisitor,
  TerminalMetadata,
  TuiEvent,
  ThemeColorEntry,
  ThemeDefinition,
  ThemeStyleEntry,
  TuiOpenOptions,
  TuiOperation,
  TuiRuntime,
  VerticalAlign,
  WrapMode,
  TuiFailure,
  ViewSlot,
  ScrollPane,
} from "./types.ts";
export { View, ChildrenBuilder, GridBuilder, GridRowBuilder } from "./values/view.ts";
export type { OverflowIndicator, ViewChildren } from "./values/view.ts";
export { defineView } from "./define-view.ts";
export type { ViewComponent } from "./execution.ts";
export { state } from "./tracked-state.ts";
export type { State } from "./tracked-state.ts";
export { Insets } from "./values/geometry.ts";
export {
  Style,
  StyleRef,
  StyleSelector,
  StyleSpec,
  StyleStateKey,
  StyleStateValue,
} from "./values/style.ts";
export { TextSelector, TextSpan } from "./values/text.ts";
export { History } from "./history.ts";
export { TextInput, NativeOutputHandle } from "./text-input.ts";
export { TextStream, StreamPane } from "./stream.ts";
export { Component, ViewSlot as NativeViewSlot } from "./component.ts";
export { NativeScrollPane } from "./scroll-pane.ts";
export { TextContent, RawText } from "./values/text-content.ts";
export { Annotations } from "./values/annotations.ts";
export { Projection, ProjectionBuilder, Smooth } from "./values/projection.ts";
export { DiffRange, DiffLine, DiffHunk, DiffRenderer } from "./values/diff.ts";
export { Theme, ThemeKey } from "./values/theme.ts";
export type { DiffLineKind, DiffLineTermination } from "./values/diff.ts";
export type { ProjectionSpan } from "./values/projection.ts";
export type { SemanticTag, SemanticValue } from "./values/annotations.ts";
export type { TextFormat, TextOrigin } from "./values/text-content.ts";
export { PlainTextProjector, MarkdownProjector } from "./projectors.ts";
export { RendererAdapter } from "./traits/renderer.ts";
export { ProjectorAdapter } from "./traits/projector.ts";
export { TextVisitorAdapter } from "./traits/text-visitor.ts";
export { TextRewriterAdapter } from "./traits/text-rewriter.ts";
export { StreamingSourceAdapter } from "./traits/streaming-source.ts";
export { ComponentAdapterBridge } from "./traits/component.ts";
export { OutputRouter, RouteConflict } from "./output.ts";
export { FocusController, InteractionRouter } from "./interaction.ts";
export { Scene } from "./scene.ts";
export { Tui } from "./runtime.ts";
export { keyEvent, pasteEvent, resizeEvent, terminateEvent } from "./events.ts";
export { AppHarness, createAppHarness } from "./testing.ts";

export const tuiSmoke = "iyon:tui/t1" as const;
