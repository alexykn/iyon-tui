import { describe, expect, test } from "bun:test";
import { native, requireNativeClass } from "../src/transport/native/addon.ts";

const Host = requireNativeClass(native.NativeTuiHost, "NativeTuiHost");
const Source = requireNativeClass(native.NativeTextSource, "NativeTextSource");

function initialBatch(namespace: number, expectedRevision = 0): Uint32Array {
	const structure = [
		1,
		4,
		1,
		1,
		3,
		14,
		namespace,
		1,
		1,
		1,
		0,
		1,
		0,
		1,
		0,
		0,
		0,
		0,
	];
	return new Uint32Array([
		0x49595549,
		1,
		16 + structure.length,
		namespace,
		expectedRevision,
		0,
		1,
		structure.length,
		0,
		0,
		0,
		0,
		0,
		0,
		0,
		0,
		...structure,
	]);
}

describe("qualified occurrence native ingress", () => {
	test("accepts nonzero typed-array offsets and allocates the acknowledgement before apply", () => {
		const host = new Host(20, 4, true);
		try {
			const source = initialBatch(host.uiNamespace());
			const backing = new ArrayBuffer(source.byteLength + 4);
			new Uint8Array(backing, 4).set(new Uint8Array(source.buffer));
			const offsetView = new Uint32Array(backing, 4, source.length);
			const acknowledgement = host.commitUiV1(
				offsetView,
				new Uint8Array(),
				new Uint8Array(),
				[],
			);
			expect(acknowledgement[0]).toBe(0);
			expect(acknowledgement[1]).toBe(1);
			expect(acknowledgement[3]).toBe(1);
			expect(acknowledgement.length).toBe(12);
		} finally {
			host.dispose();
		}
	});

	test("rejects malformed and shared-backed input before native mutation", () => {
		const host = new Host(20, 4, true);
		try {
			expect(() =>
				host.commitUiV1(
					new Uint32Array([0]),
					new Uint8Array(),
					new Uint8Array(),
					[],
				),
			).toThrow();
			const shared = new Uint32Array(new SharedArrayBuffer(64));
			expect(() =>
				host.commitUiV1(shared, new Uint8Array(), new Uint8Array(), []),
			).toThrow();
			const mismatchedLocalCount = initialBatch(host.uiNamespace());
			mismatchedLocalCount[6] = 2;
			expect(() =>
				host.commitUiV1(
					mismatchedLocalCount,
					new Uint8Array(),
					new Uint8Array(),
					[],
				),
			).toThrow();
			const overCapacity = initialBatch(host.uiNamespace());
			overCapacity[6] = 1_048_577;
			expect(() =>
				host.commitUiV1(overCapacity, new Uint8Array(), new Uint8Array(), []),
			).toThrow();
			const detachedBacking = new ArrayBuffer(64);
			const detached = new Uint32Array(detachedBacking);
			structuredClone(detachedBacking, { transfer: [detachedBacking] });
			expect(() =>
				host.commitUiV1(detached, new Uint8Array(), new Uint8Array(), []),
			).toThrow();
		} finally {
			host.dispose();
		}
	});

	test("rejects wrong-class and prototype-spoofed Sources before unwrapping", () => {
		const host = new Host(20, 4, true);
		const source = new Source("stream");
		const port = host.contentPort("text");
		try {
			const words = initialBatch(host.uiNamespace());
			words[14] = 1;
			const wrongClass = host as unknown as typeof source;
			const spoofed = Object.create(Object.getPrototypeOf(source));
			expect(() =>
				host.commitUiV1(words, new Uint8Array(), new Uint8Array(), [
					wrongClass,
				]),
			).toThrow();
			expect(() =>
				host.commitUiV1(words, new Uint8Array(), new Uint8Array(), [spoofed]),
			).toThrow();
			expect(
				host.commitUiV1(words, new Uint8Array(), new Uint8Array(), [source])[0],
			).toBe(0);
			expect(() =>
				port.connect(spoofed, "plain", "word", true, false, 0, 0, 0, 0),
			).toThrow();
		} finally {
			host.dispose();
			source.dispose();
		}
	});

	test("returns a rejected acknowledgement without publishing a failed cycle", () => {
		const host = new Host(20, 4, true);
		try {
			const first = host.commitUiV1(
				initialBatch(host.uiNamespace()),
				new Uint8Array(),
				new Uint8Array(),
				[],
			);
			const body = [host.uiNamespace(), 1, 1, 1];
			const child = [...first.slice(8, 12)];
			const structure = [3, 14, ...child, ...body, 0, 0, 0, 0];
			const cycle = new Uint32Array([
				0x49595549,
				1,
				16 + structure.length,
				host.uiNamespace(),
				1,
				0,
				0,
				structure.length,
				0,
				0,
				0,
				0,
				0,
				0,
				0,
				0,
				...structure,
			]);
			const rejected = host.commitUiV1(
				cycle,
				new Uint8Array(),
				new Uint8Array(),
				[],
			);
			expect(rejected[0]).toBe(0x8000_0000);
			expect(rejected[1]).toBe(1);
			expect(rejected[5]).toBe(6);
		} finally {
			host.dispose();
		}
	});

	test("qualifies Source identity and retains membership through the content plane", () => {
		const host = new Host(20, 4, true);
		const source = new Source("stream");
		try {
			const structure = [
				1,
				4,
				1,
				1,
				3,
				14,
				host.uiNamespace(),
				1,
				1,
				1,
				0,
				1,
				0,
				1,
				0,
				0,
				0,
				0,
				6,
				10,
				0,
				1,
				0,
				1,
				0,
				2,
				0,
				2,
			];
			const content = [
				0x20, 9, 2, 1, 1, 0, 1, 0, 1, 0x21, 11, 3, 0, 0, 2, 0, 2, 0, 0, 1,
			];
			const words = new Uint32Array([
				0x49595549,
				1,
				16 + structure.length + content.length,
				host.uiNamespace(),
				0,
				0,
				3,
				structure.length,
				0,
				content.length,
				0,
				0,
				0,
				0,
				1,
				0,
				...structure,
				...content,
			]);
			const acknowledgement = host.commitUiV1(
				words,
				new Uint8Array(),
				new Uint8Array(),
				[source],
			);
			expect(acknowledgement[0]).toBe(0);
			expect(() => source.dispose()).toThrow();
		} finally {
			host.dispose();
		}
	});

	test("disposes an explicit Connector once before disposing its Port", () => {
		const host = new Host(20, 4, true);
		const source = new Source("stream");
		try {
			const structure = [
				1,
				4,
				1,
				1,
				3,
				14,
				host.uiNamespace(),
				1,
				1,
				1,
				0,
				1,
				0,
				1,
				0,
				0,
				0,
				0,
			];
			const content = [
				0x20, 9, 2, 1, 2, 0, 0, 0, 0, 0x21, 11, 3, 0, 0, 2, 0, 2, 0, 0, 2,
			];
			const first = new Uint32Array([
				0x49595549,
				1,
				16 + structure.length + content.length,
				host.uiNamespace(),
				0,
				0,
				3,
				structure.length,
				0,
				content.length,
				0,
				0,
				0,
				0,
				1,
				0,
				...structure,
				...content,
			]);
			const acknowledgement = host.commitUiV1(
				first,
				new Uint8Array(),
				new Uint8Array(),
				[source],
			);
			const port = [...acknowledgement.slice(12, 16)];
			const connector = [...acknowledgement.slice(16, 20)];
			expect(() => source.dispose()).toThrow();

			const dispose = [0x23, 6, ...connector, 0x24, 6, ...port];
			const second = new Uint32Array([
				0x49595549,
				1,
				16 + dispose.length,
				host.uiNamespace(),
				1,
				0,
				0,
				0,
				0,
				dispose.length,
				0,
				0,
				0,
				0,
				1,
				0,
				...dispose,
			]);
			const result = host.commitUiV1(
				second,
				new Uint8Array(),
				new Uint8Array(),
				[source],
			);
			expect(result[0]).toBe(0);
			expect(() => source.dispose()).not.toThrow();
		} finally {
			host.dispose();
		}
	});

	test("releases Source membership when the native host closes", () => {
		const host = new Host(20, 4, true);
		const source = new Source("stream");
		try {
			const structure = [
				1,
				4,
				1,
				1,
				3,
				14,
				host.uiNamespace(),
				1,
				1,
				1,
				0,
				1,
				0,
				1,
				0,
				0,
				0,
				0,
			];
			const content = [
				0x20, 9, 2, 1, 2, 0, 0, 0, 0, 0x21, 11, 3, 0, 0, 2, 0, 2, 0, 0, 2,
			];
			const words = new Uint32Array([
				0x49595549,
				1,
				16 + structure.length + content.length,
				host.uiNamespace(),
				0,
				0,
				3,
				structure.length,
				0,
				content.length,
				0,
				0,
				0,
				0,
				1,
				0,
				...structure,
				...content,
			]);
			const acknowledgement = host.commitUiV1(
				words,
				new Uint8Array(),
				new Uint8Array(),
				[source],
			);
			expect(acknowledgement[0]).toBe(0);
			expect(() => source.dispose()).toThrow();

			host.dispose();
			expect(() => source.dispose()).not.toThrow();
		} finally {
			host.dispose();
		}
	});

	test("prepares typed control commands and editor replacement in one batch", () => {
		const host = new Host(20, 4, true);
		try {
			const structure = [0x08, 11, 1, 1, 2, 0, 0, 0, 0, 0, 1];
			const state = [0x18, 7, 0, 1, 0, 4, 3];
			const descriptor = [0x41, 10, 0, 1, 0, 4, 0, 3, 1, 0];
			const words = new Uint32Array([
				0x49595549,
				1,
				16 + structure.length + state.length + descriptor.length,
				host.uiNamespace(),
				0,
				0,
				1,
				structure.length,
				state.length,
				0,
				0,
				descriptor.length,
				1,
				3,
				0,
				0,
				...structure,
				...state,
				...descriptor,
			]);
			const acknowledgement = host.commitUiV1(
				words,
				new Uint8Array([1]),
				new TextEncoder().encode("abc"),
				[],
			);
			expect(acknowledgement[0]).toBe(0);
			expect(acknowledgement[3]).toBe(1);

			const control = [...acknowledgement.slice(8, 12)];
			const invalid = [0x18, 7, ...control, 0xffff];
			const invalidWords = new Uint32Array([
				0x49595549,
				1,
				16 + invalid.length,
				host.uiNamespace(),
				1,
				0,
				0,
				0,
				invalid.length,
				0,
				0,
				0,
				0,
				0,
				0,
				0,
				...invalid,
			]);
			expect(
				host.commitUiV1(
					invalidWords,
					new Uint8Array(),
					new Uint8Array(),
					[],
				)[0],
			).toBe(0x8000_0000);

			const next = [0x18, 8, ...control, 1, "y".charCodeAt(0)];
			const nextWords = new Uint32Array([
				0x49595549,
				1,
				16 + next.length,
				host.uiNamespace(),
				1,
				0,
				0,
				0,
				next.length,
				0,
				0,
				0,
				0,
				0,
				0,
				0,
				...next,
			]);
			expect(
				host.commitUiV1(nextWords, new Uint8Array(), new Uint8Array(), [])[0],
			).toBe(0);
		} finally {
			host.dispose();
		}
	});

	test("decodes finite root configuration and rejects wrong-kind configuration", () => {
		const host = new Host(20, 4, true);
		try {
			const validStructure = [
				0x01,
				4,
				1,
				1,
				0x02,
				10,
				2,
				3,
				0,
				0,
				0,
				0,
				0,
				10,
				0x03,
				14,
				host.uiNamespace(),
				1,
				1,
				1,
				0,
				1,
				0,
				1,
				0,
				0,
				0,
				0,
			];
			const validMetadata = new Uint8Array([1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
			const validWords = new Uint32Array([
				0x49595549,
				1,
				16 + validStructure.length,
				host.uiNamespace(),
				0,
				0,
				2,
				validStructure.length,
				0,
				0,
				0,
				0,
				validMetadata.length,
				0,
				0,
				0,
				...validStructure,
			]);
			const validResult = host.commitUiV1(
				validWords,
				validMetadata,
				new Uint8Array(),
				[],
			);
			expect(validResult[0]).toBe(0);
		} finally {
			host.dispose();
		}

		const invalidHost = new Host(20, 4, true);
		try {
			const invalidStructure = [
				0x01,
				4,
				1,
				1,
				0x02,
				10,
				2,
				1,
				0,
				0,
				0,
				0,
				0,
				10,
				0x03,
				14,
				invalidHost.uiNamespace(),
				1,
				1,
				1,
				0,
				1,
				0,
				1,
				0,
				0,
				0,
				0,
			];
			const invalidWords = new Uint32Array([
				0x49595549,
				1,
				16 + invalidStructure.length,
				invalidHost.uiNamespace(),
				0,
				0,
				2,
				invalidStructure.length,
				0,
				0,
				0,
				0,
				10,
				0,
				0,
				0,
				...invalidStructure,
			]);
			expect(() =>
				invalidHost.commitUiV1(
					invalidWords,
					new Uint8Array([1, 1, 1, 0, 0, 0, 0, 0, 0, 0]),
					new Uint8Array(),
					[],
				),
			).toThrow();
		} finally {
			invalidHost.dispose();
		}
	});

	test("does not release an earlier Connector when a later prepared resource fails", () => {
		const host = new Host(20, 4, true);
		const source = new Source("stream");
		try {
			const content = [
				0x20, 9, 2, 1, 1, 0, 1, 0, 1, 0x21, 11, 3, 0, 0, 2, 0, 2, 0, 0, 1,
			];
			const structure = [
				1,
				4,
				1,
				1,
				3,
				14,
				host.uiNamespace(),
				1,
				1,
				1,
				0,
				1,
				0,
				1,
				0,
				0,
				0,
				0,
				6,
				10,
				0,
				1,
				0,
				1,
				0,
				2,
				0,
				2,
			];
			const words = new Uint32Array([
				0x49595549,
				1,
				16 + structure.length + content.length,
				host.uiNamespace(),
				0,
				0,
				3,
				structure.length,
				0,
				content.length,
				0,
				0,
				0,
				0,
				1,
				0,
				...structure,
				...content,
			]);
			const accepted = host.commitUiV1(
				words,
				new Uint8Array(),
				new Uint8Array(),
				[source],
			);
			const port = [...accepted.slice(12, 16)];
			const connector = [...accepted.slice(16, 20)];
			expect(() => source.dispose()).toThrow();

			const lateFailure = [
				0x23,
				6,
				...connector,
				0x21,
				11,
				1,
				1,
				...port,
				0,
				0,
				2,
			];
			const rejectedWords = new Uint32Array([
				0x49595549,
				1,
				16 + lateFailure.length,
				host.uiNamespace(),
				1,
				0,
				1,
				0,
				0,
				lateFailure.length,
				0,
				0,
				0,
				0,
				1,
				0,
				...lateFailure,
			]);
			const rejected = host.commitUiV1(
				rejectedWords,
				new Uint8Array(),
				new Uint8Array(),
				[source],
			);
			expect(rejected[0]).toBe(0x8000_0000);
			expect(rejected[1]).toBe(1);
			expect(() => source.dispose()).toThrow();
		} finally {
			host.dispose();
		}
	});
});
