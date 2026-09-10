/**
 * Content-plane control transport.
 *
 * This module owns the small N-API control calls for Source, Port, and
 * Connector identities. It does not lower semantic Views or implement Source
 * payload/projection work; bulk Source data belongs to the content data plane.
 */

import type { NativeTextSourceContract } from "../native/addon.ts";
import { nativeTui } from "../native/factories.ts";

export type { NativeTextSourceContract };

export function createTextSource(
	kind: "block" | "stream",
	options?: object,
): NativeTextSourceContract {
	return nativeTui.textSource(kind, options);
}
