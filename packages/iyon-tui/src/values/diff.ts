import { View } from "./view.ts";
import type { DiffHunkNode } from "../ir.ts";

export type DiffLineKind = "context" | "addition" | "deletion";
export type DiffLineTermination = "lf" | "crlf" | "none";

export class DiffRange {
  readonly kind = "diff-range" as const;

  constructor(readonly start: number, readonly lineCount: number) {
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(lineCount) || start < 0 || lineCount < 0 || start + lineCount > Number.MAX_SAFE_INTEGER) {
      throw new RangeError("invalid diff range");
    }
  }

  isEmpty(): boolean { return this.lineCount === 0; }
  end(): number { return this.start + this.lineCount; }
}

export class DiffLine {
  readonly kind = "diff-line" as const;

  constructor(
    readonly lineKind: DiffLineKind,
    readonly text: string,
    readonly termination: DiffLineTermination = "lf",
    readonly coordinates: { readonly oldLine?: number; readonly newLine?: number } = {},
  ) {
    if (coordinates.oldLine !== undefined) validateLineNumber(coordinates.oldLine, "oldLine");
    if (coordinates.newLine !== undefined) validateLineNumber(coordinates.newLine, "newLine");
  }

  get oldLine(): number | undefined { return this.coordinates.oldLine; }
  get newLine(): number | undefined { return this.coordinates.newLine; }

  static context(oldLine: number, newLine: number, text: string, termination: DiffLineTermination = "lf"): DiffLine {
    return new DiffLine("context", text, termination, { oldLine, newLine });
  }
  static addition(newLine: number, text: string, termination: DiffLineTermination = "lf"): DiffLine {
    return new DiffLine("addition", text, termination, { newLine });
  }
  static deletion(oldLine: number, text: string, termination: DiffLineTermination = "lf"): DiffLine {
    return new DiffLine("deletion", text, termination, { oldLine });
  }
}

export class DiffHunk {
  readonly kind = "diff-hunk" as const;

  constructor(
    readonly oldRange: DiffRange,
    readonly newRange: DiffRange,
    readonly lines: readonly DiffLine[] = [],
  ) {
    this.validate();
  }

  validate(): void {
    let oldConsumed = 0;
    let newConsumed = 0;
    let oldLine = this.oldRange.lineCount === 0 ? undefined : this.oldRange.start + 1;
    let newLine = this.newRange.lineCount === 0 ? undefined : this.newRange.start + 1;
    for (const line of this.lines) {
      const expectedOld = line.lineKind === "addition" ? undefined : oldLine;
      const expectedNew = line.lineKind === "deletion" ? undefined : newLine;
      if (line.oldLine !== undefined && line.oldLine !== expectedOld) throw new RangeError("diff line old coordinate does not match its hunk");
      if (line.newLine !== undefined && line.newLine !== expectedNew) throw new RangeError("diff line new coordinate does not match its hunk");
      if (line.lineKind !== "addition") { oldConsumed += 1; if (oldLine !== undefined) oldLine += 1; }
      if (line.lineKind !== "deletion") { newConsumed += 1; if (newLine !== undefined) newLine += 1; }
    }
    const expectedOld = this.oldRange.lineCount;
    const expectedNew = this.newRange.lineCount;
    if (oldConsumed !== expectedOld || newConsumed !== expectedNew) {
      throw new RangeError(`diff hunk consumed old ${oldConsumed}/${expectedOld} and new ${newConsumed}/${expectedNew} lines`);
    }
  }

  render(): View {
    return new DiffRenderer().render(this);
  }
}

/** Semantic diff renderer. Rust lowers the diff node into the themed View. */
export class DiffRenderer {
  render(hunks: DiffHunk | readonly DiffHunk[]): View {
    const values = Array.isArray(hunks) ? hunks : [hunks];
    return View.diff(values.map(toNode));
  }

  renderHunk(hunk: DiffHunk): View {
    return this.render(hunk);
  }
}

function validateLineNumber(value: number, name: string): void {
  if (!Number.isSafeInteger(value) || value < 1) throw new RangeError(`${name} must be a positive safe integer`);
}

function toNode(hunk: DiffHunk): DiffHunkNode {
  let oldLine = hunk.oldRange.start + 1;
  let newLine = hunk.newRange.start + 1;
  const lines = hunk.lines.map((line) => {
    const node = {
      kind: line.lineKind,
      text: line.text,
      termination: line.termination === "none" ? "unterminated" : "terminated",
      ...(line.lineKind === "context" ? { oldLine: line.oldLine ?? oldLine, newLine: line.newLine ?? newLine } : {}),
      ...(line.lineKind === "addition" ? { newLine: line.newLine ?? newLine } : {}),
      ...(line.lineKind === "deletion" ? { oldLine: line.oldLine ?? oldLine } : {}),
    } as const;
    if (line.lineKind !== "addition") oldLine += 1;
    if (line.lineKind !== "deletion") newLine += 1;
    return node;
  });
  return {
    oldRange: { start: hunk.oldRange.start, count: hunk.oldRange.lineCount },
    newRange: { start: hunk.newRange.start, count: hunk.newRange.lineCount },
    lines,
  };
}
