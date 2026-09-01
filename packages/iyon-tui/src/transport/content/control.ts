/**
 * Content-plane control transport.
 *
 * This module owns the small N-API control calls for Source, Port, and
 * Connector identities. It does not lower semantic Views or implement Source
 * payload/projection work; bulk data belongs to the later content data plane.
 */

import type {
  NativeContentConnectorContract,
  NativeContentPortContract,
  NativeStateWake,
  NativeTextSourceContract,
  NativeTuiHostContract,
} from "../native/addon.ts";
import { nativeTui } from "../native/factories.ts";

export type {
  NativeContentConnectorContract,
  NativeContentPortContract,
  NativeStateWake,
  NativeTextSourceContract,
};

export function createTextSource(
  kind: "block" | "stream",
  options?: object,
): NativeTextSourceContract {
  return nativeTui.textSource(kind, options);
}

export function createContentPort(
  host: NativeTuiHostContract,
  family: string,
): NativeContentPortContract {
  return host.contentPort(family);
}

export function deactivatePort(port: NativeContentPortContract): NativeStateWake {
  return port.deactivate();
}

export function contentPortMounted(port: NativeContentPortContract): boolean {
  return port.mounted();
}

export function connectContent(
  port: NativeContentPortContract,
  source: NativeTextSourceContract,
  funnel: object,
): NativeContentConnectorContract {
  return port.connect(source, funnel);
}

export function activateContent(
  connector: NativeContentConnectorContract,
): NativeStateWake {
  return connector.activate();
}

export function deactivateContent(
  connector: NativeContentConnectorContract,
): NativeStateWake {
  return connector.deactivate();
}

export function disposeContentConnector(
  connector: NativeContentConnectorContract,
): NativeStateWake {
  return connector.dispose();
}

export function contentConnectorStatus(
  connector: NativeContentConnectorContract,
): object {
  return connector.status();
}
