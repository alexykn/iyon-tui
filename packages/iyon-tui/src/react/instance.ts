import type { ReactNode, Ref } from "react";
import type {
	ContentConnector as ExplicitContentConnector,
	ContentPort as ExplicitContentPort,
} from "../api/content/explicit.ts";
import {
	type ContentSource,
	type Funnel,
	TextBlockSource,
	TextFunnel,
	TextStreamSource,
} from "../api/content/retained.ts";
import {
	type Insets,
	type InsetsValue,
	insets,
} from "../api/presentation/geometry.ts";
import {
	semanticColorFor,
	semanticStyleFor,
} from "../api/presentation/semantic-style.ts";
import {
	type BorderEdges,
	type BorderGlyphs,
	type BorderStyle,
	StyleRef,
	StyleSpec,
	type StyleSpecValue,
	type TextAttribute,
	validateTextAttribute,
} from "../api/presentation/style.ts";
import type { ColorSpec } from "../api/presentation/theme.ts";
import {
	HOST_KINDS,
	type HostKind,
	UI_PROPERTIES,
	UI_PROPERTY_DESCRIPTORS,
	type UiPropertyId,
	type UiPropertyName,
	uiPropertyDescriptorByName,
	uiValueEncoding,
} from "../transport/ui/generated/ui_schema.ts";
import type { RootContainer } from "./commit.ts";
import {
	isExplicitContentConnector,
	isExplicitContentPort,
} from "./resources.ts";

export type HostType =
	| "box"
	| "row"
	| "column"
	| "grid"
	| "portal"
	| "content"
	| "text"
	| "editor"
	| "scroll"
	| "animation"
	| "historyUnit";

export type HistoryFlowBoundary = "default" | "attachToPrevious";
export type HistoryUnitAction = "live" | "freeze";

export type LayoutKind = "box" | "row" | "column" | "grid";

/** Finite logical geometry values. Percent values are fractions in [0, 1]. */
export interface DimensionValue {
	readonly unit: "length" | "percent";
	readonly value: number;
}
export type DimensionInput = DimensionValue | "auto" | number;
export type DimensionInsetsInput =
	| DimensionInput
	| {
			readonly top?: DimensionInput;
			readonly right?: DimensionInput;
			readonly bottom?: DimensionInput;
			readonly left?: DimensionInput;
	  };
export type GridTrack =
	| "auto"
	| "minContent"
	| "maxContent"
	| {
			readonly type: "length" | "percent" | "fr";
			readonly value: number;
	  }
	| {
			readonly type: "minmax";
			readonly min: GridTrackMinBound;
			readonly max: GridTrackMaxBound;
	  };
export type GridTrackMinBound =
	| "auto"
	| "minContent"
	| "maxContent"
	| { readonly type: "length" | "percent"; readonly value: number };
export type GridTrackMaxBound =
	| "auto"
	| "minContent"
	| "maxContent"
	| { readonly type: "length" | "percent" | "fr"; readonly value: number };
export interface GridPlacement {
	readonly start?: number | "auto" | { readonly span: number };
	readonly end?: number | "auto" | { readonly span: number };
}

export interface LayoutProps {
	readonly width?: "fit" | "fill" | DimensionValue | number;
	readonly height?: "fit" | "fill" | DimensionValue | number;
	readonly padding?: number | Insets | InsetsValue;
	readonly minWidth?: number;
	readonly maxWidth?: number;
	readonly minHeight?: number;
	readonly maxHeight?: number;
	readonly gap?: number;
	readonly display?: "flex" | "grid" | "none";
	readonly direction?: "ltr" | "rtl";
	readonly flexDirection?: "row" | "column" | "rowReverse" | "columnReverse";
	readonly flexWrap?: "nowrap" | "wrap" | "wrapReverse";
	readonly flexGrow?: number;
	readonly flexShrink?: number;
	readonly flexBasis?: DimensionInput;
	readonly margin?: DimensionInsetsInput;
	readonly alignItems?: AlignmentMode;
	readonly alignSelf?: AlignmentMode;
	readonly alignContent?: AlignmentMode;
	readonly justifyContent?: AlignmentMode;
	readonly justifyItems?: AlignmentMode;
	readonly justifySelf?: AlignmentMode;
	readonly columnGap?: DimensionInput;
	readonly rowGap?: DimensionInput;
	readonly gridTemplateColumns?: readonly GridTrack[];
	readonly gridTemplateRows?: readonly GridTrack[];
	readonly gridAutoColumns?: readonly GridTrack[];
	readonly gridAutoRows?: readonly GridTrack[];
	readonly gridAutoFlow?: "row" | "column" | "rowDense" | "columnDense";
	readonly gridColumn?: GridPlacement;
	readonly gridRow?: GridPlacement;
	readonly position?: "relative" | "absolute";
	readonly inset?: DimensionInsetsInput;
	readonly alignment?: {
		readonly horizontal?: "start" | "center" | "end" | "top" | "bottom";
		readonly vertical?: "start" | "center" | "end" | "top" | "bottom";
	};
	readonly borderEdges?:
		| BorderEdges
		| {
				readonly top?: boolean;
				readonly right?: boolean;
				readonly bottom?: boolean;
				readonly left?: boolean;
		  };
}

export type AlignmentMode =
	| "start"
	| "end"
	| "center"
	| "stretch"
	| "baseline"
	| "spaceBetween"
	| "spaceEvenly"
	| "spaceAround";

export interface UiEventTarget {
	readonly host_namespace: number;
	readonly slot: number;
	readonly generation: number;
	readonly kind: number;
}

export interface UiEventBase {
	readonly target: UiEventTarget;
	readonly revision?: number;
}

export interface UiPressEvent extends UiEventBase {
	readonly type: "press";
	readonly key?: string;
}

export interface UiEditEvent extends UiEventBase {
	readonly type: "input" | "edit" | "change" | "selectionChange" | "submit";
	readonly text: string;
	readonly cursorBytes: number;
	readonly key?: string;
}

export type UiEvent = UiPressEvent | UiEditEvent;
export type UiEventHandler<E extends UiEvent = UiEvent> = (event: E) => void;

export interface EventProps {
	readonly onPress?: UiEventHandler<UiPressEvent> | undefined;
	readonly onInput?: UiEventHandler<UiEditEvent> | undefined;
	readonly onEdit?: UiEventHandler<UiEditEvent> | undefined;
	readonly onChange?: UiEventHandler<UiEditEvent> | undefined;
	readonly onSelectionChange?: UiEventHandler<UiEditEvent> | undefined;
	readonly onSubmit?: UiEventHandler<UiEditEvent> | undefined;
}

/**
 * The finite public surface exposed by a committed occurrence ref.
 *
 * Ref values are occurrence targets, not reconciler instances or native
 * pointers.  They remain useful for imperative overrides, focus, geometry,
 * diagnostics, and History identity while keeping renderer internals private.
 */
export interface OccurrenceRef {
	readonly kind: string;
	readonly lifecycle: "candidate" | "accepted" | "retired";
	diagnostics(): {
		readonly lifecycle: string;
		readonly hasAcceptedHandle: boolean;
	};
	setOverride(
		property: string,
		value: unknown,
	): { readonly revision: number; readonly accepted: true };
	clearOverride(property: string): {
		readonly revision: number;
		readonly accepted: true;
	};
	setStyleState(
		key: string,
		value: string,
	): {
		readonly revision: number;
		readonly accepted: true;
	};
	clearStyleState(key: string): {
		readonly revision: number;
		readonly accepted: true;
	};
	focus(): void;
	interceptPaste(routeId: string): void;
	visibleGeometry(): Promise<UiGeometry | null>;
	historyIdentity(): number | string;
}

export interface UiGeometry {
	readonly x: number;
	readonly y: number;
	readonly width: number;
	readonly height: number;
}

export interface BoxProps extends LayoutProps, EventProps {
	readonly children?: ReactNode;
	readonly ref?: Ref<OccurrenceRef>;
	readonly foreground?: ColorSpec;
	readonly background?: ColorSpec;
	readonly borderColor?: ColorSpec;
	readonly borderStyle?: BorderStyle;
	readonly borderGlyphs?: BorderGlyphs;
	readonly textAttributes?: Partial<Record<TextAttribute, boolean>>;
	readonly style?: StyleSpecValue | StyleSpec | StyleRef;
	readonly styleStates?: Readonly<Record<string, string>>;
	readonly hidden?: boolean;
}

export interface ContentProps extends LayoutProps, EventProps, BoxProps {
	readonly source?: ContentSource;
	readonly port?:
		| ContentPortToken
		| ContentConnectorToken
		| ExplicitContentPort
		| ExplicitContentConnector;
	readonly funnel?: Funnel;
	/** Internal Text/Content lowering marker; not part of the public props. */
	readonly literal?: ReactNode;
	readonly children?: ReactNode;
}

export interface ContentPortToken {
	readonly kind: "content-port-token";
	readonly family: "text";
}

/** Internal lifecycle shared by the Port and Connector tokens from one hook owner. */
export interface HookOwner {
	readonly tokens: Set<object>;
	readonly dependentConnectors: Set<ContentConnectorToken>;
	/** The token installed by the latest committed connector-hook effect. */
	currentToken: object | undefined;
	mounted: boolean;
	released: boolean;
	coordinator: RootContainer["coordinator"] | undefined;
	notifyRelease: (() => void) | undefined;
	/** Called after a committed connector dependency replacement. */
	notifyTokenReplacement: ((token: object) => void) | undefined;
	release: (() => void) | undefined;
}

export interface ContentConnectorToken {
	readonly kind: "content-connector-token";
	readonly port: ContentPortToken;
	readonly source: ContentSource;
	readonly funnel: TextFunnel;
}

const contentTokenOwners = new WeakMap<object, HookOwner>();

export function registerContentToken(token: object, owner: HookOwner): void {
	if (contentTokenOwners.has(token))
		throw new Error("Content token was already registered");
	contentTokenOwners.set(token, owner);
}

export function contentTokenOwner(token: object): HookOwner | undefined {
	return contentTokenOwners.get(token);
}

export interface EditorProps extends LayoutProps, EventProps {
	readonly children?: ReactNode;
	readonly ref?: Ref<OccurrenceRef>;
	readonly multiline?: boolean;
	readonly value?: string;
	readonly defaultValue?: string;
	readonly foreground?: ColorSpec;
	readonly background?: ColorSpec;
	readonly borderColor?: ColorSpec;
	readonly borderStyle?: BorderStyle;
	readonly borderGlyphs?: BorderGlyphs;
	readonly textAttributes?: Partial<Record<TextAttribute, boolean>>;
	readonly styleStates?: Readonly<Record<string, string>>;
	readonly hidden?: boolean;
}

export interface AnimationProps extends BoxProps {
	readonly intervalMs?: number;
}

export interface HistoryProps {
	readonly children?: ReactNode;
}

export interface HistoryUnitProps extends BoxProps {
	readonly flowBoundary?: HistoryFlowBoundary;
	readonly action?: HistoryUnitAction;
}

export interface NormalizedProperty {
	readonly name: UiPropertyName;
	readonly id: UiPropertyId;
	readonly value: unknown;
}

export interface NormalizedContent {
	readonly mode: "literal" | "source" | "lazy";
	readonly text?: string;
	readonly source?: ContentSource;
	readonly portToken?: ContentPortToken;
	readonly connectorToken?: ContentConnectorToken;
	readonly explicitPort?: ExplicitContentPort;
	readonly explicitConnector?: ExplicitContentConnector;
	readonly funnel: TextFunnel;
}

export interface NormalizedProps {
	readonly layout: LayoutKind;
	readonly properties: ReadonlyMap<UiPropertyName, NormalizedProperty>;
	readonly styleStates: ReadonlyMap<string, string>;
	readonly hidden: boolean;
	readonly content?: NormalizedContent;
	readonly control?: {
		readonly multiline: boolean;
		readonly value?: string;
		readonly intervalMs?: number;
		readonly controlled?: boolean;
	};
	readonly events: Map<string, UiEventHandler>;
	readonly historyUnit?: {
		readonly flowBoundary: 0 | 1;
		readonly action: 0 | 1;
	};
}

export function contentValuesEqual(
	left: NormalizedContent | undefined,
	right: NormalizedContent | undefined,
): boolean {
	if (left === undefined || right === undefined) return left === right;
	if (left.mode !== right.mode) return false;
	if (left.text !== right.text) return false;
	if (left.source !== right.source) return false;
	if (left.portToken !== right.portToken) return false;
	if (left.connectorToken !== right.connectorToken) return false;
	if (left.explicitPort !== right.explicitPort) return false;
	if (left.explicitConnector !== right.explicitConnector) return false;
	return funnelsEqual(left.funnel, right.funnel);
}

function funnelsEqual(left: TextFunnel, right: TextFunnel): boolean {
	if (
		left.mode !== right.mode ||
		left.wrap !== right.wrap ||
		left.hyperlinks !== right.hyperlinks ||
		left.delivery.kind !== right.delivery.kind
	)
		return false;
	return true;
}

export interface UiHandle {
	readonly host_namespace: number;
	readonly slot: number;
	readonly generation: number;
	readonly kind: number;
}

export interface AcceptedSnapshot {
	readonly props: NormalizedProps;
	readonly handle: UiHandle;
}

export class HostInstance {
	readonly root: RootContainer;
	readonly kind: HostKind;
	readonly initial: NormalizedProps;
	rootRole: "portal" | "historyUnit" | undefined;
	/** Accepted owner correspondence for a typed portal root. */
	portalOwner: HostInstance | undefined;
	parent: HostInstance | undefined;
	firstChild: HostInstance | undefined;
	lastChild: HostInstance | undefined;
	previousSibling: HostInstance | undefined;
	nextSibling: HostInstance | undefined;
	lifecycle: "candidate" | "accepted" | "retired" = "candidate";
	handle: UiHandle | undefined;
	/** Qualified resource handles are private renderer correspondence. */
	port: UiHandle | undefined;
	connector: UiHandle | undefined;
	control: UiHandle | undefined;
	portToken: ContentPortToken | undefined;
	connectorToken: ContentConnectorToken | undefined;
	explicitPort: ExplicitContentPort | undefined;
	explicitConnector: ExplicitContentConnector | undefined;
	localOrdinal: number | undefined;
	accepted: AcceptedSnapshot | undefined;
	pending: NormalizedProps;

	constructor(root: RootContainer, kind: HostKind, props: NormalizedProps) {
		this.root = root;
		this.kind = kind;
		this.initial = props;
		this.rootRole = undefined;
		this.portalOwner = undefined;
		this.explicitPort = undefined;
		this.explicitConnector = undefined;
		this.pending = props;
	}
}

/** Text instances are frontend candidates and lower to ordinary ContentHost nodes. */
export class HostTextInstance {
	readonly root: RootContainer;
	readonly node: HostInstance;
	text: string;

	constructor(root: RootContainer, text: string) {
		this.root = root;
		this.text = text;
		this.node = new HostInstance(
			root,
			HOST_KINDS.contentHost,
			normalizeProps("content", { children: text }),
		);
	}
}

export type HostChild = HostInstance | HostTextInstance;

export function hostNode(child: HostChild): HostInstance {
	return child instanceof HostTextInstance ? child.node : child;
}

export function hostKindForType(type: string): HostKind {
	switch (type) {
		case "box":
		case "row":
		case "column":
		case "grid":
		case "portal":
			return HOST_KINDS.box;
		case "content":
		case "text":
			return HOST_KINDS.contentHost;
		case "editor":
			return HOST_KINDS.editor;
		case "scroll":
			return HOST_KINDS.scroll;
		case "animation":
			return HOST_KINDS.animation;
		case "historyUnit":
			return HOST_KINDS.box;
		default:
			throw new TypeError(`unsupported iyon host type ${JSON.stringify(type)}`);
	}
}

function hostTypeForKind(kind: HostKind): HostType {
	switch (kind) {
		case HOST_KINDS.contentHost:
			return "content";
		case HOST_KINDS.editor:
			return "editor";
		case HOST_KINDS.scroll:
			return "scroll";
		case HOST_KINDS.animation:
			return "animation";
		default:
			return "box";
	}
}

export function isHostType(type: unknown): type is string {
	return (
		typeof type === "string" &&
		[
			"box",
			"row",
			"column",
			"grid",
			"portal",
			"content",
			"text",
			"editor",
			"scroll",
			"animation",
			"historyUnit",
		].includes(type)
	);
}

const EVENT_BITS: Readonly<Record<string, number>> = {
	onPress: 1,
	onInput: 2,
	onEdit: 4,
	onChange: 8,
	onSelectionChange: 16,
	onSubmit: 32,
};

export function eventMask(events: ReadonlyMap<string, UiEventHandler>): number {
	let mask = 0;
	for (const name of events.keys()) mask |= EVENT_BITS[name] ?? 0;
	return mask >>> 0;
}

export function normalizeProps(type: string, input: unknown): NormalizedProps {
	if (typeof input !== "object" || input === null)
		throw new TypeError(`${type} props must be an object`);
	const props = input as Record<string, unknown>;
	const kind = hostKindForType(type);
	const properties = normalizeProperties(kind, props);
	const layout = layoutKindForType(type);
	if (layout !== "box") {
		properties.set("layout", {
			name: "layout",
			id: UI_PROPERTIES.layout,
			value: layout,
		});
	}
	const hidden = normalizeHidden(props);
	const styleStates = normalizeStyleStates(props.styleStates);
	const events = normalizeEvents(props);
	const content =
		kind === HOST_KINDS.contentHost ? normalizeContent(props) : undefined;
	const control = normalizeControl(kind, props);
	const historyUnit =
		type === "historyUnit" ? normalizeHistoryUnit(props) : undefined;
	assertKnownProps(
		type,
		props,
		kind === HOST_KINDS.contentHost,
		kind === HOST_KINDS.editor,
	);
	return {
		layout: layoutKindForType(type),
		properties,
		styleStates,
		hidden,
		...(content === undefined ? {} : { content }),
		...(control === undefined ? {} : { control }),
		events,
		...(historyUnit === undefined ? {} : { historyUnit }),
	};
}

/** Normalizes one imperative override using the same finite property boundary. */
export function normalizePublicProperty(
	kind: HostKind,
	name: string,
	value: unknown,
): NormalizedProperty {
	const type = hostTypeForKind(kind);
	const id = publicUiPropertyId(kind, name);
	const normalized = normalizeProps(type, { [name]: value });
	const property = normalized.properties.get(name as UiPropertyName);
	if (property === undefined)
		throw new TypeError(
			`property ${JSON.stringify(name)} is not legal on ${type}`,
		);
	if (property.id !== id)
		throw new Error("generated property descriptor drift");
	return property;
}

export function publicUiPropertyId(kind: HostKind, name: string): UiPropertyId {
	if (!(name in UI_PROPERTIES))
		throw new RangeError(`unknown UI property ${JSON.stringify(name)}`);
	const kindName =
		kind === HOST_KINDS.box
			? "Box"
			: kind === HOST_KINDS.contentHost
				? "ContentHost"
				: kind === HOST_KINDS.editor
					? "Editor"
					: kind === HOST_KINDS.scroll
						? "Scroll"
						: "Animation";
	const descriptor = UI_PROPERTY_DESCRIPTORS.find(
		(entry) => entry.name === name,
	);
	if (descriptor === undefined || !descriptor.legalKinds.includes(kindName))
		throw new TypeError(
			`property ${JSON.stringify(name)} is not legal on ${kindName}`,
		);
	return descriptor.id as UiPropertyId;
}

function layoutKindForType(type: string): LayoutKind {
	if (type === "row" || type === "column" || type === "grid") return type;
	return "box";
}

function normalizeProperties(
	kind: HostKind,
	props: Record<string, unknown>,
): Map<UiPropertyName, NormalizedProperty> {
	const properties = new Map<UiPropertyName, NormalizedProperty>();
	for (const name of [
		"width",
		"height",
		"padding",
		"minWidth",
		"maxWidth",
		"minHeight",
		"maxHeight",
		"gap",
		"display",
		"direction",
		"flexDirection",
		"flexWrap",
		"flexGrow",
		"flexShrink",
		"flexBasis",
		"margin",
		"alignItems",
		"alignSelf",
		"alignContent",
		"justifyContent",
		"justifyItems",
		"justifySelf",
		"columnGap",
		"rowGap",
		"gridTemplateColumns",
		"gridTemplateRows",
		"gridAutoColumns",
		"gridAutoRows",
		"gridAutoFlow",
		"gridColumn",
		"gridRow",
		"position",
		"inset",
		"alignment",
		"borderEdges",
		"foreground",
		"background",
		"borderColor",
		"borderStyle",
		"borderGlyphs",
		"textAttributes",
		"style",
	] as const) {
		const value = props[name];
		if (value === undefined) continue;
		const normalized = normalizeProperty(name, value);
		properties.set(name, {
			name,
			id: UI_PROPERTIES[name],
			value: normalized,
		});
	}
	validateLegalProperties(kind, properties);
	return properties;
}

function normalizeHidden(props: Record<string, unknown>): boolean {
	const hidden = props.hidden ?? false;
	if (typeof hidden !== "boolean")
		throw new TypeError("hidden must be boolean");
	return hidden;
}

/** Shared validation for props and imperative style-state entry points. */
export function validateStyleStateKey(key: unknown): asserts key is string {
	if (typeof key !== "string" || key.length === 0 || key.includes("\0"))
		throw new RangeError("style state keys must be nonempty and NUL-free");
}

export function validateStyleState(
	key: unknown,
	value: unknown,
): asserts value is string {
	validateStyleStateKey(key);
	if (typeof value !== "string" || value.length === 0 || value.includes("\0"))
		throw new RangeError(
			"style state values must be nonempty NUL-free strings",
		);
}

function normalizeStyleStates(value: unknown): ReadonlyMap<string, string> {
	if (value === undefined) return new Map();
	if (typeof value !== "object" || value === null || Array.isArray(value))
		throw new TypeError("styleStates must be an object");
	const result = new Map<string, string>();
	for (const key of Object.keys(value as Record<string, unknown>)) {
		const stateValue = (value as Record<string, unknown>)[key];
		validateStyleState(key, stateValue);
		result.set(key, stateValue);
	}
	return result;
}

function normalizeEvents(
	props: Record<string, unknown>,
): Map<string, UiEventHandler> {
	const events = new Map<string, UiEventHandler>();
	for (const name of Object.keys(EVENT_BITS)) {
		const value = props[name];
		if (value === undefined) continue;
		if (typeof value !== "function")
			throw new TypeError(`${name} must be a function`);
		events.set(name, value as UiEventHandler);
	}
	return events;
}

function normalizeControl(
	kind: HostKind,
	props: Record<string, unknown>,
): NormalizedProps["control"] {
	if (kind === HOST_KINDS.editor) return normalizeEditorControl(props);
	if (kind === HOST_KINDS.scroll)
		return { multiline: false, controlled: false };
	if (kind !== HOST_KINDS.animation) return undefined;
	const intervalValue = props.intervalMs;
	if (
		intervalValue !== undefined &&
		(!Number.isSafeInteger(intervalValue) ||
			(intervalValue as number) <= 0 ||
			(intervalValue as number) > 0xffff_ffff)
	)
		throw new RangeError("animation intervalMs must be a positive u32");
	const intervalMs = intervalValue as number | undefined;
	return {
		multiline: false,
		...(intervalMs === undefined ? {} : { intervalMs }),
		controlled: false,
	};
}

function normalizeHistoryUnit(
	props: Record<string, unknown>,
): NonNullable<NormalizedProps["historyUnit"]> {
	const flowBoundary = props.flowBoundary ?? "default";
	if (flowBoundary !== "default" && flowBoundary !== "attachToPrevious")
		throw new RangeError("HistoryUnit flowBoundary is invalid");
	const action = props.action ?? "live";
	if (action !== "live" && action !== "freeze")
		throw new RangeError("HistoryUnit action is invalid");
	return {
		flowBoundary: flowBoundary === "attachToPrevious" ? 1 : 0,
		action: action === "freeze" ? 1 : 0,
	};
}

function normalizeEditorControl(
	props: Record<string, unknown>,
): NormalizedProps["control"] {
	const multiline = props.multiline ?? false;
	if (typeof multiline !== "boolean")
		throw new TypeError("editor multiline must be boolean");
	const value = props.value ?? props.defaultValue;
	if (value !== undefined && typeof value !== "string")
		throw new TypeError("editor value must be a string");
	if (props.value !== undefined && props.defaultValue !== undefined)
		throw new TypeError("editor cannot specify both value and defaultValue");
	return {
		multiline,
		...(value === undefined ? {} : { value }),
		controlled: props.value !== undefined,
	};
}

function assertKnownProps(
	type: string,
	props: Record<string, unknown>,
	content: boolean,
	editor: boolean,
): void {
	const known = new Set([
		"children",
		"ref",
		"hidden",
		"width",
		"height",
		"padding",
		"minWidth",
		"maxWidth",
		"minHeight",
		"maxHeight",
		"gap",
		"display",
		"direction",
		"flexDirection",
		"flexWrap",
		"flexGrow",
		"flexShrink",
		"flexBasis",
		"margin",
		"alignItems",
		"alignSelf",
		"alignContent",
		"justifyContent",
		"justifyItems",
		"justifySelf",
		"columnGap",
		"rowGap",
		"gridTemplateColumns",
		"gridTemplateRows",
		"gridAutoColumns",
		"gridAutoRows",
		"gridAutoFlow",
		"gridColumn",
		"gridRow",
		"position",
		"inset",
		"alignment",
		"borderEdges",
		"foreground",
		"background",
		"borderColor",
		"borderStyle",
		"borderGlyphs",
		"textAttributes",
		"style",
		"styleStates",
		...Object.keys(EVENT_BITS),
		...(content ? ["source", "port", "funnel", "literal"] : []),
		...(editor ? ["multiline", "value", "defaultValue"] : []),
		...(type === "portal" ? ["portalOwner"] : []),
		...(type === "historyUnit" ? ["flowBoundary", "action"] : []),
		...(hostKindForType(type) === HOST_KINDS.animation ? ["intervalMs"] : []),
	]);
	for (const name of Object.keys(props))
		if (!known.has(name))
			throw new RangeError(
				`${type} does not support prop ${JSON.stringify(name)}`,
			);
}

function validateLegalProperties(
	kind: HostKind,
	properties: ReadonlyMap<UiPropertyName, NormalizedProperty>,
): void {
	const kindName =
		kind === HOST_KINDS.box
			? "Box"
			: kind === HOST_KINDS.contentHost
				? "ContentHost"
				: kind === HOST_KINDS.editor
					? "Editor"
					: kind === HOST_KINDS.scroll
						? "Scroll"
						: "Animation";
	for (const property of properties.values()) {
		const legal =
			UI_PROPERTY_DESCRIPTORS.find((entry) => entry.id === property.id)
				?.legalKinds ?? [];
		if (!legal.includes(kindName))
			throw new TypeError(
				`property ${property.name} is not legal on ${kindName}`,
			);
	}
}

function normalizeContent(props: Record<string, unknown>): NormalizedContent {
	const source = props.source as ContentSource | undefined;
	const port = props.port as
		| ContentPortToken
		| ContentConnectorToken
		| undefined;
	if (source !== undefined && port !== undefined)
		throw new TypeError("Content accepts either source or port, not both");
	const selectedFunnel = normalizeFunnel(props.funnel);
	if (port !== undefined) return normalizePort(port, selectedFunnel);
	if (source !== undefined) return normalizeSource(source, selectedFunnel);
	return normalizeLiteral(props, selectedFunnel);
}

function normalizeFunnel(value: unknown): TextFunnel {
	const funnel = value === undefined ? TextFunnel.plain() : value;
	if (!(funnel instanceof TextFunnel))
		throw new TypeError("Content funnel must be a TextFunnel");
	return funnel;
}

function normalizePort(
	port:
		| ContentPortToken
		| ContentConnectorToken
		| ExplicitContentPort
		| ExplicitContentConnector,
	funnel: TextFunnel,
): NormalizedContent {
	if (typeof port !== "object" || port === null)
		throw new TypeError("Content port must be a live ContentPort");
	if (
		(port as unknown as ContentConnectorToken).kind ===
		"content-connector-token"
	) {
		const connector = port as ContentConnectorToken;
		const connectorOwner = contentTokenOwner(connector);
		const portOwner = contentTokenOwner(connector.port);
		if (connectorOwner === undefined || portOwner === undefined)
			throw new TypeError("Content Connector is not a live qualified token");
		const connectorFunnel = normalizeFunnel(connector.funnel);
		normalizeSource(connector.source, connectorFunnel);
		return {
			mode: "lazy",
			portToken: connector.port,
			connectorToken: connector,
			source: connector.source,
			funnel: connectorFunnel,
		};
	}
	if ((port as unknown as ContentPortToken).kind === "content-port-token") {
		const token = port as unknown as ContentPortToken;
		if (contentTokenOwner(token) === undefined)
			throw new TypeError("Content Port is not a live qualified token");
		return {
			mode: "lazy",
			portToken: token,
			funnel,
		};
	}
	if (isExplicitContentConnector(port)) {
		if (port.disposed) throw new TypeError("Content Connector is disposed");
		return {
			mode: "lazy",
			explicitPort: port.attachedPort,
			explicitConnector: port,
			source: port.attachedSource,
			funnel: port.funnel,
		};
	}
	if (isExplicitContentPort(port)) {
		if (port.disposed) throw new TypeError("Content Port is disposed");
		return {
			mode: "lazy",
			explicitPort: port,
			funnel,
		};
	}
	throw new TypeError("Content port is not a qualified React Port token");
}

function normalizeSource(
	source: ContentSource,
	funnel: TextFunnel,
): NormalizedContent {
	if (
		!(source instanceof TextBlockSource) &&
		!(source instanceof TextStreamSource)
	)
		throw new TypeError("Content source must be a text Source");
	return {
		mode: "source",
		source,
		funnel,
	};
}

function normalizeLiteral(
	props: Record<string, unknown>,
	selectedFunnel: TextFunnel,
): NormalizedContent {
	const child = props.literal ?? props.children;
	const text =
		child === undefined || child === null || child === false
			? ""
			: textValue(child);
	return {
		mode: "literal",
		text,
		funnel: selectedFunnel,
	};
}

function textValue(value: unknown): string {
	if (typeof value === "string") return value;
	if (typeof value === "number" && Number.isFinite(value)) return String(value);
	if (typeof value === "bigint") return String(value);
	throw new TypeError("text content must be a string, number, or bigint");
}

function finiteScalar(
	value: unknown,
	name: string,
	minimum: number,
	maximum: number,
): number {
	if (typeof value !== "number" || !Number.isFinite(value))
		throw new TypeError(`${name} must be a finite number`);
	const rounded = Math.fround(value);
	if (!Number.isFinite(rounded) || rounded < minimum || rounded > maximum)
		throw new RangeError(
			`${name} must fit finite f32 range ${minimum}..${maximum}`,
		);
	return rounded === 0 ? 0 : rounded;
}

function dimension(
	value: unknown,
	name: string,
	allowAuto: boolean,
	allowNegative = false,
	allowLegacyModes = false,
): DimensionValue | "auto" | "fit" | "fill" {
	if (value === "fit" || value === "fill") {
		if (!allowLegacyModes)
			throw new RangeError(`${name} accepts only typed dimensions`);
		return value;
	}
	if (value === "auto") {
		if (!allowAuto) throw new RangeError(`${name} does not accept auto`);
		return value;
	}
	if (typeof value === "number")
		return {
			unit: "length",
			value: finiteScalar(value, name, allowNegative ? -65535 : 0, 65535),
		};
	if (typeof value !== "object" || value === null)
		throw new TypeError(`${name} must be a length, percentage, or auto`);
	return dimensionObject(value as Record<string, unknown>, name, allowNegative);
}

function dimensionObject(
	candidate: Record<string, unknown>,
	name: string,
	allowNegative: boolean,
): DimensionValue {
	for (const key of Object.keys(candidate))
		if (key !== "unit" && key !== "value")
			throw new RangeError(
				`${name} does not support field ${JSON.stringify(key)}`,
			);
	if (candidate.unit !== "length" && candidate.unit !== "percent")
		throw new RangeError(`${name}.unit must be length or percent`);
	return {
		unit: candidate.unit,
		value: finiteScalar(
			candidate.value,
			`${name}.value`,
			candidate.unit === "percent" ? 0 : allowNegative ? -65535 : 0,
			candidate.unit === "percent" ? 1 : 65535,
		),
	};
}

function dimensionInsets(
	value: unknown,
	name: string,
	allowAuto: boolean,
	defaultValue: DimensionInput = allowAuto ? "auto" : 0,
): Record<"top" | "right" | "bottom" | "left", DimensionValue | "auto"> {
	const source =
		typeof value === "number" || value === "auto" || isDimensionObject(value)
			? { top: value, right: value, bottom: value, left: value }
			: value;
	if (typeof source !== "object" || source === null || Array.isArray(source))
		throw new TypeError(`${name} must be a scalar or side object`);
	const item = source as Record<string, unknown>;
	const sides = ["top", "right", "bottom", "left"] as const;
	for (const key of Object.keys(item))
		if (!(sides as readonly string[]).includes(key))
			throw new RangeError(
				`${name} does not support field ${JSON.stringify(key)}`,
			);
	const result = {} as Record<(typeof sides)[number], DimensionValue | "auto">;
	for (const side of sides)
		result[side] = dimension(
			item[side] ?? defaultValue,
			`${name}.${side}`,
			allowAuto,
			true,
		) as DimensionValue | "auto";
	return result;
}

function isDimensionObject(value: unknown): value is DimensionValue {
	return (
		typeof value === "object" &&
		value !== null &&
		!Array.isArray(value) &&
		((value as Record<string, unknown>).unit === "length" ||
			(value as Record<string, unknown>).unit === "percent")
	);
}

function generatedEnumValue(
	name: UiPropertyName,
	value: unknown,
	formName: string,
): string {
	const descriptor = uiPropertyDescriptorByName(name);
	const form = uiValueEncoding(descriptor.valueKind).forms.find(
		(candidate) => candidate.name === formName,
	);
	if (
		typeof value !== "string" ||
		form === undefined ||
		!form.names.includes(value) ||
		(descriptor.allowedValues.length > 0 &&
			!descriptor.allowedValues.includes(value))
	)
		throw new RangeError(`${name} has an unknown generated enum value`);
	return value;
}

function trackList(value: unknown, name: string): readonly GridTrack[] {
	if (!Array.isArray(value) || value.length > 64)
		throw new RangeError(`${name} must contain at most 64 tracks`);
	return value.map((raw, index) => normalizeTrack(raw, `${name}[${index}]`));
}

function normalizeTrack(raw: unknown, name: string): GridTrack {
	if (raw === "auto" || raw === "minContent" || raw === "maxContent")
		return raw;
	if (typeof raw !== "object" || raw === null)
		throw new TypeError(`${name} must be a typed track`);
	const track = raw as Record<string, unknown>;
	if (track.type === "minmax") return normalizeMinMaxTrack(track, name);
	return normalizeSimpleTrack(track, name);
}

function normalizeMinMaxTrack(
	track: Record<string, unknown>,
	name: string,
): GridTrack {
	for (const key of Object.keys(track))
		if (key !== "type" && key !== "min" && key !== "max")
			throw new RangeError(
				`${name} does not support field ${JSON.stringify(key)}`,
			);
	return {
		type: "minmax",
		min: normalizeTrackBound(
			track.min,
			`${name}.min`,
			false,
		) as GridTrackMinBound,
		max: normalizeTrackBound(
			track.max,
			`${name}.max`,
			true,
		) as GridTrackMaxBound,
	};
}

function hasOnlyKeys(
	value: Record<string, unknown>,
	keys: readonly string[],
): boolean {
	return Object.keys(value).every((key) => keys.includes(key));
}

function isSimpleTrack(value: unknown): value is {
	readonly type: "length" | "percent" | "fr";
	readonly value: number;
} {
	return (
		typeof value === "object" &&
		value !== null &&
		!Array.isArray(value) &&
		hasOnlyKeys(value as Record<string, unknown>, ["type", "value"]) &&
		((value as Record<string, unknown>).type === "length" ||
			(value as Record<string, unknown>).type === "percent" ||
			(value as Record<string, unknown>).type === "fr") &&
		typeof (value as Record<string, unknown>).value === "number"
	);
}

function normalizeSimpleTrack(
	value: unknown,
	name: string,
): { readonly type: "length" | "percent" | "fr"; readonly value: number } {
	if (!isSimpleTrack(value))
		throw new TypeError(`${name} must be a simple track`);
	return {
		type: value.type,
		value: finiteScalar(
			value.value,
			`${name}.value`,
			0,
			value.type === "percent" ? 1 : 65535,
		),
	};
}

function normalizeTrackBound(
	value: unknown,
	name: string,
	allowFr: boolean,
): GridTrackMinBound | GridTrackMaxBound {
	if (value === "auto" || value === "minContent" || value === "maxContent")
		return value;
	const track = normalizeSimpleTrack(value, name);
	if (track.type === "fr" && !allowFr)
		throw new RangeError(`${name} cannot use an fr minimum track`);
	return track;
}

function placement(value: unknown, name: string): GridPlacement {
	if (typeof value !== "object" || value === null || Array.isArray(value))
		throw new TypeError(`${name} must be a placement object`);
	const candidate = value as Record<string, unknown>;
	for (const key of Object.keys(candidate))
		if (key !== "start" && key !== "end")
			throw new RangeError(
				`${name} does not support field ${JSON.stringify(key)}`,
			);
	const normalizeLine = (
		line: unknown,
		side: string,
	): GridPlacement["start"] => {
		if (line === undefined || line === "auto") return "auto";
		if (
			typeof line === "number" &&
			Number.isSafeInteger(line) &&
			line !== 0 &&
			line >= -32768 &&
			line <= 32767
		)
			return line;
		if (typeof line === "object" && line !== null) {
			const candidate = line as Record<string, unknown>;
			if (!hasOnlyKeys(candidate, ["span"]))
				throw new RangeError(
					`${name}.${side} span does not support extra fields`,
				);
			const span = u16(candidate.span, `${name}.${side}.span`);
			if (span === 0)
				throw new RangeError(`${name}.${side}.span must be positive`);
			return { span };
		}
		throw new RangeError(
			`${name}.${side} must be auto, a nonzero line, or a positive span`,
		);
	};
	return {
		start: normalizeLine(candidate.start, "start"),
		end: normalizeLine(candidate.end, "end"),
	};
}

function normalizeProperty(name: UiPropertyName, value: unknown): unknown {
	switch (name) {
		case "width":
		case "height":
			return dimension(value, name, false, false, true);
		case "padding": {
			const result = insetsValue(value);
			return result;
		}
		case "minWidth":
		case "maxWidth":
		case "minHeight":
		case "maxHeight":
		case "gap":
			return u16(value, name);
		case "display":
			return generatedEnumValue(name, value, "display");
		case "direction":
			return generatedEnumValue(name, value, "direction");
		case "flexDirection":
			return generatedEnumValue(name, value, "flex_direction");
		case "flexWrap":
			return generatedEnumValue(name, value, "flex_wrap");
		case "flexGrow":
		case "flexShrink":
			return finiteScalar(value, name, 0, 1_000_000);
		case "flexBasis":
			return dimension(value, name, true);
		case "margin":
			return dimensionInsets(value, name, true, 0);
		case "alignItems":
		case "alignSelf":
		case "justifyItems":
		case "justifySelf":
			return generatedEnumValue(name, value, "alignment_mode");
		case "alignContent":
		case "justifyContent":
			return generatedEnumValue(name, value, "alignment_mode");
		case "columnGap":
		case "rowGap":
			return dimension(value, name, false, false);
		case "gridTemplateColumns":
		case "gridTemplateRows":
		case "gridAutoColumns":
		case "gridAutoRows":
			return trackList(value, name);
		case "gridAutoFlow":
			return generatedEnumValue(name, value, "grid_auto_flow");
		case "gridColumn":
		case "gridRow":
			return placement(value, name);
		case "position":
			return generatedEnumValue(name, value, "position");
		case "inset":
			return dimensionInsets(value, name, true, "auto");
		case "alignment":
			return alignment(value);
		case "borderEdges":
			return edges(value);
		case "foreground":
		case "background":
		case "borderColor":
			return color(value);
		case "borderStyle":
			return generatedEnumValue(name, value, "border");
		case "borderGlyphs":
			return glyphs(value);
		case "textAttributes":
			return attributes(value);
		case "style":
			return normalizeStyle(value);
	}
}

function normalizeStyle(
	value: unknown,
): StyleSpecValue & { readonly theme?: string } {
	let raw: StyleSpecValue;
	let theme: string | undefined;
	if (value instanceof StyleRef) {
		semanticStyleFor(value);
		raw = value.local.value;
		theme = value.themeKey?.value;
	} else if (value instanceof StyleSpec) {
		semanticStyleFor(value);
		raw = value.value;
	} else if (isStyleValueObject(value)) {
		raw = ownedStyleInput(value);
		semanticStyleFor(raw);
	} else {
		throw new TypeError(
			"style must be a StyleSpec, StyleRef, or finite Style value",
		);
	}
	return ownedStyleValue(raw, theme);
}

function isStyleValueObject(value: unknown): value is StyleSpecValue {
	if (typeof value !== "object" || value === null) return false;
	const candidate = value as Record<string, unknown>;
	if ((candidate.kind as string | undefined) !== undefined) return false;
	const known = new Set(["foreground", "background", "attributes"]);
	if (!Object.hasOwn(candidate, "attributes")) return false;
	for (const key of Object.keys(candidate))
		if (!known.has(key))
			throw new RangeError(
				`style does not support prop ${JSON.stringify(key)}`,
			);
	return true;
}

function ownedStyleInput(value: StyleSpecValue): StyleSpecValue {
	const candidate = value as unknown as Record<string, unknown>;
	return {
		...(Object.hasOwn(candidate, "foreground")
			? { foreground: candidate.foreground as ColorSpec }
			: {}),
		...(Object.hasOwn(candidate, "background")
			? { background: candidate.background as ColorSpec }
			: {}),
		attributes: candidate.attributes as StyleSpecValue["attributes"],
	};
}

function ownedStyleValue(
	value: StyleSpecValue,
	theme: string | undefined,
): StyleSpecValue & { readonly theme?: string } {
	const attributes = ownedTextAttributes(value.attributes);
	const result = {
		...(value.foreground === undefined
			? {}
			: { foreground: ownedStyleColor(value.foreground) }),
		...(value.background === undefined
			? {}
			: { background: ownedStyleColor(value.background) }),
		attributes,
		...(theme === undefined ? {} : { theme: ownedThemeKey(theme) }),
	};
	return Object.freeze(result);
}

function ownedTextAttributes(
	value: StyleSpecValue["attributes"],
): Readonly<Partial<Record<TextAttribute, boolean>>> {
	if (typeof value !== "object" || value === null)
		throw new TypeError("style attributes must be an object");
	const result: Partial<Record<TextAttribute, boolean>> = {};
	for (const [name, enabled] of Object.entries(value)) {
		if (enabled === undefined) continue;
		const attribute = validateTextAttribute(name);
		if (typeof enabled !== "boolean")
			throw new TypeError(
				`text attribute ${JSON.stringify(name)} must be boolean`,
			);
		result[attribute] = enabled;
	}
	return Object.freeze(result);
}

function ownedThemeKey(value: unknown): string {
	if (typeof value !== "string" || value.length === 0 || /[\s\0]/u.test(value))
		throw new RangeError(
			"theme key must be non-empty and contain no whitespace or NUL",
		);
	return value;
}

function u16(value: unknown, name: string): number {
	if (
		!Number.isSafeInteger(value) ||
		(value as number) < 0 ||
		(value as number) > 65535
	)
		throw new RangeError(`${name} must be an integer from 0 to 65535`);
	return value as number;
}

function insetsValue(value: unknown): InsetsValue {
	if (typeof value === "number") return insets(value);
	if (typeof value !== "object" || value === null)
		throw new TypeError("padding must be a number or Insets value");
	const candidate = value as Record<string, unknown>;
	if ("value" in candidate) return insetsValue(candidate.value);
	return {
		top: u16(candidate.top, "padding.top"),
		right: u16(candidate.right, "padding.right"),
		bottom: u16(candidate.bottom, "padding.bottom"),
		left: u16(candidate.left, "padding.left"),
	};
}

function alignment(value: unknown): {
	readonly horizontal: number;
	readonly vertical: number;
} {
	if (typeof value !== "object" || value === null)
		throw new TypeError("alignment must be an object");
	const candidate = value as Record<string, unknown>;
	return {
		horizontal: axis(candidate.horizontal ?? "start"),
		vertical: axis(candidate.vertical ?? "top"),
	};
}

function axis(value: unknown): number {
	if (value === "start") return 0;
	if (value === "center") return 1;
	if (value === "end") return 2;
	if (value === "top") return 3;
	if (value === "bottom") return 4;
	throw new RangeError("alignment axis is invalid");
}

function edges(value: unknown): [boolean, boolean, boolean, boolean] {
	if (value === "all") return [true, true, true, true];
	if (value === "topBottom") return [true, false, true, false];
	if (typeof value !== "object" || value === null)
		throw new TypeError("borderEdges is invalid");
	const candidate = value as Record<string, unknown>;
	return [
		bool(candidate.top),
		bool(candidate.right),
		bool(candidate.bottom),
		bool(candidate.left),
	];
}

function bool(value: unknown): boolean {
	return value === undefined
		? false
		: typeof value === "boolean"
			? value
			: (() => {
					throw new TypeError("border edge must be boolean");
				})();
}

function color(value: unknown): ColorSpec {
	const candidate =
		typeof value === "string" ? { type: "named", value } : value;
	if (typeof candidate !== "object" || candidate === null)
		throw new TypeError("color is invalid");
	semanticColorFor(candidate as ColorSpec);
	return ownedStyleColor(candidate as ColorSpec);
}

function ownedStyleColor(value: ColorSpec): ColorSpec {
	switch (value.type) {
		case "named":
			return Object.freeze({ type: "named", value: value.value });
		case "indexed":
			return Object.freeze({ type: "indexed", value: value.value });
		case "rgb":
			return Object.freeze({ type: "rgb", r: value.r, g: value.g, b: value.b });
		case "theme":
			throw new TypeError("theme colors require a StyleRef theme key");
		default:
			throw new TypeError("color is invalid");
	}
}

function glyphs(value: unknown): BorderGlyphs {
	if (typeof value !== "object" || value === null)
		throw new TypeError("borderGlyphs is invalid");
	const result = value as Record<string, unknown>;
	const names = [
		"top",
		"right",
		"bottom",
		"left",
		"topLeft",
		"topRight",
		"bottomLeft",
		"bottomRight",
	] as const;
	const output = {} as Record<string, string>;
	for (const name of names) {
		if (
			typeof result[name] !== "string" ||
			result[name] === "" ||
			result[name].includes("\0")
		)
			throw new RangeError(
				`borderGlyphs.${name} must be a nonempty string without NUL`,
			);
		output[name] = result[name];
	}
	return output as unknown as BorderGlyphs;
}

function attributes(value: unknown): Partial<Record<TextAttribute, boolean>> {
	if (typeof value !== "object" || value === null)
		throw new TypeError("textAttributes is invalid");
	const names: TextAttribute[] = [
		"bold",
		"dim",
		"italic",
		"underline",
		"reversed",
		"strikethrough",
	];
	const result = value as Record<string, unknown>;
	const output: Partial<Record<TextAttribute, boolean>> = {};
	for (const name of Object.keys(result)) {
		if (
			!names.includes(name as TextAttribute) ||
			typeof result[name] !== "boolean"
		)
			throw new RangeError(`textAttributes.${name} is invalid`);
		output[name as TextAttribute] = result[name] as boolean;
	}
	return output;
}
