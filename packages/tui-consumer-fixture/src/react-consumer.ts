import {
	Box,
	Column,
	createReactRoot,
	type IyonReactRoot,
	Text,
} from "@iyon/tui/react";
import { AppHarness } from "@iyon/tui/testing";
import { createElement } from "react";
import type { ConsumerState } from "./consumer.ts";

/** A third-party-shaped React consumer: only documented package entrypoints. */
export interface ReactConsumerSession {
	readonly tui: AppHarness;
	readonly root: IyonReactRoot;
	render(
		state: ConsumerState,
	): Promise<{ readonly revision: number; readonly accepted: true }>;
	close(): Promise<void>;
}

function ConsumerApp({ state }: { readonly state: ConsumerState }) {
	const children = [
		createElement(Text, { key: "title" }, state.title),
		...(state.showHint
			? [createElement(Text, { key: "hint" }, "type a message; ctrl+c exits")]
			: []),
		...state.items.map((item) =>
			createElement(Text, { key: item }, `- ${item}`),
		),
		createElement(Text, { key: "footer" }, `${state.title} · ${state.status}`),
	];
	return createElement(Column, {}, createElement(Box, {}, children));
}

export async function openReactConsumerSession(): Promise<ReactConsumerSession> {
	const tui = await AppHarness.open({ width: 60, height: 16 });
	const root = createReactRoot(tui);
	return {
		tui,
		root,
		render: (state) => root.render(createElement(ConsumerApp, { state })),
		async close(): Promise<void> {
			if (!root.faulted) await root.unmount().catch(() => {});
			tui.close();
		},
	};
}
