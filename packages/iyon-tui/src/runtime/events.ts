/** Routed output event delivered by a live Tui runtime. */
export interface OutputEvent {
  readonly type: "output";
  readonly routeId: string;
  readonly payload?: string;
}

/** Terminal/runtime termination event. */
export interface TerminateEvent {
  readonly type: "terminate";
  readonly reason?: string;
}

export type TuiEvent = OutputEvent | TerminateEvent;
