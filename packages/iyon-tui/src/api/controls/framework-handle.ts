import { asTuiError, tuiError } from "../errors.ts";
import {
  disposeFrameworkResource,
  registerFrameworkHandle,
} from "../../runtime/handle-registry.ts";
import { nativeResourceOf } from "../../transport/native/resources.ts";
import type { View } from "../view/view.ts";

declare const handleIdBrand: unique symbol;
/** JavaScript-local framework handle identity; this is not a native identifier. */
export type HandleId = number & { readonly [handleIdBrand]: "HandleId" };

/**
 * Nominal base for public framework handles. Runtime ownership and raw native
 * resource storage are delegated to their private seams; callers only see an
 * opaque semantic handle and its local identity.
 */
export abstract class FrameworkHandle<K extends string = string> {
  #frameworkHandleBrand!: void;
  readonly id: HandleId;
  readonly kind: K;
  private isDisposed = false;

  protected constructor(
    kind: K,
    resource: never,
    options: {
      readonly owner?: { readonly environment: object; readonly host?: object };
      readonly acceptedNodeKinds?: ReadonlySet<number>;
    } = {},
  ) {
    this.kind = kind;
    this.id = registerFrameworkHandle(this, resource as unknown as object, kind, options);
  }

  protected nativeAs<T extends object>(): T {
    return nativeResourceOf<T>(this);
  }

  get disposed(): boolean { return this.isDisposed; }

  dispose(): void {
    if (this.isDisposed) return;
    disposeFrameworkResource(this);
    this.isDisposed = true;
  }

  protected ensureOpen(): void {
    if (this.isDisposed) throw tuiError("disposed-handle", `${this.kind} handle has been disposed`, { id: this.id });
  }

  protected call<R>(operation: () => R): R {
    try {
      this.ensureOpen();
      return operation();
    } catch (error) {
      throw asTuiError(error);
    }
  }
}

/** Public mounted component handle projected into the semantic View tree. */
export interface ComponentHandle extends FrameworkHandle<"component" | "text-input"> {
  readonly kind: "component" | "text-input";
  view(): View;
}
