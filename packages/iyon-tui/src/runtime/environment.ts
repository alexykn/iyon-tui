import {
  NativeResourceRegistry,
  runtimeResourceEnvironment,
  runtimeResourceRegistry,
} from "./native-resource-registry.ts";
import { RuntimeErrorChannel } from "./error-channel.ts";
import {
  EnvironmentWakeBroker,
  type NativeFrameHost,
  type RuntimeHostRegistration,
} from "./wake-broker.ts";

export interface RuntimeEnvironment {
  readonly token: object;
  readonly resources: NativeResourceRegistry;
  readonly wakeBroker: EnvironmentWakeBroker;
  registerHost(
    native: NativeFrameHost,
    errors: RuntimeErrorChannel,
    onCommitted: () => void,
  ): RuntimeHostRegistration;
}

class EnvironmentRuntime implements RuntimeEnvironment {
  readonly token = runtimeResourceEnvironment();
  readonly resources = runtimeResourceRegistry();
  readonly wakeBroker = new EnvironmentWakeBroker();

  registerHost(
    native: NativeFrameHost,
    errors: RuntimeErrorChannel,
    onCommitted: () => void,
  ): RuntimeHostRegistration {
    return this.wakeBroker.register(native, errors, onCommitted);
  }
}

const ENVIRONMENT_KEY = Symbol.for("iyon:tui:runtime-environment");
type RuntimeGlobals = typeof globalThis & { [ENVIRONMENT_KEY]?: RuntimeEnvironment };
const globals = globalThis as RuntimeGlobals;

/** One runtime environment per JavaScript realm/module instance. */
export function runtimeEnvironment(): RuntimeEnvironment {
  return globals[ENVIRONMENT_KEY] ??= new EnvironmentRuntime();
}
