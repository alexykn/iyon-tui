import type { OutputEvent, TuiEvent } from "./types.ts";

export class FocusController {
  private focused = 0;
  focus(id: number): void { this.focused = id; }
  current(): number { return this.focused; }
}

export class InteractionRouter {
  constructor(private readonly focus = new FocusController()) {}
  route(event: TuiEvent, output: (event: OutputEvent, focused: number) => boolean): boolean {
    if (event.type !== "output") return false;
    return output(event, this.focus.current());
  }
  focusController(): FocusController { return this.focus; }
}
