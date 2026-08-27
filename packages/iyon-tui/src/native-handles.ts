import { native } from "./native.ts";
import { requireNativeClass } from "./handles.ts";
import type { TextStreamOptions } from "./types.ts";

/** Private factories for native-backed framework handles. */
export const nativeTui = {
  history: () => new (requireNativeClass(native.NativeHistory, "NativeHistory"))(),
  textStream: (options?: TextStreamOptions) => new (requireNativeClass(native.NativeTextStream, "NativeTextStream"))(options),
  markdownProjector: () => new (requireNativeClass(native.NativeMarkdownProjector, "NativeMarkdownProjector"))(),
  plainProjector: () => new (requireNativeClass(native.NativePlainProjector, "NativePlainProjector"))(),
};
