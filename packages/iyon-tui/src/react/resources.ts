import type {
	ContentConnectorStatus,
	ContentSource,
	TextFunnel,
} from "../api/content/retained.ts";
import type { ContentConnector, ContentPort } from "../api/content/explicit.ts";
import { TextBlockSource, TextStreamSource } from "../api/content/retained.ts";

/**
 * Caller-owned content resources created after a React root is installed.
 *
 * These values deliberately do not wrap the legacy native ContentPort and
 * Connector classes.  Their identity is a qualified UI resource handle from
 * the root's CommitCoordinator, so structural attachment, selection and
 * disposal all use the same accepted revision stream as React mutations.
 */
export interface ExplicitResourceCoordinator {
	createExplicitConnector(
		port: ReactContentPort,
		source: ContentSource,
		funnel: TextFunnel,
	): ReactContentConnector;
	selectExplicitConnector(
		port: ReactContentPort,
		connector: ReactContentConnector | undefined,
	): void;
	deactivateExplicitConnector(connector: ReactContentConnector): void;
	explicitSelectedConnector(
		port: ReactContentPort,
	): ReactContentConnector | undefined;
	disposeExplicitConnector(connector: ReactContentConnector): void;
	disposeExplicitPort(port: ReactContentPort): void;
	explicitPortMounted(port: ReactContentPort): boolean;
	explicitConnectorStatus(
		connector: ReactContentConnector,
	): ContentConnectorStatus;
}

export class ReactContentPort implements ContentPort {
	readonly kind = "content-port" as const;
	readonly family = "text" as const;
	private closed = false;

	constructor(
		private readonly coordinator: ExplicitResourceCoordinator,
		readonly handle: Readonly<{
			host_namespace: number;
			slot: number;
			generation: number;
			kind: number;
		}>,
	) {}

	/** Compatibility identity for code that only records attachment identity. */
	get id(): number {
		return this.handle.slot;
	}

	get disposed(): boolean {
		return this.closed;
	}

	connect(source: ContentSource, funnel: TextFunnel): ReactContentConnector {
		this.ensureOpen();
		validateSource(source);
		if (!isTextFunnel(funnel))
			throw new TypeError("ContentPort.connect requires a text Funnel");
		return this.coordinator.createExplicitConnector(this, source, funnel);
	}

	activate(): void {
		this.ensureOpen();
		const selected = this.coordinator.explicitSelectedConnector(this);
		if (selected === undefined)
			throw new Error("ContentPort has no Connector to activate");
		this.coordinator.selectExplicitConnector(this, selected);
	}

	deactivate(): void {
		this.ensureOpen();
		this.coordinator.selectExplicitConnector(this, undefined);
	}

	mounted(): boolean {
		this.ensureOpen();
		return this.coordinator.explicitPortMounted(this);
	}

	isMounted(): boolean {
		return this.mounted();
	}

	dispose(): void {
		if (this.closed) return;
		this.coordinator.disposeExplicitPort(this);
	}

	/** @internal */
	markDisposed(): void {
		this.closed = true;
	}

	private ensureOpen(): void {
		if (this.closed) throw new Error("ContentPort is disposed");
	}
}

export class ReactContentConnector implements ContentConnector {
	readonly kind = "connector" as const;
	private closed = false;

	constructor(
		private readonly coordinator: ExplicitResourceCoordinator,
		readonly handle: Readonly<{
			host_namespace: number;
			slot: number;
			generation: number;
			kind: number;
		}>,
		readonly attachedPort: ReactContentPort,
		readonly attachedSource: ContentSource,
		readonly funnel: TextFunnel,
	) {}

	get disposed(): boolean {
		return this.closed;
	}

	activate(): void {
		this.ensureOpen();
		this.coordinator.selectExplicitConnector(this.attachedPort, this);
	}

	deactivate(): void {
		this.ensureOpen();
		this.coordinator.deactivateExplicitConnector(this);
	}

	status(): ContentConnectorStatus {
		this.ensureOpen();
		return this.coordinator.explicitConnectorStatus(this);
	}

	dispose(): void {
		if (this.closed) return;
		this.coordinator.disposeExplicitConnector(this);
	}

	/** @internal */
	markDisposed(): void {
		this.closed = true;
	}

	private ensureOpen(): void {
		if (this.closed) throw new Error("Connector is disposed");
	}
}

export function isExplicitContentPort(
	value: unknown,
): value is ReactContentPort {
	return value instanceof ReactContentPort && value.kind === "content-port";
}

export function isExplicitContentConnector(
	value: unknown,
): value is ReactContentConnector {
	return value instanceof ReactContentConnector && value.kind === "connector";
}

function validateSource(source: ContentSource): void {
	if (
		!(source instanceof TextBlockSource) &&
		!(source instanceof TextStreamSource)
	)
		throw new TypeError("ContentPort.connect requires a text Source");
}

function isTextFunnel(value: unknown): value is TextFunnel {
	return (
		value !== null &&
		typeof value === "object" &&
		(value as { readonly kind?: unknown }).kind === "text-funnel" &&
		(value as { readonly family?: unknown }).family === "text"
	);
}
