/**
 * Passive application state shared by the legacy and React fixture examples.
 *
 * Keeping this value type free of renderer imports keeps the example route
 * removed without making the React consumer depend on it.
 */
export interface ConsumerState {
	readonly title: string;
	readonly status: string;
	readonly items: readonly string[];
	readonly showHint: boolean;
}
