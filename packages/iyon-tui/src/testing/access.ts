type TestingInputEvent =
  | { readonly type: "key"; readonly key: string; readonly modifiers?: readonly string[] }
  | { readonly type: "paste"; readonly text: string }
  | { readonly type: "resize"; readonly width: number; readonly height: number };

type TuiTestingAccess = {
  flush(): void;
  enqueue(event: TestingInputEvent): void;
  screenRows(): readonly string[];
  nativeHistoryRows(): readonly string[];
  styleAt(row: number, column: number): Readonly<Record<string, unknown>>;
  cellXOfText(row: number, text: string): number | null;
  advance(milliseconds: number): void;
  exited(): boolean;
};

const accesses = new WeakMap<object, TuiTestingAccess>();

export function registerTuiTestingAccess(owner: object, access: TuiTestingAccess): void {
  accesses.set(owner, access);
}

export function tuiTestingAccess(owner: object): TuiTestingAccess {
  const access = accesses.get(owner);
  if (access === undefined) throw new Error("TUI testing access is unavailable");
  return access;
}
