export {
  TuiError,
  asTuiError,
  isTuiCancelledError,
  isTuiError,
  tuiError,
} from "./errors.ts";
export type {
  ComponentAdapter,
  ComponentCapabilities,
  ComponentContext,
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
  StreamAnnotation,
  StreamSnapshot,
  TextStreamOptions,
  TextStreamPacing,
  TextStreamPresentation,
  StreamingSource,
  TerminateEvent,
  TextRewriter,
  TextVisitor,
  TuiEvent,
  TuiOpenOptions,
  TuiOperation,
  TuiRuntime,
  TuiFailure,
  ViewSlot,
  ScrollPane,
} from "./types.ts";
export { View, ChildrenBuilder } from "./values/view.ts";
export { defineView } from "./define-view.ts";
export type { ViewComponent } from "./execution.ts";
export { state } from "./tracked-state.ts";
export type { State } from "./tracked-state.ts";
export { Insets } from "./values/geometry.ts";
export { Style, StyleSpec } from "./values/style.ts";
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
export type { ThemeSelector } from "./values/theme.ts";
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

export const tuiSmoke = "iyon:tui/t5" as const;
