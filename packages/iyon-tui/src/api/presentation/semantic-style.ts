import type { ColorSpec } from "./theme.ts";
import type { StyleRef, StyleSpec, StyleSpecValue } from "./style.ts";

export type SemanticColor = ColorSpec;
export type SemanticStyle = Readonly<{
	readonly foreground?: ColorSpec;
	readonly background?: ColorSpec;
	readonly attributes: Readonly<Record<string, boolean>>;
}>;

export function semanticColorFor(color: ColorSpec): SemanticColor {
	if (color.type === "named" && !ANSI_COLORS.has(color.value))
		throw new RangeError(`unknown ANSI color ${JSON.stringify(color.value)}`);
	if (
		color.type === "indexed" &&
		(!Number.isInteger(color.value) || color.value < 0 || color.value > 255)
	)
		throw new RangeError("indexed ANSI color must be an integer from 0 to 255");
	if (
		color.type === "rgb" &&
		[color.r, color.g, color.b].some(
			(value) => !Number.isInteger(value) || value < 0 || value > 255,
		)
	)
		throw new RangeError("RGB channel must be an integer from 0 to 255");
	return Object.freeze({ ...color });
}

const ANSI_COLORS = new Set([
	"black",
	"red",
	"green",
	"yellow",
	"blue",
	"magenta",
	"cyan",
	"gray",
	"darkGray",
	"lightRed",
	"lightGreen",
	"lightYellow",
	"lightBlue",
	"lightMagenta",
	"lightCyan",
	"white",
]);

export function semanticStyleFor(
	style: StyleRef | StyleSpec | StyleSpecValue,
): SemanticStyle {
	const candidate = style as StyleRef & {
		readonly kind?: string;
		readonly local?: StyleSpec;
		readonly value?: StyleSpecValue;
	};
	const value =
		candidate.kind === "style-ref"
			? candidate.local!.value
			: candidate.kind === "style"
				? candidate.value!
				: (style as StyleSpecValue);
	return Object.freeze({
		...(value.foreground === undefined
			? {}
			: { foreground: semanticColorFor(value.foreground) }),
		...(value.background === undefined
			? {}
			: { background: semanticColorFor(value.background) }),
		attributes: Object.freeze({ ...value.attributes }),
	});
}
