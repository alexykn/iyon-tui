import { useLayoutEffect, useMemo } from "react";
import { type ContentSource, TextFunnel } from "../api/content/retained.ts";
import type {
	ContentConnectorToken,
	ContentPortToken,
	HookOwner,
} from "./instance.ts";
import { registerContentToken } from "./instance.ts";

export interface ContentConnectorOptions {
	readonly port: ContentPortToken;
	readonly source: ContentSource;
	readonly funnel?: TextFunnel;
}

function useHookOwner(): HookOwner {
	const owner = useMemo<HookOwner>(
		() => ({
			tokens: new Set(),
			dependentConnectors: new Set(),
			currentToken: undefined,
			mounted: false,
			released: false,
			coordinator: undefined,
			notifyRelease: undefined,
			notifyTokenReplacement: undefined,
			release: undefined,
		}),
		[],
	);
	useLayoutEffect(() => {
		owner.mounted = true;
		owner.released = false;
		return () => {
			owner.mounted = false;
			owner.released = true;
			queueMicrotask(() => {
				if (!owner.mounted) owner.notifyRelease?.();
			});
		};
	}, [owner]);
	return owner;
}

/** Creates a JS-only lazy ContentPort token. */
export function useContentPort(): ContentPortToken {
	const owner = useHookOwner();
	return useMemo(() => {
		const token: ContentPortToken = {
			kind: "content-port-token",
			family: "text",
		};
		registerContentToken(token, owner);
		return token;
	}, [owner]);
}

/**
 * Associates a JS-only connector token with a lazy Port.  Neither hook calls
 * native code; materialization occurs only if the Port reaches a committed
 * Content occurrence.
 */
export function useContentConnector(
	options: ContentConnectorOptions,
): ContentConnectorToken {
	const { port, source, funnel } = options;
	const owner = useHookOwner();
	const defaultFunnel = useMemo(() => TextFunnel.plain(), []);
	const selectedFunnel = funnel ?? defaultFunnel;
	const connector = useMemo(() => {
		const connector: ContentConnectorToken = {
			kind: "content-connector-token",
			port,
			source,
			funnel: selectedFunnel,
		};
		registerContentToken(connector, owner);
		return connector;
	}, [owner, port, source, selectedFunnel]);
	useLayoutEffect(() => {
		// This effect is the committed boundary for a connector-hook dependency
		// change.  Render may be abandoned, so the coordinator must not be told
		// about a replacement until React has installed this effect.  The
		// microtask also coalesces StrictMode's setup/cleanup replay with the next
		// setup and lets the owner cleanup win when the hook unmounts.
		owner.currentToken = connector;
		return () => {
			queueMicrotask(() => {
				if (owner.mounted && owner.currentToken !== connector)
					owner.notifyTokenReplacement?.(connector);
			});
		};
	}, [owner, connector]);
	return connector;
}
