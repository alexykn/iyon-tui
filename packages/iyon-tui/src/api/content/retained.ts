import { tuiError, asTuiError } from "../errors.ts";
import { FrameworkHandle } from "../controls/framework-handle.ts";
import { runtimeResourceRegistry } from "../../transport/native/resource-registry.ts";
import { runtimeEnvironment } from "../../runtime/environment.ts";
import {
	appendTextSource,
	clearTextSource,
	replaceTextSource,
	sealTextSource,
	truncateTextSource,
	decodeSemanticStylePayload,
} from "../../transport/content/ffi.ts";
import { nativeResourceOf } from "../../transport/native/resources.ts";
import {
	createTextSource,
	type NativeTextSourceContract,
} from "../../transport/content/control.ts";
import type { TextContent } from "./text-content.ts";
import type { SemanticTextStyle } from "./annotations.ts";

export type ContentFamily = "text";

export interface TextRetentionPolicy {
	readonly maxBytes?: number;
	readonly maxLines?: number;
	readonly overflow: "drop-oldest" | "error";
}

export interface TextSourceOptions {
	readonly retention?: TextRetentionPolicy;
}

export type TextSourceAnnotationKind = "tag" | "style" | "atomic" | "point";
export type { SemanticTextStyle } from "./annotations.ts";

/** Fixed-envelope Source annotation input. Ranges are operation-local UTF-8 bytes. */
export interface TextSourceAnnotation {
	readonly kind?: TextSourceAnnotationKind;
	readonly startByte?: number;
	readonly endByte?: number;
	readonly namespace?: string;
	readonly name?: string;
	readonly style?: SemanticTextStyle;
	readonly payload?: Uint8Array;
}

export interface TextSourceMutation {
	readonly revision: bigint;
	readonly environmentWakeEpoch: bigint;
	readonly scheduleEnvironmentDrain: boolean;
}

export interface TextSourceAnnotationSnapshot {
	readonly kind: number;
	readonly flags: number;
	readonly startByte: bigint;
	readonly endByte: bigint;
	readonly payload: Uint8Array;
	readonly aux0: number;
	readonly aux1: number;
	readonly style?: SemanticTextStyle;
}

export interface TextSourceSnapshot {
	readonly sourceId: bigint;
	readonly sourceGeneration: number;
	readonly contentGeneration: bigint;
	readonly revision: bigint;
	readonly sourceBase: bigint;
	readonly sourceEnd: bigint;
	readonly sealed: boolean;
	readonly headPartial: boolean;
	readonly text: string;
	readonly annotations: readonly TextSourceAnnotationSnapshot[];
}

export interface TextSourceStats {
	readonly revision: bigint;
	readonly sourceBase: bigint;
	readonly sourceEnd: bigint;
	readonly retainedBytes: bigint;
	readonly retainedLines: bigint;
	readonly chunkCount: number;
	readonly sealed: boolean;
	readonly headPartial: boolean;
	readonly acceptedBytes: bigint;
	readonly copiedBytes: bigint;
	readonly droppedHeadBytes: bigint;
}

export interface ContentPortOptions {
	readonly family?: ContentFamily;
}

export type TextFunnelWrap = "word" | "grapheme" | "noWrap";

export type TextFunnelKind = "plain" | "markdown" | "diff" | "ansi";

export interface TextSmoothOptions {
	readonly tickIntervalMs?: number;
	readonly spring?: number;
	readonly minUnitsPerSecond?: number;
	readonly maxUnitsPerSecond?: number;
}

export interface TextFunnelOptions {
	readonly wrap?: TextFunnelWrap;
	readonly smooth?: boolean | TextSmoothOptions;
	readonly hyperlinks?: boolean;
}

export interface TextFunnelDelivery {
	readonly kind: "immediate" | "smooth";
	readonly options?: Readonly<TextSmoothOptions>;
}

export interface Funnel<TContent = TextContent> {
	readonly kind: "text-funnel";
	readonly family: ContentFamily;
	/** Phantom content family marker; a Funnel carries no Source data. */
	readonly __content?: TContent;
}

export type ContentSource = TextStreamSource | TextBlockSource;
export type Source<TContent = TextContent> = ContentSource & {
	readonly content?: TContent;
};

export type ContentConnectorPhase =
	| "idle"
	| "waiting-for-mount"
	| "activation-pending"
	| "active"
	| "failed"
	| "disposing"
	| "disposed"
	| "blocked-geometry"
	| "unsupported-backend";

export interface ContentConnectorError {
	readonly code: string;
	readonly diagnostic: string;
}

export interface ContentConnectorStatus {
	readonly phase: ContentConnectorPhase;
	readonly requested: boolean;
	readonly visible: boolean;
	readonly projectedSourceRevision?: bigint;
	readonly error?: ContentConnectorError;
}

interface SourceOwner {
	readonly environment: object;
}

function validateTextSourceOptions(options: TextSourceOptions): void {
	if (typeof options !== "object" || options === null) {
		throw new TypeError("text source options must be an object");
	}
	for (const key of Object.keys(options)) {
		if (key !== "retention")
			throw new RangeError(`unknown text source option ${JSON.stringify(key)}`);
	}
	const retention = options.retention;
	if (retention === undefined) return;
	if (typeof retention !== "object" || retention === null) {
		throw new TypeError("text source retention must be an object");
	}
	if (retention.maxBytes === undefined && retention.maxLines === undefined) {
		throw new RangeError("text source retention requires maxBytes or maxLines");
	}
	for (const key of Object.keys(retention)) {
		if (key !== "maxBytes" && key !== "maxLines" && key !== "overflow") {
			throw new RangeError(
				`unknown text source retention option ${JSON.stringify(key)}`,
			);
		}
	}
	for (const [name, value] of [
		["maxBytes", retention.maxBytes],
		["maxLines", retention.maxLines],
	] as const) {
		if (value !== undefined && (!Number.isSafeInteger(value) || value <= 0)) {
			throw new RangeError(
				`text source retention ${name} must be a positive safe integer`,
			);
		}
	}
	if (retention.overflow !== "drop-oldest" && retention.overflow !== "error") {
		throw new RangeError(
			"text source retention overflow must be drop-oldest or error",
		);
	}
}

function sourceWake(): void {
	// Source payload mutation is serviced by the native environment; TypeScript
	// does not run a frame or wake-broker drain.
}

function validateSourceMutation(
	text: string,
	annotations: readonly TextSourceAnnotation[],
): void {
	if (typeof text !== "string")
		throw new TypeError("Source text must be a string");
	if (!Array.isArray(annotations))
		throw new TypeError("Source annotations must be an array");
}

const MAX_SOURCE_U64 = 0xffff_ffff_ffff_ffffn;

function sourceBigInt(value: unknown, name: string): bigint {
	let normalized: bigint;
	if (typeof value === "bigint") {
		normalized = value;
	} else if (typeof value === "number") {
		if (!Number.isSafeInteger(value)) {
			throw tuiError("runtime", `native Source ${name} is not a valid u64`);
		}
		normalized = BigInt(value);
	} else if (typeof value === "string" && /^\d+$/u.test(value)) {
		normalized = BigInt(value);
	} else {
		throw tuiError("runtime", `native Source ${name} is not a valid u64`);
	}
	if (normalized < 0n || normalized > MAX_SOURCE_U64) {
		throw tuiError("runtime", `native Source ${name} is not a valid u64`);
	}
	return normalized;
}

interface NativeSourceSnapshot {
	readonly sourceId: string | number;
	readonly sourceGeneration: number;
	readonly contentGeneration: string | number;
	readonly revision: string | number;
	readonly sourceBase: string | number;
	readonly sourceEnd: string | number;
	readonly sealed: boolean;
	readonly headPartial: boolean;
	readonly text: string;
	readonly annotations: readonly {
		readonly kind: number;
		readonly flags: number;
		readonly startByte: string | number;
		readonly endByte: string | number;
		readonly payload?: readonly number[];
		readonly aux0: number;
		readonly aux1: number;
	}[];
}

interface NativeSourceStats {
	readonly revision: string | number;
	readonly sourceBase: string | number;
	readonly sourceEnd: string | number;
	readonly retainedBytes: string | number;
	readonly retainedLines: string | number;
	readonly chunkCount: number;
	readonly sealed: boolean;
	readonly headPartial: boolean;
	readonly acceptedBytes: string | number;
	readonly copiedBytes: string | number;
	readonly droppedHeadBytes: string | number;
}

function sourceSnapshot(
	resource: NativeTextSourceContract,
): TextSourceSnapshot {
	const native = resource.snapshot() as NativeSourceSnapshot;
	return {
		sourceId: sourceBigInt(native.sourceId, "source id"),
		sourceGeneration: sourceGenerationValue(native.sourceGeneration),
		contentGeneration: sourceBigInt(
			native.contentGeneration,
			"content generation",
		),
		revision: sourceBigInt(native.revision, "revision"),
		sourceBase: sourceBigInt(native.sourceBase, "source base"),
		sourceEnd: sourceBigInt(native.sourceEnd, "source end"),
		sealed: native.sealed,
		headPartial: native.headPartial,
		text: native.text,
		annotations: native.annotations.map((annotation) => {
			const payload = Uint8Array.from(annotation.payload ?? []);
			return {
				kind: annotation.kind,
				flags: annotation.flags,
				startByte: sourceBigInt(annotation.startByte, "annotation start"),
				endByte: sourceBigInt(annotation.endByte, "annotation end"),
				payload,
				aux0: annotation.aux0,
				aux1: annotation.aux1,
				...(annotation.kind === 2
					? { style: decodeSemanticStylePayload(payload) }
					: {}),
			};
		}),
	};
}

function sourceStats(resource: NativeTextSourceContract): TextSourceStats {
	const native = resource.stats() as NativeSourceStats;
	return {
		revision: sourceBigInt(native.revision, "revision"),
		sourceBase: sourceBigInt(native.sourceBase, "source base"),
		sourceEnd: sourceBigInt(native.sourceEnd, "source end"),
		retainedBytes: sourceBigInt(native.retainedBytes, "retained bytes"),
		retainedLines: sourceBigInt(native.retainedLines, "retained lines"),
		chunkCount: native.chunkCount,
		sealed: native.sealed,
		headPartial: native.headPartial,
		acceptedBytes: sourceBigInt(native.acceptedBytes, "accepted bytes"),
		copiedBytes: sourceBigInt(native.copiedBytes, "copied bytes"),
		droppedHeadBytes: sourceBigInt(
			native.droppedHeadBytes,
			"dropped head bytes",
		),
	};
}

function sourceId(resource: NativeTextSourceContract): bigint {
	return sourceBigInt(resource.sourceId(), "source id");
}

function sourceGenerationValue(value: unknown): number {
	if (
		typeof value !== "number" ||
		!Number.isSafeInteger(value) ||
		value < 0 ||
		value > 0xffff_ffff
	) {
		throw tuiError("runtime", "native Source generation is invalid");
	}
	return value;
}

function sourceGeneration(resource: NativeTextSourceContract): number {
	return sourceGenerationValue(resource.sourceGeneration());
}

/** Environment-owned text Source identity. Payload mutation arrives in E. */
export class TextStreamSource extends FrameworkHandle<"source"> {
	private constructor(resource: object, owner: SourceOwner) {
		try {
			super("source", resource as never, { owner });
		} catch (error) {
			try {
				(resource as NativeTextSourceContract).dispose();
			} catch (cleanupError) {
				throw new AggregateError(
					[error, cleanupError],
					"Text Source registration cleanup failed",
				);
			}
			throw error;
		}
	}

	static create(options: TextSourceOptions = {}): TextStreamSource {
		validateTextSourceOptions(options);
		return new TextStreamSource(createTextSource("stream", options), {
			environment: runtimeResourceRegistry().environment,
		});
	}

	sourceId(): bigint {
		return this.call(() => sourceId(this.nativeAs<NativeTextSourceContract>()));
	}
	sourceGeneration(): number {
		return this.call(() =>
			sourceGeneration(this.nativeAs<NativeTextSourceContract>()),
		);
	}
	environmentSlot(): number {
		return this.call(() =>
			this.nativeAs<NativeTextSourceContract>().environmentSlot(),
		);
	}
	environmentGeneration(): number {
		return this.call(() =>
			this.nativeAs<NativeTextSourceContract>().environmentGeneration(),
		);
	}
	contentGeneration(): bigint {
		return this.call(() =>
			sourceBigInt(
				this.nativeAs<NativeTextSourceContract>().contentGeneration(),
				"content generation",
			),
		);
	}
	snapshot(): TextSourceSnapshot {
		return this.call(() =>
			sourceSnapshot(this.nativeAs<NativeTextSourceContract>()),
		);
	}
	stats(): TextSourceStats {
		return this.call(() =>
			sourceStats(this.nativeAs<NativeTextSourceContract>()),
		);
	}
	append(
		text: string,
		annotations: readonly TextSourceAnnotation[] = [],
	): TextSourceMutation {
		return this.call(() => {
			validateSourceMutation(text, annotations);
			return appendTextSource(
				this.nativeAs<NativeTextSourceContract>(),
				text,
				annotations,
				sourceWake,
			);
		});
	}
	appendUtf8(
		text: string,
		annotations: readonly TextSourceAnnotation[] = [],
	): TextSourceMutation {
		return this.append(text, annotations);
	}
	replace(
		text: string,
		annotations: readonly TextSourceAnnotation[] = [],
	): TextSourceMutation {
		return this.call(() => {
			validateSourceMutation(text, annotations);
			return replaceTextSource(
				this.nativeAs<NativeTextSourceContract>(),
				text,
				annotations,
				sourceWake,
			);
		});
	}
	replaceUtf8(
		text: string,
		annotations: readonly TextSourceAnnotation[] = [],
	): TextSourceMutation {
		return this.replace(text, annotations);
	}
	clear(): TextSourceMutation {
		return this.call(() =>
			clearTextSource(this.nativeAs<NativeTextSourceContract>(), sourceWake),
		);
	}
	seal(): TextSourceMutation {
		return this.call(() =>
			sealTextSource(this.nativeAs<NativeTextSourceContract>(), sourceWake),
		);
	}
	truncateHead(offset: bigint | number): TextSourceMutation {
		return this.call(() =>
			truncateTextSource(
				this.nativeAs<NativeTextSourceContract>(),
				offset,
				sourceWake,
			),
		);
	}
}

/** Environment-owned replacement-style text Source identity. */
export class TextBlockSource extends FrameworkHandle<"source"> {
	private constructor(resource: object, owner: SourceOwner) {
		try {
			super("source", resource as never, { owner });
		} catch (error) {
			try {
				(resource as NativeTextSourceContract).dispose();
			} catch (cleanupError) {
				throw new AggregateError(
					[error, cleanupError],
					"Text Source registration cleanup failed",
				);
			}
			throw error;
		}
	}

	static create(options: TextSourceOptions = {}): TextBlockSource {
		validateTextSourceOptions(options);
		return new TextBlockSource(createTextSource("block", options), {
			environment: runtimeResourceRegistry().environment,
		});
	}

	sourceId(): bigint {
		return this.call(() => sourceId(this.nativeAs<NativeTextSourceContract>()));
	}
	sourceGeneration(): number {
		return this.call(() =>
			sourceGeneration(this.nativeAs<NativeTextSourceContract>()),
		);
	}
	environmentSlot(): number {
		return this.call(() =>
			this.nativeAs<NativeTextSourceContract>().environmentSlot(),
		);
	}
	environmentGeneration(): number {
		return this.call(() =>
			this.nativeAs<NativeTextSourceContract>().environmentGeneration(),
		);
	}
	contentGeneration(): bigint {
		return this.call(() =>
			sourceBigInt(
				this.nativeAs<NativeTextSourceContract>().contentGeneration(),
				"content generation",
			),
		);
	}
	snapshot(): TextSourceSnapshot {
		return this.call(() =>
			sourceSnapshot(this.nativeAs<NativeTextSourceContract>()),
		);
	}
	stats(): TextSourceStats {
		return this.call(() =>
			sourceStats(this.nativeAs<NativeTextSourceContract>()),
		);
	}
	replace(
		text: string,
		annotations: readonly TextSourceAnnotation[] = [],
	): TextSourceMutation {
		return this.call(() => {
			validateSourceMutation(text, annotations);
			return replaceTextSource(
				this.nativeAs<NativeTextSourceContract>(),
				text,
				annotations,
				sourceWake,
			);
		});
	}
	replaceUtf8(
		text: string,
		annotations: readonly TextSourceAnnotation[] = [],
	): TextSourceMutation {
		return this.replace(text, annotations);
	}
	clear(): TextSourceMutation {
		return this.call(() =>
			clearTextSource(this.nativeAs<NativeTextSourceContract>(), sourceWake),
		);
	}
	truncateHead(offset: bigint | number): TextSourceMutation {
		return this.call(() =>
			truncateTextSource(
				this.nativeAs<NativeTextSourceContract>(),
				offset,
				sourceWake,
			),
		);
	}
}

/** Immutable, Source-neutral text transformation and delivery configuration. */
export class TextFunnel implements Funnel<TextContent> {
	readonly kind = "text-funnel" as const;
	readonly family = "text" as const;
	readonly mode: TextFunnelKind;
	readonly wrap: TextFunnelWrap;
	readonly delivery: TextFunnelDelivery;
	readonly hyperlinks: boolean;

	private constructor(mode: TextFunnelKind, options: TextFunnelOptions) {
		this.mode = mode;
		this.wrap = options.wrap ?? "word";
		if (
			this.wrap !== "word" &&
			this.wrap !== "grapheme" &&
			this.wrap !== "noWrap"
		) {
			throw new RangeError("text Funnel wrap mode is invalid");
		}
		this.hyperlinks = options.hyperlinks ?? true;
		if (typeof this.hyperlinks !== "boolean")
			throw new TypeError("text Funnel hyperlinks must be boolean");
		this.delivery = deliveryFor(options.smooth);
		Object.freeze(this.delivery);
		Object.freeze(this);
	}

	static plain(options: TextFunnelOptions = {}): TextFunnel {
		return TextFunnel.create("plain", options);
	}
	static markdown(options: TextFunnelOptions = {}): TextFunnel {
		return TextFunnel.create("markdown", options);
	}
	static diff(options: TextFunnelOptions = {}): TextFunnel {
		return TextFunnel.create("diff", options);
	}
	static ansi(options: TextFunnelOptions = {}): TextFunnel {
		return TextFunnel.create("ansi", options);
	}

	/** Returns a new Funnel with Connector-local native smoothing enabled. */
	smooth(options: TextSmoothOptions = {}): TextFunnel {
		return TextFunnel.create(this.mode, {
			wrap: this.wrap,
			hyperlinks: this.hyperlinks,
			smooth: options,
		});
	}

	immediate(): TextFunnel {
		return TextFunnel.create(this.mode, {
			wrap: this.wrap,
			hyperlinks: this.hyperlinks,
			smooth: false,
		});
	}

	private static create(
		mode: TextFunnelKind,
		options: TextFunnelOptions,
	): TextFunnel {
		if (typeof options !== "object" || options === null)
			throw new TypeError("text Funnel options must be an object");
		for (const key of Object.keys(options)) {
			if (key !== "wrap" && key !== "smooth" && key !== "hyperlinks") {
				throw new RangeError(
					`unknown text Funnel option ${JSON.stringify(key)}`,
				);
			}
		}
		if (
			options.hyperlinks !== undefined &&
			typeof options.hyperlinks !== "boolean"
		) {
			throw new TypeError("text Funnel hyperlinks must be boolean");
		}
		return new TextFunnel(mode, options);
	}
}

function deliveryFor(
	value: boolean | TextSmoothOptions | undefined,
): TextFunnelDelivery {
	if (value === undefined || value === false) return { kind: "immediate" };
	if (value === true) return { kind: "smooth", options: {} };
	if (typeof value !== "object" || value === null)
		throw new TypeError("text Funnel smooth must be boolean or an object");
	for (const key of Object.keys(value)) {
		if (
			key !== "tickIntervalMs" &&
			key !== "spring" &&
			key !== "minUnitsPerSecond" &&
			key !== "maxUnitsPerSecond"
		) {
			throw new RangeError(`unknown Smooth option ${JSON.stringify(key)}`);
		}
	}
	const options = { ...value };
	for (const [name, candidate] of Object.entries(options)) {
		if (candidate === undefined) continue;
		if (
			typeof candidate !== "number" ||
			!Number.isFinite(candidate) ||
			candidate < 0
		) {
			throw new RangeError(
				`Smooth option ${name} must be a finite non-negative number`,
			);
		}
	}
	if (
		options.tickIntervalMs !== undefined &&
		(!Number.isInteger(options.tickIntervalMs) || options.tickIntervalMs === 0)
	) {
		throw new RangeError(
			"Smooth option tickIntervalMs must be a positive integer",
		);
	}
	if (
		options.minUnitsPerSecond !== undefined &&
		options.maxUnitsPerSecond !== undefined &&
		options.minUnitsPerSecond > options.maxUnitsPerSecond
	) {
		throw new RangeError("Smooth minimum rate cannot exceed maximum rate");
	}
	return { kind: "smooth", options: Object.freeze(options) };
}
