import * as THREE from 'three';
import { emit } from '../runtime/event-bus.js';

export type PoseMemory = {
  position: THREE.Vector3;
  quaternion: THREE.Quaternion;
  scale: THREE.Vector3;
  parentWorld: THREE.Matrix4;
};

export function poseMemory(): PoseMemory {
  return {
    position: new THREE.Vector3(), quaternion: new THREE.Quaternion(), scale: new THREE.Vector3(1, 1, 1),
    parentWorld: new THREE.Matrix4(),
  };
}

export function derivePose(memory: PoseMemory, object: THREE.Object3D): void {
  if (object.parent) {
    object.matrix.copy(object.parent.matrixWorld).invert().multiply(object.matrixWorld);
    memory.parentWorld.copy(object.parent.matrixWorld);
  } else {
    object.matrix.copy(object.matrixWorld);
    memory.parentWorld.identity();
  }
  object.matrix.decompose(object.position, object.quaternion, object.scale);
  memory.position.copy(object.position);
  memory.quaternion.copy(object.quaternion);
  memory.scale.copy(object.scale);
}

export function keepPose(memory: PoseMemory, object: THREE.Object3D, ogId: string): void {
  if (!object.position.equals(memory.position) || !object.quaternion.equals(memory.quaternion)
    || !object.scale.equals(memory.scale)) {
    emit('warning', { code: 'PlacementOverwritten', ogId });
    object.position.copy(memory.position);
    object.quaternion.copy(memory.quaternion);
    object.scale.copy(memory.scale);
  }
  const parentWorld = object.parent ? object.parent.matrixWorld : new THREE.Matrix4();
  if (!parentWorld.equals(memory.parentWorld)) derivePose(memory, object);
}
