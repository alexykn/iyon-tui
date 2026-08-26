import type { TuiError } from "./errors.ts";
import type { View as SemanticView } from "./values/view.ts";
import type { TextContent as SemanticTextContent } from "./values/text-content.ts";
import type { StyleRef } from "./values/style.ts";
import type { Theme as SemanticTheme } from "./values/theme.ts";
import type { ThemeKey } from "./values/theme-key.ts";

declare const handleBrand: unique symbol;
export type NativeHandleId = number & { readonly [handleBrand]: "NativeHandleId" };

export interface NativeHandle {
  readonly kind: string;
  readonly id: NativeHandleId;
  readonly disposed: boolean;
  dispose(): void;
}

export type TuiOperation<T> = T;

export type View = SemanticView;

/** Public geometry value used by semantic View construction. */
export interface InsetsValue {
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
  readonly left: number;
}

/** Named ANSI colors supported by terminal backends. */
export type AnsiColor =
  | "black"
  | "red"
  | "green"
  | "yellow"
  | "blue"
  | "magenta"
  | "cyan"
  | "gray"
  | "darkGray"
  | "lightRed"
  | "lightGreen"
  | "lightYellow"
  | "lightBlue"
  | "lightMagenta"
  | "lightCyan"
  | "white";

/** A resolved color value stored in a Theme definition. */
export interface ThemeColorDefault {
  readonly type: "default";
}

export interface ThemeColorNamed {
  readonly type: "named";
  readonly value: AnsiColor;
}

export interface ThemeColorIndexed {
  readonly type: "indexed";
  readonly value: number;
}

export interface RgbColor {
  readonly type: "rgb";
  readonly r: number;
  readonly g: number;
  readonly b: number;
}

export type ThemeColor = ThemeColorDefault | ThemeColorNamed | ThemeColorIndexed | RgbColor;

/** A semantic reference resolved by the active Theme. */
export interface ThemeColorReference {
  readonly type: "theme";
  readonly key: string | ThemeKey;
}

/** Explicit theme reference, named ANSI, indexed ANSI, or RGB color. */
export type ColorSpec = ThemeColorReference | ThemeColorNamed | ThemeColorIndexed | RgbColor;

export type BorderStyle = "plain" | "rounded" | "double";
export type BorderEdges = "all" | "topBottom";
export type BorderGlyphs = Readonly<Record<string, string>>;

/** Public border semantics; the native glyph/layout record is private. */
export interface BorderSpec {
  readonly glyphs?: BorderGlyphs;
  readonly style?: BorderStyle;
  readonly edges?: BorderEdges;
  readonly color?: ColorSpec;
}

/** Sparse direct style data. Named-style identity belongs to StyleRef. */
export interface StyleSpecValue {
  readonly foreground?: ColorSpec;
  readonly background?: ColorSpec;
  readonly attributes: Readonly<Record<string, boolean>>;
}

export type VerticalAlign = "top" | "center" | "bottom";
export type HorizontalAlign = "start" | "center" | "end";
export type WrapMode = "wordThenGrapheme" | "grapheme" | "noWrap";

export type LayoutChild =
  | { readonly kind: "normal"; readonly child: View }
  | { readonly kind: "fixed"; readonly size: number; readonly child: View }
  | { readonly kind: "flex"; readonly child: View }
  | { readonly kind: "flexMax"; readonly maxRows: number; readonly child: View }
  | { readonly kind: "contentMax"; readonly maxRows: number; readonly child: View };

export type GridTrack =
  | { readonly kind: "content" }
  | { readonly kind: "contentMax"; readonly max: number }
  | { readonly kind: "fixed"; readonly size: number }
  | { readonly kind: "flex" }
  | { readonly kind: "flexMax"; readonly max: number };

export interface GridCell {
  readonly view: View;
  readonly columnSpan?: number;
  readonly rowSpan?: number;
  readonly horizontalAlign?: HorizontalAlign;
  readonly verticalAlign?: VerticalAlign;
}

export interface GridRow {
  readonly track?: GridTrack;
  readonly cells: readonly GridCell[];
}

export interface GridSpec {
  readonly columns?: readonly GridTrack[];
  readonly rows: readonly GridRow[];
  readonly columnGap?: number;
  readonly rowGap?: number;
}

export interface TextSelectorValue {
  readonly focused?: boolean;
  readonly focusWithin?: boolean;
  readonly states?: Readonly<Record<string, string>>;
  readonly roles?: readonly string[];
  readonly parts?: readonly string[];
  readonly annotations?: readonly StreamAnnotation[];
  readonly language?: string;
  readonly origin?: string;
  readonly format?: string;
}

export interface TextSpanValue {
  readonly text: string;
  readonly style?: StyleRef;
}

/** Structural selector value used when a Theme crosses the host boundary. */
export interface StyleSelectorValue {
  readonly focused?: boolean;
  readonly focusWithin?: boolean;
  readonly states?: Readonly<Record<string, string>>;
}

export interface TextInputOptions {
  readonly multiline?: boolean;
  readonly border?: BorderSpec;
}

export interface ThemeStyleEntry {
  readonly base?: StyleSpecValue;
  readonly variants: readonly { readonly selector: StyleSelectorValue; readonly value: StyleSpecValue }[];
}

export interface ThemeColorEntry {
  readonly base?: ThemeColor;
  readonly variants: readonly { readonly selector: StyleSelectorValue; readonly value: ThemeColor }[];
}

/** Semantic theme definition used by the host boundary and its private lowering. */
export interface ThemeDefinition {
  readonly styles: Readonly<Record<string, ThemeStyleEntry>>;
  readonly colors: Readonly<Record<string, ThemeColorEntry>>;
  readonly textStyles: readonly { readonly selector: TextSelectorValue; readonly value: StyleSpecValue }[];
}

export type TextContent = SemanticTextContent;

export interface History extends NativeHandle {
  readonly kind: "history";
  layout(): TuiOperation<HistoryLayout>;
  push(view: View): TuiOperation<number>;
  freeze(unit: number, view: View): TuiOperation<void>;
  discardLive(unit: number): TuiOperation<void>;
  pushStream(stream: TextStream): TuiOperation<void>;
  sealStream(stream: TextStream): TuiOperation<void>;
  setLayout?(layout: HistoryLayout): TuiOperation<void>;
}

export interface HistoryLayout {
  readonly padding: number;
  readonly gap: number;
}

export interface TextInput extends NativeHandle {
  readonly kind: "text-input";
  text(): TuiOperation<string>;
  cursorBytes(): TuiOperation<number>;
  setText(value: string): TuiOperation<void>;
  clear(): TuiOperation<void>;
  submitted(): TuiOperation<OutputHandle<string>>;
  setMultiline(enabled: boolean): TuiOperation<void>;
  isMultiline(): TuiOperation<boolean>;
  view(): TuiOperation<View>;
}

export interface TextStreamOptions {
  readonly projector?: "markdown";
  readonly presentation?: TextStreamPresentation;
  readonly pacing?: TextStreamPacing;
}

export interface TextStreamPresentation {
  readonly insets?: { readonly top?: number; readonly right?: number; readonly bottom?: number; readonly left?: number };
}

export interface TextStreamPacing {
  readonly tickIntervalMs?: number;
  readonly spring?: number;
  readonly minUnitsPerSecond?: number;
  readonly maxUnitsPerSecond?: number;
}

export interface TextStream extends NativeHandle {
  readonly kind: "text-stream";
  update(text: string): TuiOperation<void>;
  append(text: string, annotations?: readonly StreamAnnotation[]): TuiOperation<void>;
  seal(): TuiOperation<void>;
  snapshot(): TuiOperation<StreamSnapshot>;
}

export type StreamPane = TextStream;

export interface StreamAnnotation {
  readonly namespace: string;
  readonly name: string;
}

export interface StreamSnapshot {
  readonly text: string;
  readonly revision: number;
  readonly sealed: boolean;
  readonly segments?: readonly StreamSegmentSnapshot[];
}

export interface StreamSegmentSnapshot {
  readonly annotations: readonly StreamAnnotation[];
  readonly text: string;
}

export interface Component extends NativeHandle {
  readonly kind: "component";
  view(): TuiOperation<View>;
  capabilities(): TuiOperation<ComponentCapabilities>;
}

export interface ViewSlot extends Component {
  setView(view: View | (() => View)): TuiOperation<void>;
  setAnimation(frames: readonly View[], intervalMs: number): TuiOperation<void>;
  setAnimationAtCycleBoundary(frames: readonly View[], intervalMs: number): TuiOperation<void>;
  stopAnimation(view: View): TuiOperation<void>;
  revision(): TuiOperation<number>;
}

export interface ScrollPane extends Component {
  setContent(view: View): TuiOperation<void>;
  followEnd(): TuiOperation<void>;
}

export interface ComponentCapabilities {
  readonly focusable?: boolean;
  readonly modal?: boolean;
  readonly keys?: readonly string[];
  readonly paste?: boolean;
  readonly ticks?: boolean;
}

export interface Renderer {
  render(view: View, context?: RenderContext): View | Promise<View>;
}

export interface Projector {
  project(content: TextContent): TextContent | Promise<TextContent>;
}

export interface TextVisitor {
  visit(content: TextContent): void | Promise<void>;
}

export interface TextRewriter {
  rewrite(content: TextContent): TextContent | Promise<TextContent>;
}

export interface StreamingSource {
  snapshot(): StreamSnapshot | Promise<StreamSnapshot>;
  advance(): boolean | Promise<boolean>;
  seal(): void | Promise<void>;
  compact?(): void | Promise<void>;
}

export interface ComponentAdapter {
  view(context: ComponentContext): View | Promise<View>;
  capabilities?(context: ComponentContext): ComponentCapabilities | Promise<ComponentCapabilities>;
  onKey?(event: KeyEvent, context: ComponentContext): InteractionResult | Promise<InteractionResult>;
  onPaste?(event: PasteEvent, context: ComponentContext): InteractionResult | Promise<InteractionResult>;
  onTick?(context: ComponentContext): InteractionResult | Promise<InteractionResult>;
}

export interface ComponentContext {
  readonly componentId: NativeHandleId;
  emit(output: Output): void;
}

export type InteractionResult =
  | { readonly type: "handled" }
  | { readonly type: "ignored" }
  | { readonly type: "output"; readonly output: Output };

export type Output = Readonly<Record<string, unknown>>;

export interface OutputHandle<T> {
  readonly kind: "output";
  readonly payload: T;
}

export interface KeyEvent {
  readonly type: "key";
  readonly key: string;
  readonly modifiers?: readonly string[];
}

export interface PasteEvent {
  readonly type: "paste";
  readonly text: string;
}

export interface ResizeEvent {
  readonly type: "resize";
  readonly width: number;
  readonly height: number;
}

export interface TerminateEvent {
  readonly type: "terminate";
  readonly reason?: string;
}

export interface OutputEvent {
  readonly type: "output";
  readonly routeId: string;
  readonly payload?: string;
}

export type TuiEvent = OutputEvent | TerminateEvent;

export interface RenderContext {
  readonly width: number;
  readonly height: number;
}

export interface Scene {
  readonly history?: History;
  readonly body: View;
}

/** A scene value or a producer closure evaluated inside the retained root scope. */
export type SceneProducer = Scene | (() => Scene);

export interface TuiOpenOptions {
  readonly width?: number;
  readonly height?: number;
  readonly headless?: boolean;
  readonly signal?: AbortSignal;
  readonly theme?: SemanticTheme;
}

export interface TerminalMetadata {
  readonly width: number;
  readonly height: number;
}

export interface TuiRuntime {
  readonly size: TuiOperation<TerminalMetadata>;
  nextEvent(signal?: AbortSignal): Promise<TuiEvent>;
  render(scene: SceneProducer, signal?: AbortSignal): TuiOperation<void>;
  resize(width: number, height: number): TuiOperation<void>;
  close(): TuiOperation<void>;
  exit(): TuiOperation<void>;
  createHistory?(): History;
  createTextInput?(options?: TextInputOptions): TextInput;
  createViewSlot?(initial: View): ViewSlot;
  createScrollPane?(initial: View): ScrollPane;
  bindKey(key: string, routeId: string, modifiers?: readonly string[]): void;
  route(output: OutputHandle<string>, routeId: string): void;
  interceptPaste?(input: TextInput, routeId: string): void;
  forwardPaste?(text: string): void;
  setTheme?(theme: SemanticTheme): TuiOperation<void>;
}

export interface AppHarness extends TuiRuntime {
  nextAction(signal?: AbortSignal): Promise<{ actionId: string; payload?: string } | null>;
  createViewSlot(initial: View): ViewSlot;
  createScrollPane(initial: View): ScrollPane;
  pressKey(key: string, modifiers?: readonly string[]): void;
  paste(text: string): void;
  advance(ms: number): void;
  screenRows(): readonly string[];
  nativeHistoryRows(): readonly string[];
  styleAt(row: number, column: number): Readonly<Record<string, unknown>>;
  cellXOfText(row: number, text: string): number | null;
  exited(): boolean;
}

export type TuiFailure = TuiError;
