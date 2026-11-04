import type { Body } from '../../bodies/body';
import type { Runtime } from '../../runtime/runtime-state';

export type GroupState = { pass: number; members: Set<Body> };

export type HostGroups = {
  live: Set<GroupState>;
  of: WeakMap<Body, GroupState[]>;
  woken: Set<GroupState>;
  pending: Set<Body>;
  drawn: boolean;
  revision: number;
  dropped: FinalizationRegistry<GroupState>;
};

export function createHostGroups(release: (group: GroupState) => void): HostGroups {
  return {
    live: new Set(), of: new WeakMap(), woken: new Set(), pending: new Set(), drawn: false, revision: 0,
    dropped: new FinalizationRegistry(release),
  };
}

export function groupShown(state: Runtime, group: GroupState): boolean {
  return group.pass >= state.pass - 1;
}

export function groupDisplayed(state: Runtime, body: Body): boolean {
  const groups = state.groups.of.get(body);
  if (!groups) return false;
  for (const group of groups) if (groupShown(state, group)) return true;
  return false;
}

export function stampGroup(state: Runtime, group: GroupState): void {
  if (!groupShown(state, group)) wake(state, group);
  state.groups.drawn = true;
  group.pass = state.pass;
}

export function setMembers(state: Runtime, group: GroupState, members: readonly Body[]): void {
  for (const body of group.members) leave(state, body, group);
  group.members = new Set(members);
  for (const body of group.members) join(state, body, group);
  state.groups.revision++;
  state.lod.stale = true;
  if (groupShown(state, group)) state.groups.woken.add(group);
}

export function dropGroup(state: Runtime, group: GroupState): void {
  setMembers(state, group, []);
  group.pass = -Infinity;
  state.groups.live.delete(group);
  state.groups.woken.delete(group);
}

export function dropMember(state: Runtime, body: Body): void {
  for (const group of state.groups.of.get(body) ?? []) group.members.delete(body);
  state.groups.of.delete(body);
  state.groups.pending.delete(body);
  state.groups.revision++;
}

export function groupedBodies(state: Runtime): Body[] {
  const bodies: Body[] = [];
  for (const group of state.groups.live) {
    if (groupShown(state, group)) for (const body of group.members) bodies.push(body);
  }
  return bodies;
}

export function groupsKey(state: Runtime): number {
  let members = 0;
  for (const group of state.groups.live) if (groupShown(state, group)) members += group.members.size + 1;
  return members;
}

export function queueGrouped(state: Runtime, body: Body): void {
  if (state.groups.of.get(body)?.length) state.groups.pending.add(body);
}

export function shownChanged(state: Runtime, body: Body): void {
  if (!state.groups.of.get(body)?.length) return;
  state.groups.pending.add(body);
  state.groups.revision++;
  state.lod.stale = true;
}

export function queueShapes(state: Runtime, shapeId?: string): void {
  if (shapeId === undefined) {
    for (const group of state.groups.live) if (groupShown(state, group)) state.groups.woken.add(group);
    return;
  }
  for (const body of state.byShape.get(shapeId) ?? []) queueGrouped(state, body);
}

export function takeWaiting(state: Runtime): Set<Body> {
  const groups = state.groups;
  const waiting = new Set(groups.pending);
  for (const group of groups.woken) for (const body of group.members) waiting.add(body);
  groups.pending.clear();
  groups.woken.clear();
  groups.drawn = false;
  return waiting;
}

function wake(state: Runtime, group: GroupState): void {
  state.lod.stale = true;
  state.groups.revision++;
  state.groups.woken.add(group);
}

function join(state: Runtime, body: Body, group: GroupState): void {
  const groups = state.groups.of.get(body);
  if (groups) groups.push(group);
  else state.groups.of.set(body, [group]);
}

function leave(state: Runtime, body: Body, group: GroupState): void {
  const groups = state.groups.of.get(body) ?? [];
  const index = groups.indexOf(group);
  if (index >= 0) groups.splice(index, 1);
}
