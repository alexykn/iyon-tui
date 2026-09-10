import type {
	ContentConnectorStatus,
	ContentSource,
	TextFunnel,
} from "./retained.ts";

/** Caller-owned, React-root-qualified UI content Port. */
export interface ContentPort {
	readonly kind: "content-port";
	readonly family: "text";
	readonly disposed: boolean;
	readonly id: number;
	connect(source: ContentSource, funnel: TextFunnel): ContentConnector;
	activate(): void;
	deactivate(): void;
	mounted(): boolean;
	isMounted(): boolean;
	dispose(): void;
}

/** Caller-owned Connector associated with exactly one explicit ContentPort. */
export interface ContentConnector {
	readonly kind: "connector";
	readonly disposed: boolean;
	readonly attachedPort: ContentPort;
	readonly attachedSource: ContentSource;
	activate(): void;
	deactivate(): void;
	status(): ContentConnectorStatus;
	dispose(): void;
}
