import type { Body } from '../../bodies/body';
import { bodyKey, type Runtime } from '../../runtime/runtime-state';

export type GroupState = { pass: number; members: readonly Body[] };

export type HostGroups = {
  live: Set<GroupState>;
  of: WeakMap<Body, GroupState[]>;
  woken: Set<GroupState>;
  pending: Set<Body>;
  drawn: boolean;
};

export function createHostGroups(): HostGroups {
  return { live: new Set(), of: new WeakMap(), woken: new Set(), pending: new Set(), drawn: false };
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
  if (!groupShown(state, group)) {
    state.lod.stale = true;
    state.groups.woken.add(group);
  }
  state.groups.drawn = true;
  group.pass = state.pass;
}

export function setMembers(state: Runtime, group: GroupState, members: readonly Body[]): void {
  for (const body of group.members) leave(state, body, group);
  group.members = members;
  for (const body of members) join(state, body, group);
  state.lod.stale = true;
  if (groupShown(state, group)) state.groups.woken.add(group);
}

export function dropGroup(state: Runtime, group: GroupState): void {
  setMembers(state, group, []);
  group.pass = -Infinity;
  state.groups.live.delete(group);
  state.groups.woken.delete(group);
}

export function liveMembers(state: Runtime, group: GroupState): readonly Body[] {
  const live = group.members.filter((body) => state.bodies.get(bodyKey(body.handle, body.generation)) === body);
  if (live.length !== group.members.length) group.members = live;
  return live;
}

export function groupedBodies(state: Runtime): Body[] {
  const bodies: Body[] = [];
  for (const group of state.groups.live) {
    if (groupShown(state, group)) for (const body of liveMembers(state, group)) bodies.push(body);
  }
  return bodies;
}

export function queueGrouped(state: Runtime, body: Body): void {
  if (state.groups.of.get(body)?.length) state.groups.pending.add(body);
}

export function takeWaiting(state: Runtime): Set<Body> {
  const groups = state.groups;
  const waiting = new Set(groups.pending);
  for (const group of groups.woken) for (const body of liveMembers(state, group)) waiting.add(body);
  groups.pending.clear();
  groups.woken.clear();
  groups.drawn = false;
  return waiting;
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
