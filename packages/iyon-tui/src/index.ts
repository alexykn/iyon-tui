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
export { Theme, ThemeKey, themeColor } from "./api/presentation/theme.ts";
export type {
	BorderEdges,
	BorderGlyphs,
	BorderSpec,
	BorderStyle,
	StyleSelectorValue,
	StyleSpecValue,
	TextAttribute,
} from "./api/presentation/style.ts";
export {
	Style,
	StyleRef,
	StyleSelector,
	StyleSpec,
	StyleStateKey,
	StyleStateValue,
} from "./api/presentation/style.ts";
export { Insets } from "./api/presentation/geometry.ts";
export type { InsetsValue } from "./api/presentation/geometry.ts";
export type {
	ContentConnectorError,
	ContentConnectorPhase,
	ContentConnectorStatus,
	ContentFamily,
	ContentPortOptions,
	ContentSource,
	Funnel,
	Source,
	TextFunnelDelivery,
	TextFunnelKind,
	TextFunnelOptions,
	TextFunnelWrap,
	TextSmoothOptions,
	TextRetentionPolicy,
	TextSourceAnnotation,
	TextSourceAnnotationKind,
	TextSourceAnnotationSnapshot,
	TextSourceMutation,
	TextSourceOptions,
	TextSourceSnapshot,
	TextSourceStats,
} from "./api/content/retained.ts";
export type { ContentPort, ContentConnector } from "./api/content/explicit.ts";
export {
	TextBlockSource,
	TextFunnel,
	TextStreamSource,
} from "./api/content/retained.ts";
export { TextContent, RawText } from "./api/content/text-content.ts";
export { TextSelector, TextSpan } from "./api/content/text.ts";
export type {
	TextAnnotation,
	TextPart,
	TextRole,
	TextSelectorValue,
	TextSpanValue,
} from "./api/content/text.ts";
export type { TextFormat, TextOrigin } from "./api/content/text-content.ts";
export type {
	TerminalMetadata,
	TuiOpenOptions,
	TuiRuntime,
} from "./runtime/runtime.ts";
export { Tui } from "./runtime/runtime.ts";
export type {
	OutputEvent,
	TerminateEvent,
	TuiEvent,
} from "./runtime/events.ts";
export {
	Annotations,
	TEXT_SOURCE_ANNOTATION_SCHEMA,
} from "./api/content/annotations.ts";
export type {
	SemanticTag,
	SemanticTextStyle,
	SemanticValue,
} from "./api/content/annotations.ts";
