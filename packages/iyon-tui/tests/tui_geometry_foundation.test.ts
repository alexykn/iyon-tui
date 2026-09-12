import { describe, expect, test } from "bun:test";
import { createElement } from "react";
import { Box, createReactRoot } from "../src/react/index.ts";
import { normalizeProps } from "../src/react/instance.ts";
import { AppHarness } from "../src/testing/index.ts";
import {
	UI_PROPERTIES,
	uiEncodePropertyValue,
	uiPropertyValuesEqual,
} from "../src/transport/ui/generated/ui_schema.ts";

describe("finite geometry boundary", () => {
	test("normalizes dimensions, percentages, tracks, and placements", () => {
		const tracks = [
			{ type: "fr" as const, value: 1 },
			{ type: "percent" as const, value: 0.5 },
			{
				type: "minmax" as const,
				min: { type: "length" as const, value: 2 },
				max: { type: "fr" as const, value: 1 },
			},
		];
		const normalized = normalizeProps("box", {
			width: { unit: "percent", value: 0.5 },
			margin: { top: -2, right: "auto", bottom: 0, left: 1.25 },
			gridTemplateColumns: tracks,
			inset: { unit: "length", value: 2 },
			gridColumn: { start: -1, end: { span: 2 } },
		});
		expect(normalized.properties.get("width")?.value).toEqual({
			unit: "percent",
			value: 0.5,
		});
		expect(normalized.properties.get("margin")?.value).toEqual({
			top: { unit: "length", value: -2 },
			right: "auto",
			bottom: { unit: "length", value: 0 },
			left: { unit: "length", value: 1.25 },
		});
		tracks[0] = { type: "fr", value: 99 };
		expect(
			normalized.properties.get("gridTemplateColumns")?.value,
		).toHaveLength(3);
		expect(
			normalized.properties.get("gridTemplateColumns")?.value,
		).toContainEqual({
			type: "minmax",
			min: { type: "length", value: 2 },
			max: { type: "fr", value: 1 },
		});
		expect(normalized.properties.get("inset")?.value).toEqual({
			top: { unit: "length", value: 2 },
			right: { unit: "length", value: 2 },
			bottom: { unit: "length", value: 2 },
			left: { unit: "length", value: 2 },
		});
		expect(
			uiEncodePropertyValue(
				"gridTemplateColumns",
				[
					{
						type: "minmax",
						min: { type: "length", value: 2 },
						max: { type: "fr", value: 1 },
					},
				],
				() => [0, 0],
			),
		).toHaveLength(6);
		const intrinsicBounds = normalizeProps("box", {
			gridTemplateRows: [
				{
					type: "minmax",
					min: "minContent",
					max: "maxContent",
				},
			],
		});
		expect(intrinsicBounds.properties.get("gridTemplateRows")?.value).toEqual([
			{ type: "minmax", min: "minContent", max: "maxContent" },
		]);
		expect(
			uiEncodePropertyValue(
				"gridTemplateRows",
				[{ type: "minmax", min: "minContent", max: "maxContent" }],
				() => [0, 0],
			),
		).toHaveLength(6);
		expect(normalized.properties.get("gridColumn")?.value).toEqual({
			start: -1,
			end: { span: 2 },
		});
	});

	test("canonicalizes signed zero and rejects malformed finite geometry", () => {
		expect(uiPropertyValuesEqual("flexGrow", -0, 0)).toBe(true);
		expect(
			uiPropertyValuesEqual(
				"margin",
				{
					top: { unit: "length", value: 1 },
					right: "auto",
					bottom: { unit: "percent", value: 0.25 },
					left: { unit: "length", value: 2 },
				},
				{
					left: { unit: "length", value: 2 },
					bottom: { unit: "percent", value: 0.25 },
					right: "auto",
					top: { unit: "length", value: 1 },
				},
			),
		).toBe(true);
		expect(
			uiPropertyValuesEqual(
				"gridTemplateColumns",
				[
					{
						type: "minmax",
						min: { type: "length", value: 1 },
						max: { type: "fr", value: 1 },
					},
				],
				[
					{
						max: { value: 1, type: "fr" },
						type: "minmax",
						min: { value: 1, type: "length" },
					},
				],
			),
		).toBe(true);
		expect(() => normalizeProps("box", { flexGrow: Number.NaN })).toThrow();
		expect(() =>
			normalizeProps("box", { width: { unit: "percent", value: 1.1 } }),
		).toThrow();
		expect(() =>
			normalizeProps("box", { gridTemplateRows: Array(65).fill("auto") }),
		).toThrow();
		expect(() => normalizeProps("box", { gridRow: { start: 0 } })).toThrow();
		expect(() => normalizeProps("box", { rowGap: -1 })).toThrow();
		expect(() => normalizeProps("box", { rowGap: "fit" })).toThrow();
		expect(() => normalizeProps("box", { flexBasis: "fill" })).toThrow();
		expect(() => normalizeProps("box", { margin: { typo: 3 } })).toThrow();
		expect(() =>
			normalizeProps("box", { display: { value: "flex" } }),
		).toThrow();
		expect(() =>
			normalizeProps("box", {
				gridColumn: { start: { span: 2, typo: true } },
			}),
		).toThrow();
		expect(() =>
			normalizeProps("box", {
				gridTemplateRows: [{ type: "length", value: 2, typo: true }],
			}),
		).toThrow();
		expect(() =>
			normalizeProps("box", {
				gridTemplateRows: [
					{ type: "minmax", min: { type: "fr", value: 1 }, max: "auto" },
				],
			}),
		).toThrow();
	});

	test("packs f32 bits and keeps stable property IDs", () => {
		const words = uiEncodePropertyValue("flexGrow", 1.5, () => [0, 0]);
		expect(words).toEqual([1069547520]);
		expect(UI_PROPERTIES.width).toBe(0x0101);
		expect(UI_PROPERTIES.height).toBe(0x0102);
	});

	test("real native boundary preserves fit/fill and realizes T6 geometry", async () => {
		const tui = await AppHarness.open({ width: 12, height: 4 });
		const root = createReactRoot(tui);
		try {
			expect(
				(await root.render(createElement(Box, { width: "fit" }, "fit")))
					.accepted,
			).toBe(true);
			await root.whenVisible();
			expect(
				(await root.render(createElement(Box, { width: "fill" }, "fill")))
					.accepted,
			).toBe(true);
			await root.whenVisible();
			expect(
				(
					await root.render(
						createElement(Box, { width: { unit: "length", value: 3 } }, "new"),
					)
				).accepted,
			).toBe(true);
			await root.whenVisible();
			expect(tui.screenRows().some((row) => row.includes("new"))).toBe(true);
		} finally {
			await root.unmount();
			tui.close();
		}
	});
});
