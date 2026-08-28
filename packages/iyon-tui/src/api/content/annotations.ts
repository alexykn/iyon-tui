export interface SemanticTag { readonly namespace: string; readonly name: string; }
export type SemanticValue = string | number | boolean | null;

export class Annotations {
  readonly kind = "annotations" as const;
  constructor(readonly tags: readonly SemanticTag[] = [], readonly properties: Readonly<Record<string, SemanticValue>> = {}) {}
  withTag(tag: SemanticTag): Annotations { return new Annotations([...this.tags, { ...tag }], this.properties); }
  withProperty(key: string, value: SemanticValue): Annotations { return new Annotations(this.tags, { ...this.properties, [key]: value }); }
  containsTag(tag: SemanticTag): boolean { return this.tags.some((item) => item.namespace === tag.namespace && item.name === tag.name); }
}
