/** Opaque semantic key resolved by a Theme. */
export class ThemeKey {
  readonly kind = "theme-key" as const;

  constructor(readonly value: string) {
    if (value.length === 0) throw new RangeError("theme key cannot be empty");
  }
}
