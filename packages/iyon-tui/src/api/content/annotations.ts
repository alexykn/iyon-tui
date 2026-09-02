import type { TextAttribute } from "../presentation/style.ts";
import type { ColorSpec } from "../presentation/theme.ts";

export interface SemanticTag { readonly namespace: string; readonly name: string; }
export type SemanticValue = string | number | boolean | null;

/** Host-independent style intent carried by a Source annotation. */
export interface SemanticTextStyle {
  readonly role?: string;
  readonly foreground?: ColorSpec;
  readonly background?: ColorSpec;
  readonly attributes?: Readonly<Partial<Record<TextAttribute, boolean>>>;
}

/** Closed v1 Source annotation schema and head-retention semantics. */
export const TEXT_SOURCE_ANNOTATION_SCHEMA = Object.freeze([
  Object.freeze({ kind: "tag", truncation: "clip", payload: "namespace\0name" }),
  Object.freeze({ kind: "style", truncation: "clip", payload: "semantic-text-style-v1" }),
  Object.freeze({ kind: "atomic", truncation: "drop", payload: "opaque-bytes" }),
  Object.freeze({ kind: "point", truncation: "point", payload: "opaque-bytes" }),
] as const);

export class Annotations {
  readonly kind = "annotations" as const;
  constructor(readonly tags: readonly SemanticTag[] = [], readonly properties: Readonly<Record<string, SemanticValue>> = {}) {}
  withTag(tag: SemanticTag): Annotations { return new Annotations([...this.tags, { ...tag }], this.properties); }
  withProperty(key: string, value: SemanticValue): Annotations { return new Annotations(this.tags, { ...this.properties, [key]: value }); }
  containsTag(tag: SemanticTag): boolean { return this.tags.some((item) => item.namespace === tag.namespace && item.name === tag.name); }
}
