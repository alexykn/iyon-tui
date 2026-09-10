import { createElement, createRef, Fragment } from "react";
import { TextStreamSource } from "../../src/index.ts";
import {
	Box,
	Column,
	Content,
	createReactRoot,
	Editor,
	History,
	HistoryUnit,
	type OccurrenceRef,
	Text,
} from "../../src/react/index.ts";
import { AppHarness } from "../../src/testing/index.ts";

export interface TuiDemoResult {
	readonly screenRows: readonly string[];
	readonly nativeHistoryRows: readonly string[];
	readonly input: string;
	readonly stream: string;
	readonly focused: boolean;
	readonly submitted: string;
	readonly editorEvents: readonly string[];
	readonly historyIdentity: number | string;
}

async function runCleanupStages(
	stages: readonly (() => unknown)[],
	message: string,
): Promise<void> {
	const failures: unknown[] = [];
	for (const stage of stages) {
		try {
			await stage();
		} catch (error) {
			failures.push(error);
		}
	}
	if (failures.length === 1) throw failures[0];
	if (failures.length > 1) throw new AggregateError(failures, message);
}

export async function runTuiDemo(): Promise<TuiDemoResult> {
	const harness = await AppHarness.open({ width: 32, height: 8 });
	const root = createReactRoot(harness);
	const source = TextStreamSource.create();
	const editorRef = createRef<OccurrenceRef>();
	const historyRef = createRef<OccurrenceRef>();
	let input: string | undefined;
	const editorEvents: string[] = [];
	let resolveSubmit!: (value: string) => void;
	const submitPromise = new Promise<string>((resolve) => {
		resolveSubmit = resolve;
	});
	try {
		await root.render(
			createElement(
				Fragment,
				{},
				createElement(
					Column,
					{},
					createElement(Box, {}, createElement(Text, {}, "composer")),
					createElement(Editor, {
						defaultValue: "compose",
						ref: editorRef,
						onInput: (event) => {
							input = event.text;
							editorEvents.push(`input:${event.text}`);
						},
						onSubmit: (event) => {
							editorEvents.push(`submit:${event.text}`);
							resolveSubmit(event.text);
						},
					}),
					createElement(Content, { source }),
				),
				createElement(
					History,
					{},
					createElement(HistoryUnit, { ref: historyRef }, "completed history"),
				),
			),
		);
		await root.whenVisible();
		source.append("streaming text");
		await root.whenContentVisible();
		const editor = editorRef.current;
		const history = historyRef.current;
		if (editor === null || history === null)
			throw new Error("demo occurrence refs were not published");
		editor.focus();
		harness.pressKey("x");
		harness.pressKey("Enter");
		const submitted = await submitPromise;
		if (input === undefined)
			throw new Error("demo input callback was not observed");
		await root.whenVisible();
		const screenRows = harness.screenRows();
		const stream = screenRows
			.map((row) => row.trim())
			.find((row) => row === "streaming text");
		if (stream === undefined)
			throw new Error("demo source content was not observed on screen");
		const historyIdentity = history.historyIdentity();
		return {
			screenRows,
			nativeHistoryRows: harness.nativeHistoryRows(),
			input,
			stream,
			focused: input !== undefined,
			submitted,
			editorEvents,
			historyIdentity,
		};
	} finally {
		await runCleanupStages(
			[
				() => (!root.faulted ? root.unmount() : undefined),
				() => root.close(),
				() => harness.close(),
				() => source.dispose(),
			],
			"TUI demo cleanup failed",
		);
	}
}
