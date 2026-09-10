/** Private association between a public TuiRuntime and its native host. */
const hosts = new WeakMap<object, object>();
const closedRuntimes = new WeakSet<object>();
interface RootSession {
	readonly runtime: object;
	readonly host: object;
	authority: RootAuthority | undefined;
	state: "open" | "closed";
}

const sessionsByRuntime = new WeakMap<object, RootSession>();
const sessionsByHost = new WeakMap<object, RootSession>();

export interface RootAuthority {
	readonly coordinator: {
		createExplicitPort(
			family?: string,
		): import("./resources.ts").ReactContentPort;
	};
	close(): void;
	closeAfterHostExit(): void;
}

export function registerReactHost(runtime: object, host: object): void {
	hosts.set(runtime, host);
	closedRuntimes.delete(runtime);
}

export function nativeHostForReact(runtime: object): object | undefined {
	return hosts.get(runtime);
}

export function markReactHostClosed(runtime: object): void {
	closedRuntimes.add(runtime);
}

export function isReactHostClosed(runtime: object): boolean {
	return closedRuntimes.has(runtime);
}

/**
 * Reserves the one live root slot before constructing React or coordinator
 * state. The reservation is visible to duplicate callers but not as a usable
 * content-port authority until commit succeeds.
 */
export function reserveReactRootAuthority(
	runtime: object,
	host: object,
): { commit(authority: RootAuthority): void; release(): void } {
	if (closedRuntimes.has(runtime))
		throw new Error("a React root requires a live TuiRuntime host");
	const existingForRuntime = sessionsByRuntime.get(runtime);
	const existingForHost = sessionsByHost.get(host);
	if (existingForRuntime?.state === "open" || existingForHost?.state === "open")
		throw new Error("a React root is already attached to this Tui host");
	const session: RootSession = {
		runtime,
		host,
		authority: undefined,
		state: "open",
	};
	sessionsByRuntime.set(runtime, session);
	sessionsByHost.set(host, session);
	let active = true;
	return {
		commit(authority: RootAuthority): void {
			if (!active || session.state !== "open")
				throw new Error("React root authority reservation is no longer live");
			session.authority = authority;
			active = false;
		},
		release(): void {
			if (!active) return;
			active = false;
			if (sessionsByRuntime.get(runtime) === session)
				sessionsByRuntime.delete(runtime);
			if (sessionsByHost.get(host) === session) sessionsByHost.delete(host);
		},
	};
}

export function reactRootAuthority(runtime: object): RootAuthority | undefined {
	const session = sessionsByRuntime.get(runtime);
	return session?.state === "open" ? session.authority : undefined;
}

export function unregisterReactRootAuthority(runtime: object): void {
	const session = sessionsByRuntime.get(runtime);
	if (session === undefined) return;
	session.state = "closed";
	session.authority = undefined;
}

export function hasClosedReactRoot(host: object): boolean {
	return sessionsByHost.get(host)?.state === "closed";
}
