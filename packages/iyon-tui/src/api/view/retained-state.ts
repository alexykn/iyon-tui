import { tuiError } from "../errors.ts";
import { FrameworkHandle } from "../controls/framework-handle.ts";
import type {
  BorderEdges,
  BorderGlyphs,
  BorderStyle,
  StyleRef,
  StyleSpec,
  StyleStateKey,
  StyleStateValue,
  TextAttribute,
} from "../presentation/style.ts";
import { validateTextAttribute } from "../presentation/style.ts";
import type { ColorSpec } from "../presentation/theme.ts";
import type { Insets, InsetsValue } from "./geometry.ts";
import { SEMANTIC_VIEW_KIND } from "./semantic-node.ts";
import {
  normalizeClearGeometryProperties,
  normalizeClearProperties,
  normalizeGeometryPatch,
  normalizePresentationPatch,
} from "../../transport/state/control.ts";

interface NativeStateWake {
  readonly schedule_environment_drain: boolean;
}

interface NativeViewStateResource {
  dispose(): void;
  validateNodeKind(targetNodeKind: number): void;
  setGeometry(patch: object): NativeStateWake;
  clearGeometry(properties?: readonly string[]): NativeStateWake;
  setPresentation(patch: object): NativeStateWake;
  clearPresentation(properties?: readonly string[]): NativeStateWake;
  setStyleState(key: string, value: string): NativeStateWake;
  clearStyleState(key: string): NativeStateWake;
}

/** Geometry fields whose retained override can be removed independently. */
export type ViewStateGeometryProperty =
  | "width"
  | "height"
  | "padding"
  | "minWidth"
  | "maxWidth"
  | "minHeight"
  | "maxHeight"
  | "gap"
  | "alignment"
  | "borderEdges";

/** Explicit edge presence for a retained one-cell border. */
export interface ViewStateBorderEdges {
  readonly top: boolean;
  readonly right: boolean;
  readonly bottom: boolean;
  readonly left: boolean;
}

/** Alignment owned by a physical node. `start`/`center`/`end` are the
 * shorthand horizontal forms; `top`/`bottom` are vertical forms. Use the
 * object form for a vertical `center` or to name both axes explicitly. */
export type ViewStateAlignment =
  | "start"
  | "center"
  | "end"
  | "top"
  | "bottom"
  | {
    readonly horizontal?: "start" | "center" | "end";
    readonly vertical?: "top" | "center" | "bottom";
  };

export interface ViewStateGeometryPatch {
  readonly width?: "fit" | "fill";
  readonly height?: "fit" | "fill";
  readonly padding?: number | Insets | InsetsValue;
  readonly minWidth?: number | null;
  readonly maxWidth?: number | null;
  readonly minHeight?: number | null;
  readonly maxHeight?: number | null;
  readonly gap?: number;
  readonly alignment?: ViewStateAlignment;
  readonly borderEdges?: BorderEdges | ViewStateBorderEdges | null;
}

/** Presentation fields whose retained override can be removed independently. */
export type ViewStatePresentationProperty =
  | "foreground"
  | "background"
  | "borderColor"
  | "borderStyle"
  | "borderGlyphs"
  | "textAttributes"
  | "style";

export interface ViewStateTextAttributes {
  readonly bold?: boolean;
  readonly dim?: boolean;
  readonly italic?: boolean;
  readonly underline?: boolean;
  readonly reversed?: boolean;
  readonly strikethrough?: boolean;
}

/** Typed presentation-only retained state for one attached View occurrence. */
export interface ViewStatePresentationPatch {
  readonly foreground?: ColorSpec | null;
  readonly background?: ColorSpec | null;
  readonly borderColor?: ColorSpec | null;
  readonly borderStyle?: BorderStyle | null;
  readonly borderGlyphs?: BorderGlyphs | null;
  readonly textAttributes?: ViewStateTextAttributes;
  readonly style?: StyleRef | StyleSpec | null;
}

/** Semantic physical node-kind table; property capability is checked natively. */
export const VIEW_STATE_NODE_KINDS: ReadonlySet<number> = new Set([
  SEMANTIC_VIEW_KIND.text,
  SEMANTIC_VIEW_KIND.diff,
  SEMANTIC_VIEW_KIND.spacer,
  SEMANTIC_VIEW_KIND.row,
  SEMANTIC_VIEW_KIND.column,
  SEMANTIC_VIEW_KIND.grid,
  SEMANTIC_VIEW_KIND.hanging,
  SEMANTIC_VIEW_KIND.container,
  SEMANTIC_VIEW_KIND.clamp,
  SEMANTIC_VIEW_KIND.contentMax,
  SEMANTIC_VIEW_KIND.decorated,
  SEMANTIC_VIEW_KIND.contentHost,
]);

interface ViewStateOptions {
  readonly owner: { readonly environment: object; readonly host?: object };
  readonly requestWake: () => void;
  readonly assertMutationAllowed: () => void;
  readonly acceptedNodeKinds?: ReadonlySet<number>;
}

/**
 * Host-owned retained geometry and presentation overrides. The object is
 * intentionally independent of View construction; `.state()` only puts its
 * opaque HandleId into the semantic attachment record.
 */
export class ViewState extends FrameworkHandle<"state"> {
  readonly kind = "state" as const;
  private readonly requestWake: () => void;
  private readonly assertMutationAllowed: () => void;

  private constructor(resource: object, options: ViewStateOptions) {
    super("state", resource as never, {
      owner: options.owner,
      acceptedNodeKinds: options.acceptedNodeKinds,
    });
    this.requestWake = options.requestWake;
    this.assertMutationAllowed = options.assertMutationAllowed;
  }

  setGeometry(patch: ViewStateGeometryPatch): void {
    this.assertMutationAllowed();
    const normalized = normalizeGeometryPatch(patch);
    this.mutate((native) => native.setGeometry(normalized));
  }

  clearGeometry(
    ...properties: readonly (ViewStateGeometryProperty | readonly ViewStateGeometryProperty[])[]
  ): void {
    this.assertMutationAllowed();
    const requested = properties.length === 0
      ? undefined
      : properties.flatMap((property) => Array.isArray(property) ? [...property] : [property]) as readonly ViewStateGeometryProperty[];
    const normalized = normalizeClearGeometryProperties(requested);
    this.mutate((native) => normalized === undefined
      ? native.clearGeometry()
      : native.clearGeometry(normalized));
  }

  setPresentation(patch: ViewStatePresentationPatch): void {
    this.assertMutationAllowed();
    const normalized = normalizePresentationPatch(patch);
    this.mutate((native) => native.setPresentation(normalized));
  }

  clearPresentation(
    ...properties: readonly (ViewStatePresentationProperty | readonly ViewStatePresentationProperty[])[]
  ): void {
    this.assertMutationAllowed();
    const requested = properties.length === 0
      ? undefined
      : properties.flatMap((property) => Array.isArray(property) ? [...property] : [property]) as readonly ViewStatePresentationProperty[];
    const normalized = normalizeClearProperties(requested);
    this.mutate((native) => normalized === undefined
      ? native.clearPresentation()
      : native.clearPresentation(normalized));
  }

  setStyleState(key: string | StyleStateKey, value: string | StyleStateValue): void {
    this.assertMutationAllowed();
    const normalizedKey = stateText(key, "style state key");
    const normalizedValue = stateText(value, "style state value");
    this.mutate((native) => native.setStyleState(normalizedKey, normalizedValue));
  }

  clearStyleState(key: string | StyleStateKey): void {
    this.assertMutationAllowed();
    const normalizedKey = stateText(key, "style state key");
    this.mutate((native) => native.clearStyleState(normalizedKey));
  }

  /** @internal Constructs a wrapper around a Tui-created native state. */
  static create(resource: object, options: ViewStateOptions): ViewState {
    return new ViewState(resource, options);
  }

  private mutate(operation: (native: NativeViewStateResource) => NativeStateWake): void {
    const wake = this.call(() => operation(this.nativeAs<NativeViewStateResource>()));
    if (wake.schedule_environment_drain) this.requestWake();
  }
}

/** @internal Constructs a Tui-owned ViewState from its native host resource. */
export function createViewState(
  resource: object,
  owner: { readonly environment: object; readonly host?: object },
  requestWake: () => void,
  assertMutationAllowed: () => void,
): ViewState {
  return ViewState.create(resource, {
    owner,
    requestWake,
    assertMutationAllowed,
    acceptedNodeKinds: VIEW_STATE_NODE_KINDS,
  });
}

function stateText(value: string | StyleStateKey | StyleStateValue, label: string): string {
  const text = typeof value === "string" ? value : value.value;
  if (typeof text !== "string" || text.length === 0) throw tuiError("validation", `${label} cannot be empty`);
  return text;
}
