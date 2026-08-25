import type { InsetsNode } from "../ir.ts";

export class Insets {
  readonly kind = "insets" as const;

  private constructor(readonly value: InsetsNode) {}

  static all(value: number): Insets {
    return Insets.of(value, value, value, value);
  }

  static vertical(value: number): Insets {
    return Insets.of(value, 0, value, 0);
  }

  static horizontal(value: number): Insets {
    return Insets.of(0, value, 0, value);
  }

  static of(top: number, right: number, bottom: number, left: number): Insets {
    for (const [name, part] of Object.entries({ top, right, bottom, left })) {
      if (!Number.isInteger(part) || part < 0 || part > 65535) {
        throw new RangeError(`inset ${name} must be an integer from 0 to 65535`);
      }
    }
    return new Insets({ top, right, bottom, left });
  }
}

export function insets(value: number | Insets | InsetsNode): InsetsNode {
  if (typeof value === "number") {
    return Insets.all(value).value;
  }
  return "value" in value ? { ...value.value } : { ...value };
}
