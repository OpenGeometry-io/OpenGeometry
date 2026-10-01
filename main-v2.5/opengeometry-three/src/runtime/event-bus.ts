import type { Events, Listener } from '../dto/events';
import { currentRuntime, runtime } from './runtime-state';

export function on(event: Events, handler: Listener): () => boolean {
  const listeners = runtime().listeners;
  const group = listeners.get(event) ?? new Set<Listener>();
  group.add(handler);
  listeners.set(event, group);
  return () => group.delete(handler);
}

export function emit(event: Events, detail: unknown): void {
  for (const handler of currentRuntime()?.listeners.get(event) ?? []) handler(detail);
}
