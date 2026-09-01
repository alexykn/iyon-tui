import { expect, test } from "bun:test";

import { View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";
import { attachSemanticResourceForTesting } from "../src/api/view/view.ts";
import { semanticNodeOf, SEMANTIC_VIEW_KIND } from "../src/api/view/semantic-node.ts";
import {
  NativeResourceRegistry,
  runtimeResourceEnvironment,
  runtimeResourceRegistry,
  type ResourceOwner,
} from "../src/runtime/native-resource-registry.ts";
import { AttachmentBindingState, prepareSemanticAttachments } from "../src/runtime/attachments.ts";
import { RuntimeErrorChannel } from "../src/runtime/error-channel.ts";
import { EnvironmentWakeBroker } from "../src/runtime/wake-broker.ts";
import { native } from "../src/transport/native/addon.ts";
import {
  nativeViewAbiSession,
  tryNativeMaterialize,
} from "../src/transport/structural/native-view-abi.ts";
import {
  RetainedRootBoundary,
  setRootColdMaterializer,
} from "../src/transport/structural/retained-dag.ts";
import type { HandleId } from "../src/api/controls/framework-handle.ts";

const PERF13A = "PERF-13-A runtime seam";

test(`${PERF13A} validates and leases semantic attachments during prepare`, () => {
  const environment = {};
  const host = {};
  const owner: ResourceOwner = { environment, host };
  const registry = new NativeResourceRegistry(environment);
  const handle = {};
  const handleId = 101 as HandleId;
  registry.registerInternal(
    handle,
    handleId,
    "state",
    handle,
    owner,
    new Set([SEMANTIC_VIEW_KIND.text]),
  );
  const attached = attachSemanticResourceForTesting(
    View.text("attached"),
    "stateAttachment",
    handleId,
    handle,
  );
  const binding = new AttachmentBindingState();
  const prepared = prepareSemanticAttachments(
    semanticNodeOf(attached),
    registry,
    environment,
    host,
  );
  expect(registry.stats().preparedLeases).toBe(1);
  binding.commitDesired(prepared);
  binding.commitVisible();
  expect(binding.desiredCount()).toBe(1);
  expect(binding.visibleCount()).toBe(1);
  expect(registry.stats().desiredLeases).toBe(1);
  expect(registry.stats().visibleLeases).toBe(1);

  expect(() => prepareSemanticAttachments(
    semanticNodeOf(attached),
    registry,
    environment,
    {},
  )).toThrow("different host");
  const unsupported = attachSemanticResourceForTesting(
    View.spacer(1),
    "stateAttachment",
    handleId,
    handle,
  );
  expect(() => prepareSemanticAttachments(
    semanticNodeOf(unsupported),
    registry,
    environment,
    host,
  )).toThrow("unsupported");

  binding.dispose();
  registry.beginDisposal(handleId);
  registry.release(handleId);
  expect(() => registry.registerInternal({}, handleId, "state")).toThrow("retired");
  expect(registry.stats()).toEqual({
    live: 0,
    disposing: 0,
    preparedLeases: 0,
    desiredLeases: 0,
    visibleLeases: 0,
  });
});

test(`${PERF13A} rejects duplicate attachment use and restores prepare leases`, () => {
  const environment = {};
  const host = {};
  const registry = new NativeResourceRegistry(environment);
  const handle = {};
  const handleId = 102 as HandleId;
  registry.registerInternal(handle, handleId, "content", handle, { environment, host });
  const attached = attachSemanticResourceForTesting(
    View.text("shared"),
    "contentAttachment",
    handleId,
    handle,
  );
  const duplicate = View.horizontal([attached, attached]);
  expect(() => prepareSemanticAttachments(
    semanticNodeOf(duplicate),
    registry,
    environment,
    host,
  )).toThrow("duplicate content attachment");
  expect(registry.stats().preparedLeases).toBe(0);
});

test(`${PERF13A} rejects an invalid attachment in H3 prepare before visible mutation`, async () => {
  const registry = runtimeResourceRegistry();
  const environment = runtimeResourceEnvironment();
  const handle = {};
  const handleId = nextFrameworkHandleTestId();
  registry.registerInternal(handle, handleId, "state", handle, { environment });
  const attached = attachSemanticResourceForTesting(
    View.text("stable"),
    "stateAttachment",
    handleId,
    handle,
  );
  const duplicate = View.horizontal([attached, attached]);
  const tui = await AppHarness.open({ width: 20, height: 4 });
  try {
    tui.render({ body: attached });
    const before = [...tui.screenRows()];
    expect(() => tui.render({ body: duplicate })).toThrow("duplicate state attachment");
    expect(tui.screenRows()).toEqual(before);
  } finally {
    tui.close();
    registry.beginDisposal(handleId);
    registry.release(handleId);
  }
});

test(`${PERF13A} coalesces automatic wakes and retries only at an explicit barrier`, async () => {
  let desired = 1n;
  let committed = 0n;
  let calls = 0;
  let fail = true;
  const nativeHost = {
    epochs: () => ({
      host_id: "1",
      desired_structural_revision: desired.toString(),
      visible_frame_revision: committed.toString(),
      pending_epoch: desired.toString(),
      committed_epoch: committed.toString(),
    }),
    flushPendingHosts: (_budget?: number, forceRetry?: boolean) => {
      calls += 1;
      if (fail && !forceRetry) {
        return {
          rearm: false,
          attempted: 1,
          committed_hosts: [],
          errors: [{
            host_id: "1",
            attempted_epoch: desired.toString(),
            desired_revision: desired.toString(),
            phase: "frame",
            code: "FRAME_PREPARATION_FAILED",
            retryable: true,
            diagnostic: "blocked",
          }],
          wake_epoch: "1",
        };
      }
      committed = desired;
      return {
        rearm: false,
        attempted: 1,
        committed_hosts: ["1"],
        errors: [],
        wake_epoch: "1",
      };
    },
  };
  const errors = new RuntimeErrorChannel(() => true);
  let commits = 0;
  const broker = new EnvironmentWakeBroker(1);
  const registration = broker.register(nativeHost, errors, () => { commits += 1; });
  registration.markPending();
  registration.markPending();
  await Promise.resolve();
  await Promise.resolve();
  expect(calls).toBe(1);
  expect(commits).toBe(0);
  expect(errors.latestFor("1")?.diagnostic).toBe("blocked");

  fail = false;
  registration.flush();
  expect(calls).toBe(2);
  expect(commits).toBe(1);
  expect(errors.latestFor("1")).toBeUndefined();
  registration.dispose();
});

test(`${PERF13A} polls asynchronous presentation receipts without a microtask spin`, async () => {
  let ready = false;
  let calls = 0;
  let commits = 0;
  const nativeHost = {
    epochs: () => ({
      host_id: "1",
      desired_structural_revision: "1",
      visible_frame_revision: ready ? "1" : "0",
      pending_epoch: "1",
      committed_epoch: ready ? "1" : "0",
    }),
    flushPendingHosts: () => {
      calls += 1;
      return ready
        ? {
          rearm: false,
          waiting_for_presentation: false,
          attempted: 1,
          committed_hosts: ["1"],
          commits: [{ host_id: "1", committed_epoch: "1", visible_structural_revision: "1" }],
          errors: [],
          wake_epoch: "1",
        }
        : {
          rearm: false,
          waiting_for_presentation: true,
          attempted: 1,
          committed_hosts: [],
          commits: [],
          errors: [],
          wake_epoch: "1",
        };
    },
  };
  const broker = new EnvironmentWakeBroker(1);
  const registration = broker.register(nativeHost, new RuntimeErrorChannel(() => true), () => {
    commits += 1;
  });
  registration.markPending();
  await Promise.resolve();
  expect(calls).toBe(1);
  expect(commits).toBe(0);

  ready = true;
  await new Promise((resolve) => setTimeout(resolve, 10));
  expect(calls).toBe(2);
  expect(commits).toBe(1);
  registration.dispose();
});

test(`${PERF13A} separates desired structural publication from visible frame commit`, () => {
  const session = nativeViewAbiSession();
  const Host = native.NativeTuiHost;
  if (session === undefined || Host === undefined) throw new Error("native PERF-13-A surface is unavailable");
  const host = new Host(20, 4, true);
  setRootColdMaterializer(tryNativeMaterialize);
  const boundary = new RetainedRootBoundary(
    session,
    () => host,
    undefined,
    { deferHostCommit: true },
  );
  try {
    const first = View.text("first");
    const firstPublication = boundary.prepareDesiredInstall(first);
    if (firstPublication === undefined) throw new Error("first desired publication refused");
    firstPublication.commit();
    const pending = host.epochs?.();
    expect(pending?.desired_structural_revision).toBe("1");
    expect(pending?.visible_frame_revision).toBe("0");
    expect(pending?.pending_epoch).not.toBe(pending?.committed_epoch);

    const firstReport = host.flushPendingHosts?.(8, true);
    expect(firstReport?.errors).toEqual([]);
    boundary.commitVisible();
    expect(host.screenRows().some((row) => row.includes("first"))).toBe(true);

    const second = View.text("second");
    const secondPublication = boundary.prepareDesiredInstall(second);
    if (secondPublication === undefined) throw new Error("second desired publication refused");
    secondPublication.commit();
    const superseded = host.epochs?.();
    expect(superseded?.desired_structural_revision).toBe("2");
    expect(superseded?.visible_frame_revision).toBe("1");
    expect(host.screenRows().some((row) => row.includes("first"))).toBe(true);

    const secondReport = host.flushPendingHosts?.(8, true);
    expect(secondReport?.errors).toEqual([]);
    boundary.commitVisible();
    expect(host.screenRows().some((row) => row.includes("second"))).toBe(true);
  } finally {
    boundary.close();
    host.dispose();
  }
});

function nextFrameworkHandleTestId(): HandleId {
  const key = Symbol.for("iyon:tui:private-handle-counter");
  const counter = (globalThis as Record<PropertyKey, unknown>)[key] as { next: number } | undefined;
  if (counter === undefined) throw new Error("framework handle counter is unavailable");
  return counter.next++ as HandleId;
}
