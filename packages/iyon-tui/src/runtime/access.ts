type RuntimeInputEvent =
  | { readonly type: "key"; readonly key: string; readonly modifiers?: readonly string[] }
  | { readonly type: "paste"; readonly text: string }
  | { readonly type: "resize"; readonly width: number; readonly height: number };

type RuntimeAccess = {
  flush(): void;
  enqueue(event: RuntimeInputEvent): void;
  screenRows(): readonly string[];
  nativeHistoryRows(): readonly string[];
  styleAt(row: number, column: number): Readonly<Record<string, unknown>>;
  cellXOfText(row: number, text: string): number | null;
  advance(milliseconds: number): void;
  exited(): boolean;
};

const accesses = new WeakMap<object, RuntimeAccess>();

export function registerRuntimeAccess(owner: object, access: RuntimeAccess): void {
  accesses.set(owner, access);
}

export function runtimeAccess(owner: object): RuntimeAccess {
  const access = accesses.get(owner);
  if (access === undefined) throw new Error("TUI runtime access is unavailable");
  return access;
}
