import { native, requireNativeClass } from "./addon.ts";
import type { TextSourceOptions } from "../../api/content/retained.ts";

/** Private factories for native-backed framework handles. */
export const nativeTui = {
  history: () => new (requireNativeClass(native.NativeHistory, "NativeHistory"))(),
  textSource: (kind: "block" | "stream", options?: TextSourceOptions | Readonly<Record<string, unknown>>) => new (requireNativeClass(native.NativeTextSource, "NativeTextSource"))(kind, options),
  markdownProjector: () => new (requireNativeClass(native.NativeMarkdownProjector, "NativeMarkdownProjector"))(),
  plainProjector: () => new (requireNativeClass(native.NativePlainProjector, "NativePlainProjector"))(),
};
