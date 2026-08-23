/**
 * PERF-12 T13.1 R4 — generic tracked `State<T>` (handoff §9,
 * AMENDMENT-C §7/§18 Step 7R).
 *
 * A minimal observable primitive that exists ONLY as an invalidation source
 * (Review Addendum §33.3): reads during a retained scope's evaluation
 * subscribe that scope; writes that change by `Object.is` mark exactly the
 * subscribed live scopes dirty and enqueue them once. No computed values, no
 * effects, no derived graphs, no proxies, no deep observation — the framework
 * needs to answer one question: "which retained execution scopes must run?"
 *
 * Contract:
 *   - reads are allowed anywhere (untracked outside evaluation);
 *   - writes are FORBIDDEN inside any component body (pure evaluation,
 *     AMENDMENT-C §7.2) and reject deterministically without mutating;
 *   - dependency sets are execution-dependent: a scope's committed
 *     subscriptions survive until its next evaluation COMMITS (handoff §21);
 *     aborted evaluations retain the old set (AMENDMENT-C §7.1).
 */

import {
  ExecutionError,
  activeExecutionScope,
  type RetainedExecutionScope,
} from "./execution.ts";

/** Error codes emitted by tracked-state operations. */
export const STATE_WRITE_DURING_EVALUATION = "TUI_EXECUTION_STATE_WRITE_DURING_EVALUATION";

/**
 * Internal contract between tracked sources and the execution substrate.
 * Scopes store committed/pending source sets; sources store subscriber sets.
 *
 * @internal
 */
export interface TrackedStateSource {
  subscribe(scope: RetainedExecutionScope): void;
  unsubscribe(scope: RetainedExecutionScope): void;
}

class StateSource<T> implements TrackedStateSource {
  currentValue: T;
  readonly subscribers = new Set<RetainedExecutionScope>();

  constructor(initial: T) {
    this.currentValue = initial;
  }

  get value(): T {
    // Track the read against whichever scope is currently evaluating
    // (untracked outside evaluation — plain data access).
    const scope = activeExecutionScope();
    if (scope !== undefined) scope.linkDependency(this);
    return this.currentValue;
  }

  subscribe(scope: RetainedExecutionScope): void {
    this.subscribers.add(scope);
  }

  unsubscribe(scope: RetainedExecutionScope): void {
    this.subscribers.delete(scope);
  }

  /** Publishes a confirmed value change to every subscribed live scope. */
  private publish(): void {
    for (const scope of this.subscribers) {
      scope.runtime.invalidateFromState(scope);
    }
  }

  set(next: T): void {
    if (activeExecutionScope() !== undefined) {
      throw new ExecutionError(
        STATE_WRITE_DURING_EVALUATION,
        "tracked state cannot be written while a component body is evaluating",
      );
    }
    if (Object.is(this.currentValue, next)) return; // Object.is change discipline
    this.currentValue = next;
    this.publish();
  }

  update(transition: (previous: T) => T): void {
    if (activeExecutionScope() !== undefined) {
      throw new ExecutionError(
        STATE_WRITE_DURING_EVALUATION,
        "tracked state cannot be written while a component body is evaluating",
      );
    }
    this.set(transition(this.currentValue));
  }
}

/**
 * Public tracked state (AMENDMENT-C §7):
 *
 * ```ts
 * const status = state("ready");
 * const Footer = defineView(() => View.text(status.value)); // read = subscribe
 * status.set("running");                                    // write = invalidate
 * ```
 */
export interface State<T> {
  readonly value: T;
  set(value: T): void;
  update(update: (previous: T) => T): void;
}

export function state<T>(initial: T): State<T> {
  const source = new StateSource(initial);
  const wrapper: State<T> = {
    get value(): T {
      return source.value;
    },
    set(value: T): void {
      source.set(value);
    },
    update(update: (previous: T) => T): void {
      source.update(update);
    },
  };
  trackedSubscribers.set(wrapper as State<unknown>, source.subscribers);
  return wrapper;
}

/**
 * Live subscriber count for diagnostics/tests. Not part of the semantic or
 * public contract — memory gates assert subscriber counts follow live scopes.
 */
export function trackedStateSubscriberCount(source: State<unknown>): number {
  return trackedSubscribers.get(source)?.size ?? 0;
}

/** Identity map from public wrappers to their sources (test/diagnostic aid). */
const trackedSubscribers = new WeakMap<State<unknown>, Set<RetainedExecutionScope>>();
