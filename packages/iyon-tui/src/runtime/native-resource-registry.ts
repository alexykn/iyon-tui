/**
 * Runtime-owned facade for the plane-neutral resolver. The implementation is
 * kept beside the native resource seam so transport/native never depends back
 * on runtime ownership, while callers have one runtime-level import point.
 */
export {
  NativeResourceRegistry,
  PreparedResourceLease,
  runtimeResourceEnvironment,
  runtimeResourceRegistry,
} from "../transport/native/resource-registry.ts";
export type {
  NativeResourceKind,
  NativeResourceStats,
  ResourceLifecycle,
  ResourceOwner,
  ResourceRegistration,
} from "../transport/native/resource-registry.ts";
