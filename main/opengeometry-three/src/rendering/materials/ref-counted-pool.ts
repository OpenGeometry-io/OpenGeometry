export type PoolEntry<M> = { material: M; refs: number };
export type PooledMaterial<M> = { key: string; material: M };

export function acquire<M>(pool: Map<string, PoolEntry<M>>, key: string, create: () => M): PooledMaterial<M> {
  let entry = pool.get(key);
  if (!entry) {
    entry = { material: create(), refs: 0 };
    pool.set(key, entry);
  }
  entry.refs++;
  return { key, material: entry.material };
}

export function release<M extends { dispose(): void }>(pool: Map<string, PoolEntry<M>>, key: string): void {
  const entry = pool.get(key);
  if (!entry) return;
  if (--entry.refs <= 0) {
    entry.material.dispose();
    pool.delete(key);
  }
}
