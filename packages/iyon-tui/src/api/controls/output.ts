/** Opaque typed output-channel identity used by generic component routing. */
export class Output<T> {
  #outputBrand!: void;
  /** Type-only variance marker keeps channels for different payloads distinct. */
  declare private readonly outputType: (value: T) => T;
  readonly kind = "output" as const;
  private constructor() {}
}
