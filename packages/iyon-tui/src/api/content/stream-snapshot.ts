/** Caller-defined annotation attached to a streamed semantic segment. */
export interface StreamAnnotation {
  readonly namespace: string;
  readonly name: string;
}

/** Immutable snapshot of a text stream at one source revision. */
export interface StreamSnapshot {
  readonly text: string;
  readonly revision: number;
  readonly sealed: boolean;
  readonly segments?: readonly StreamSegmentSnapshot[];
}

export interface StreamSegmentSnapshot {
  readonly annotations: readonly StreamAnnotation[];
  readonly text: string;
}
