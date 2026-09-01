import { native, requireNativeClass } from "./addon.ts";
import type { TextStreamOptions } from "../../api/controls/text-stream.ts";
import type { TextSourceOptions } from "../../api/content/retained.ts";

/** Private factories for native-backed framework handles. */
export const nativeTui = {
  history: () => new (requireNativeClass(native.NativeHistory, "NativeHistory"))(),
  textStream: (options?: TextStreamOptions) => new (requireNativeClass(native.NativeTextStream, "NativeTextStream"))(options),
  textSource: (kind: "block" | "stream", options?: TextSourceOptions) => new (requireNativeClass(native.NativeTextSource, "NativeTextSource"))(kind, options),
  markdownProjector: () => new (requireNativeClass(native.NativeMarkdownProjector, "NativeMarkdownProjector"))(),
  plainProjector: () => new (requireNativeClass(native.NativePlainProjector, "NativePlainProjector"))(),
};
