import { createElement, type ReactElement, type ReactNode } from "react";
import type {
	AnimationProps,
	BoxProps,
	ContentProps,
	EditorProps,
	HistoryProps,
	HistoryUnitProps,
	LayoutProps,
} from "./instance.ts";
import { normalizeProps } from "./instance.ts";

function checkedElement(
	type:
		| "box"
		| "row"
		| "column"
		| "grid"
		| "editor"
		| "scroll"
		| "animation"
		| "historyUnit",
	props: object,
): ReactElement {
	normalizeProps(type, props);
	return createElement(type, props);
}

/** Ordinary Box occurrence. Row, Column and Grid are finite layout conveniences over Box. */
export function Box(props: BoxProps): ReactElement {
	return checkedElement("box", props);
}
export function Row(props: BoxProps): ReactElement {
	return checkedElement("row", props);
}
export function Column(props: BoxProps): ReactElement {
	return checkedElement("column", props);
}
export function Grid(props: BoxProps): ReactElement {
	return checkedElement("grid", props);
}

/** Childless ContentHost backed by a private literal or a qualified Source/Port. */
export function Content(props: ContentProps): ReactElement {
	const { children, ...rest } = props;
	const next = { ...rest, literal: children };
	normalizeProps("content", next);
	return createElement("content", next);
}

/** Text syntax sugar; it is never a structural native Text kind. */
export function Text(props: ContentProps): ReactElement {
	const { children, ...rest } = props;
	const next = { ...rest, literal: children };
	normalizeProps("text", next);
	return createElement("text", next);
}

export function Editor(props: EditorProps): ReactElement {
	return checkedElement("editor", props);
}
export function Scroll(props: BoxProps): ReactElement {
	return checkedElement("scroll", props);
}
export function Animation(props: AnimationProps): ReactElement {
	return checkedElement("animation", props);
}

/** Logical root for one or more typed HistoryUnit occurrence roots. */
export function History(props: HistoryProps): ReactNode {
	return props.children ?? null;
}

/** Host-owned History unit root; children are validated by the native History adapter. */
export function HistoryUnit(props: HistoryUnitProps): ReactElement {
	return checkedElement("historyUnit", props);
}

export type {
	AnimationProps,
	BoxProps,
	ContentProps,
	EditorProps,
	HistoryProps,
	HistoryUnitProps,
	LayoutProps,
};
