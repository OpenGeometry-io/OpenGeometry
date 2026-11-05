import type { Body } from '../../bodies/body';
import { OGError } from '../../errors';
import { currentRuntime, runtime, type Runtime } from '../../runtime/runtime-state';
import { dropGroup, setMembers, stampGroup, type GroupState } from '../lod/host-groups';
import { checkedBodies } from './geometry-scheduler';
import { beginPass } from './render-pass';

function members(state: Runtime, bodies: readonly Body[], label: string): Body[] {
  return [...new Set(checkedBodies(state, bodies, label))];
}

export class HostGroup {
  private readonly state: Runtime = runtime();
  private readonly group: GroupState = { pass: -Infinity, members: new Set() };
  private disposed = false;

  constructor(bodies: readonly Body[]) {
    setMembers(this.state, this.group, members(this.state, bodies, 'OpenGeometry.hostGroup'));
    this.state.groups.live.add(this.group);
    this.state.groups.dropped.register(this, this.group, this);
  }

  drawn(): void {
    if (this.disposed || currentRuntime() !== this.state) return;
    beginPass(this.state);
    stampGroup(this.state, this.group);
  }

  set(bodies: readonly Body[]): void {
    if (this.disposed || currentRuntime() !== this.state) {
      throw new OGError('Disposed', 'HostGroup.set', 'the host group is disposed');
    }
    setMembers(this.state, this.group, members(this.state, bodies, 'HostGroup.set'));
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.state.groups.dropped.unregister(this);
    if (currentRuntime() === this.state) dropGroup(this.state, this.group);
  }
}
