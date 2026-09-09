/** Private association between a public TuiRuntime and its native host. */
const hosts = new WeakMap<object, object>();

export function registerReactHost(runtime: object, host: object): void {
	hosts.set(runtime, host);
}

export function nativeHostForReact(runtime: object): object | undefined {
	return hosts.get(runtime);
}
