import { asTuiError, tuiError } from "./api/errors.ts";
import {
  disposeFrameworkResource,
  nativeResourceOf,
  registerFrameworkHandle,
} from "./handle-registry.ts";
import type { View as SemanticView } from "./api/view/view.ts";
import type { TextContent as SemanticTextContent } from "./api/content/text-content.ts";
import type { StyleRef } from "./api/presentation/style.ts";
import type { Theme as SemanticTheme } from "./api/presentation/theme.ts";
import type { ThemeKey } from "./api/presentation/theme-key.ts";

declare const handleIdBrand: unique symbol;
/** JavaScript-local framework handle identity; this is not a native identifier. */
export type HandleId = number & { readonly [handleIdBrand]: "HandleId" };

declare const componentIdBrand: unique symbol;
/** Native host identity for a mounted component, distinct from a JS handle id. */
export type ComponentId = number & { readonly [componentIdBrand]: "ComponentId" };

/**
 * Nominal base for framework-owned handles. Native resources and lifecycle
 * state are kept in private registries; consumers can name the handle
 * contract without seeing or supplying a native object.
 */
export abstract class FrameworkHandle<K extends string = string> {
  #frameworkHandleBrand!: void;
  readonly id: HandleId;
  readonly kind: K;
  private isDisposed = false;

  protected constructor(kind: K, resource: never) {
    this.kind = kind;
    this.id = registerFrameworkHandle(this, resource as unknown as object);
  }

  protected nativeAs<T extends object>(): T {
    return nativeResourceOf<T>(this);
  }

  get disposed(): boolean { return this.isDisposed; }

  dispose(): void {
    if (this.isDisposed) return;
    this.isDisposed = true;
    disposeFrameworkResource(this);
  }

  protected ensureOpen(): void {
    if (this.isDisposed) throw tuiError("disposed-handle", `${this.kind} handle has been disposed`, { id: this.id });
  }

  protected call<R>(operation: () => R): R {
    try {
      this.ensureOpen();
      return operation();
    } catch (error) {
      throw asTuiError(error);
    }
  }
}

export type View = SemanticView;

/** Stable identity token for a retained user-defined View component. */
export interface ViewComponentType<P = unknown> {
  readonly render: (props: P) => View;
}

/** Callable retained View component returned by defineView. */
export interface ViewComponent<P = unknown> {
  readonly render: (props: P) => View;
  (props: P): View;
}

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
export interface BorderGlyphs {
  readonly top: string;
  readonly right: string;
  readonly bottom: string;
  readonly left: string;
  readonly topLeft: string;
  readonly topRight: string;
  readonly bottomLeft: string;
  readonly bottomRight: string;
}

/** Public border semantics; the native glyph/layout record is private. */
export interface BorderSpec {
  readonly glyphs?: BorderGlyphs;
  readonly style?: BorderStyle;
  readonly edges?: BorderEdges;
  readonly color?: ColorSpec;
}

/** Text attributes supported by the native semantic style model. */
export type TextAttribute =
  | "bold"
  | "dim"
  | "italic"
  | "underline"
  | "reversed"
  | "strikethrough";

/** Sparse direct style data. Named-style identity belongs to StyleRef. */
export interface StyleSpecValue {
  readonly foreground?: ColorSpec;
  readonly background?: ColorSpec;
  readonly attributes: Readonly<Partial<Record<TextAttribute, boolean>>>;
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

export type TextRole =
  | "paragraph"
  | "heading"
  | "blockQuote"
  | "list"
  | "listItem"
  | "codeBlock"
  | "table"
  | "tableRow"
  | "tableCell"
  | "thematicBreak"
  | "rawBlock"
  | "container"
  | "strong"
  | "emphasis"
  | "strikethrough"
  | "underline"
  | "superscript"
  | "subscript"
  | "smallCaps"
  | "inlineCode"
  | "link"
  | "image"
  | "rawInline";

export type TextPart =
  | "listMarker"
  | "taskMarker"
  | "quoteMarker"
  | "codeLabel"
  | "tableRule"
  | "thematicRule"
  | "imageFallback";

export interface TextSelectorValue {
  readonly focused?: boolean;
  readonly focusWithin?: boolean;
  readonly states?: Readonly<Record<string, string>>;
  readonly roles?: readonly TextRole[];
  readonly parts?: readonly TextPart[];
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

export type TextContent = SemanticTextContent;

export interface History extends FrameworkHandle<"history"> {
  readonly kind: "history";
  layout(): HistoryLayout;
  push(view: View): number;
  freeze(unit: number, view: View): void;
  discardLive(unit: number): void;
  pushStream(stream: TextStream): void;
  sealStream(stream: TextStream): void;
  setLayout(layout: HistoryLayout): void;
}

export interface HistoryLayout {
  readonly padding: number;
  readonly gap: number;
}

export interface TextInput extends ComponentHandle {
  readonly kind: "text-input";
  text(): string;
  cursorBytes(): number;
  setText(value: string): void;
  clear(): void;
  submitted(): Output<string>;
  setMultiline(enabled: boolean): void;
  isMultiline(): boolean;
  view(): View;
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

export interface TextStream extends FrameworkHandle<"text-stream"> {
  readonly kind: "text-stream";
  update(text: string): void;
  append(text: string, annotations?: readonly StreamAnnotation[]): void;
  seal(): void;
  snapshot(): StreamSnapshot;
}

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

/**
 * Opaque framework-owned identity that may occupy one View component node.
 * This is a mounted handle, not the user-implementable ComponentAdapter
 * behavior contract; use a control's `view()` projection for composition.
 */
export interface ComponentHandle extends FrameworkHandle<"component" | "text-input"> {
  readonly kind: "component" | "text-input";
  view(): View;
}

export interface ViewSlot extends ComponentHandle {
  readonly kind: "component";
  capabilities(): ComponentCapabilities;
  setView(view: View | (() => View)): void;
  setAnimation(frames: readonly View[], intervalMs: number): void;
  setAnimationAtCycleBoundary(frames: readonly View[], intervalMs: number): void;
  stopAnimation(view: View): void;
  revision(): number;
}

export interface ScrollPane extends ComponentHandle {
  readonly kind: "component";
  capabilities(): ComponentCapabilities;
  setContent(view: View | (() => View)): void;
  followEnd(): void;
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

/**
 * Borrow-scoped component event context. Outputs are typed channel identities;
 * their payload is supplied separately, matching the native EventCx contract.
 */
export interface ComponentContext {
  readonly componentId: ComponentId;
  emit<T>(output: Output<T>, payload: T): void;
}

/** Result of a generic component interaction; emitted values use ComponentContext.emit(). */
export type InteractionResult =
  | { readonly type: "handled" }
  | { readonly type: "ignored" };

/** Opaque typed output-channel identity; payloads are delivered separately. */
export class Output<T> {
  #outputBrand!: void;
  /** Type-only variance marker keeps channels for different payloads distinct. */
  declare private readonly outputType: (value: T) => T;
  readonly kind = "output" as const;
  private constructor() {}
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

/**
 * Structural scene root accepted by the runtime.
 *
 * A concrete `Scene` value is convenient for callers that want an explicit
 * root, while any object with this shape is also accepted at the boundary.
 */
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

/** Current terminal dimensions reported by a runtime. */
export interface TerminalMetadata {
  readonly width: number;
  readonly height: number;
}

export interface TuiRuntime {
  /** The current terminal size; it changes only after a successful resize. */
  readonly size: TerminalMetadata;
  nextEvent(signal?: AbortSignal): Promise<TuiEvent>;
  /**
   * Render either a structural scene value or a retained scene producer.
   * Direct values take over the root immediately; producers own the retained
   * root and remain subscribed to tracked state. These paths are distinct.
   */
  render(scene: SceneProducer, signal?: AbortSignal): void;
  resize(width: number, height: number): void;
  close(): void;
  exit(): void;
  createHistory(): History;
  createTextInput(options?: TextInputOptions): TextInput;
  createViewSlot(initial: View): ViewSlot;
  createScrollPane(initial: View): ScrollPane;
  bindKey(key: string, routeId: string, modifiers?: readonly string[]): void;
  route(output: Output<string>, routeId: string): void;
  interceptPaste(input: TextInput, routeId: string): void;
  forwardPaste(text: string): void;
  setTheme(theme: SemanticTheme): void;
}

export interface AppHarness extends TuiRuntime {
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
  /** Current deterministic harness clock in milliseconds. */
  now(): number;
}
