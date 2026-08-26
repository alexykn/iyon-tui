import type { Output } from "./types.ts";

export class RouteConflict extends Error {
  readonly code = "ION_ROUTE_CONFLICT";
}

export class OutputRouter<A = Output> {
  private readonly routes = new Map<string, (value: Output) => A>();
  private readonly queue: Array<{ key: string; value: Output }> = [];

  route<T extends Output>(key: string, handler: (value: T) => A): void {
    if (this.routes.has(key)) throw new RouteConflict(`output already has an application route: ${key}`);
    this.routes.set(key, handler as (value: Output) => A);
  }

  remove(key: string): boolean { return this.routes.delete(key); }
  emit(key: string, value: Output): void { this.queue.push({ key, value }); }
  drain(): A[] {
    const actions: A[] = [];
    while (this.queue.length > 0) {
      const event = this.queue.shift()!;
      const route = this.routes.get(event.key);
      if (route !== undefined) actions.push(route(event.value));
    }
    return actions;
  }
}
