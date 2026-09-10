import {
	runtimeResourceEnvironment,
	runtimeResourceRegistry,
	type NativeResourceRegistry,
} from "./native-resource-registry.ts";

export interface RuntimeEnvironment {
	readonly token: object;
	readonly resources: NativeResourceRegistry;
}

const ENVIRONMENT_KEY = Symbol.for("iyon:tui:runtime-environment");
const globals = globalThis as typeof globalThis & {
	[ENVIRONMENT_KEY]?: RuntimeEnvironment;
};

/** Sources retain their realm lifetime without a JavaScript frame or receipt poller. */
export function runtimeEnvironment(): RuntimeEnvironment {
	return (globals[ENVIRONMENT_KEY] ??= {
		token: runtimeResourceEnvironment(),
		resources: runtimeResourceRegistry(),
	});
}
