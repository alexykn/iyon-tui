import { native, requireNativeClass } from "./addon.ts";
import type { TextStreamOptions } from "../../api/controls/text-stream.ts";

/** Private factories for native-backed framework handles. */
export const nativeTui = {
  history: () => new (requireNativeClass(native.NativeHistory, "NativeHistory"))(),
  textStream: (options?: TextStreamOptions) => new (requireNativeClass(native.NativeTextStream, "NativeTextStream"))(options),
  markdownProjector: () => new (requireNativeClass(native.NativeMarkdownProjector, "NativeMarkdownProjector"))(),
  plainProjector: () => new (requireNativeClass(native.NativePlainProjector, "NativePlainProjector"))(),
};
