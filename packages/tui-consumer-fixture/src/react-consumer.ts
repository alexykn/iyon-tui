import { TextStreamSource } from "@iyon/tui";
import {
	Animation,
	Box,
	Column,
	Content,
	createReactRoot,
	Editor,
	History,
	HistoryUnit,
	type IyonReactRoot,
	type OccurrenceRef,
	Scroll,
	Text,
	useContentConnector,
	useContentPort,
} from "@iyon/tui/react";
import { AppHarness } from "@iyon/tui/testing";
import { createElement, Fragment } from "react";
import type { ConsumerState } from "./consumer-state.ts";

/** A third-party-shaped React consumer: only documented package entrypoints. */
export interface ReactConsumerSession {
	readonly tui: AppHarness;
	readonly root: IyonReactRoot;
	/** External Source ownership stays with the application, outside render. */
	readonly source: TextStreamSource;
	render(
		state: ConsumerState,
	): Promise<{ readonly revision: number; readonly accepted: true }>;
	focusEditor(): void;
	waitForSubmit(): Promise<string>;
	readonly lastInput: () => string | undefined;
	readonly submitted: () => string | undefined;
	readonly editorEvents: () => readonly string[];
	readonly historyIdentity: () => number | string;
	close(): Promise<void>;
}

interface ConsumerAppProps {
	readonly state: ConsumerState;
	readonly source: TextStreamSource;
	readonly onEditorRef: (value: OccurrenceRef | null) => void;
	readonly onHistoryRef: (value: OccurrenceRef | null) => void;
	readonly onInput: (text: string) => void;
	readonly onSubmit: (text: string) => void;
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

function ConsumerApp({
	state,
	source,
	onEditorRef,
	onHistoryRef,
	onInput,
	onSubmit,
}: ConsumerAppProps) {
	const port = useContentPort();
	const connector = useContentConnector({ port, source });
	const list = state.items.map((item) =>
		createElement(Text, { key: item }, `- ${item}`),
	);
	return createElement(
		Fragment,
		{},
		createElement(
			Column,
			{},
			createElement(
				Box,
				{ key: "heading" },
				createElement(Text, { key: "title" }, state.title),
				...(state.showHint
					? [
							createElement(
								Text,
								{ key: "hint" },
								"type a message; ctrl+c exits",
							),
						]
					: []),
			),
			createElement(Scroll, { key: "list" }, list),
			createElement(
				Animation,
				{ key: "animation", intervalMs: 10 },
				createElement(
					Box,
					{ key: "frame-a" },
					createElement(Text, {}, "animation-a"),
				),
				createElement(
					Box,
					{ key: "frame-b" },
					createElement(Text, {}, "animation-b"),
				),
			),
			createElement(Editor, {
				key: "editor",
				defaultValue: "compose",
				ref: onEditorRef,
				onInput: (event) => onInput(event.text),
				onSubmit: (event) => onSubmit(event.text),
			}),
			createElement(Content, { key: "stream", port: connector }),
			createElement(
				Text,
				{ key: "footer" },
				`${state.title} · ${state.status}`,
			),
		),
		createElement(
			History,
			{},
			createElement(
				HistoryUnit,
				{ ref: onHistoryRef },
				`history ${state.status}`,
			),
		),
	);
}

export async function openReactConsumerSession(): Promise<ReactConsumerSession> {
	const tui = await AppHarness.open({ width: 60, height: 16 });
	const root = createReactRoot(tui);
	const source = TextStreamSource.create();
	let editorRef: OccurrenceRef | undefined;
	let historyRef: OccurrenceRef | undefined;
	let lastInput: string | undefined;
	let submitted: string | undefined;
	const events: string[] = [];
	const submitWaiters: Array<(value: string) => void> = [];
	let closed = false;

	return {
		tui,
		root,
		source,
		render: (state) =>
			root
				.render(
					createElement(ConsumerApp, {
						state,
						source,
						onEditorRef: (value) => {
							editorRef = value ?? undefined;
						},
						onHistoryRef: (value) => {
							historyRef = value ?? undefined;
						},
						onInput: (text) => {
							lastInput = text;
							events.push(`input:${text}`);
						},
						onSubmit: (text) => {
							submitted = text;
							events.push(`submit:${text}`);
							for (const resolve of submitWaiters.splice(0)) resolve(text);
						},
					}),
				)
				.then(async (commit) => {
					await root.whenContentVisible(commit.revision);
					return commit;
				}),
		focusEditor(): void {
			if (editorRef === undefined) throw new Error("editor ref is unavailable");
			editorRef.focus();
		},
		waitForSubmit(): Promise<string> {
			if (submitted !== undefined) return Promise.resolve(submitted);
			return new Promise((resolve) => submitWaiters.push(resolve));
		},
		lastInput: () => lastInput,
		submitted: () => submitted,
		editorEvents: () => [...events],
		historyIdentity(): number | string {
			if (historyRef === undefined)
				throw new Error("HistoryUnit ref is unavailable");
			return historyRef.historyIdentity();
		},
		async close(): Promise<void> {
			if (closed) return;
			closed = true;
			await runCleanupStages(
				[
					() => (!root.faulted ? root.unmount() : undefined),
					() => root.close(),
					() => tui.close(),
					() => source.dispose(),
				],
				"React consumer cleanup failed",
			);
		},
	};
}
