/**
 * Passive application state shared by the legacy and React fixture examples.
 *
 * Keeping this value type free of View imports lets the old example route be
 * removed without making the React consumer depend on it.
 */
export interface ConsumerState {
	readonly title: string;
	readonly status: string;
	readonly items: readonly string[];
	readonly showHint: boolean;
}
