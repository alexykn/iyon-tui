/**
 * PERF-12 T13.1 R2 — public `defineView` component API (handoff §8,
 * AMENDMENT-C §6/§18 Step 5R).
 *
 * The component wrapper IS the retained execution boundary. Compose needed
 * compiler-generated restart groups because an `@Composable` call is
 * syntactically an ordinary function call; Iyon's explicit wrapper makes the
 * boundary an ordinary runtime abstraction — no source transform, no
 * SiteIds, no build machinery (AMENDMENT-C §11).
 *
 * Public contract:
 *
 * ```ts
 * const Footer = defineView<{ status: string }>(({ status }) =>
 *   View.text(status),
 * );
 *
 * column.child(Footer({ status: state.status }))
 * ```
 *
 * - An invocation returns a normal `View` for the parent to embed.
 * - Identity is parent-local: type + ordinal position; keys only for
 *   repeated/movable instances (keyed dynamics land in R8 — the plumbing is
 *   in place, the public key API deliberately waits for it).
 * - Unchanged props (same own-key set + Object.is per value) SKIP the body
 *   entirely; props holding fresh nested objects compare by identity and do
 *   NOT skip — immutable props are the documented contract (Review Addendum
 *   §33.6).
 * - There are no global IDs, no registration, no configuration, and no way
 *   to opt out of retained execution — it is the framework's ordinary
 *   behavior on every supported consumer path (handoff §4.1/§25).
 */

import { invokeComponent, type ViewComponent, type ViewComponentType } from "./execution.ts";
import type { View } from "./values/view.ts";

/**
 * Defines a retained view component from a pure synchronous render body.
 * The returned value is callable: invoking it inside another component's
 * render reconciles a persistent child scope under the caller's position
 * and either re-presents the child's committed output (props unchanged),
 * executes the body with new inputs (props changed), or mounts it fresh.
 */
export function defineView<P>(render: (props: P) => View): ViewComponent<P> {
  if (typeof render !== "function") {
    throw new TypeError("defineView requires a render function");
  }
  const component = ((props: P) => {
    const result = invokeComponent(component, props);
    return result.view;
  }) as unknown as ViewComponent<P>;
  (component as { render: (props: P) => View }).render = render;
  return component;
}

export type { ViewComponent, ViewComponentType };
