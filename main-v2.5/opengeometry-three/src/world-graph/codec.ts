export function encode(value: unknown): string {
  return JSON.stringify(value);
}

export function scope(options?: { instances?: 'all' }): string {
  return encode(options?.instances === 'all' ? 'AllInstances' : 'Node');
}

export function operationParams(kind: string, params: Record<string, unknown>): Record<string, unknown> {
  if (kind === 'Extrude') {
    const profile = params['profile'] as { ogId: string };
    const holes = (params['holes'] as { ogId: string }[] | undefined) ?? [];
    return { kind, profile: profile.ogId, holes: holes.map((hole) => hole.ogId), distance: params['distance'] };
  }
  if (kind === 'Sweep') {
    const profile = params['profile'] as { ogId: string };
    return { kind, profile: profile.ogId, path: (params['path'] as { ogId: string }).ogId };
  }
  return { kind, ...params };
}
