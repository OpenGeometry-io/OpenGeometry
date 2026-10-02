import * as THREE from 'three';
import { Solid, Wire } from '../../../../../dist/index.js';

const OVERRIDES = new Set(['updateMatrixWorld', 'clone', 'copy']);

function ownNames(objects: (object | null)[]): Set<string> {
  return new Set(objects.flatMap((object) => (object ? Object.getOwnPropertyNames(object) : [])));
}

export function reservedNameCollisions(
  sample: { solid: Solid; wire: Wire },
): { collisions: string[]; derived: number } {
  const reserved = new Set<string>();
  for (let object: object | null = new THREE.Group(); object; object = Reflect.getPrototypeOf(object)) {
    for (const key of Object.getOwnPropertyNames(object)) reserved.add(key);
  }
  const prototypes = ownNames([Reflect.getPrototypeOf(Wire.prototype), Wire.prototype, Solid.prototype]);
  prototypes.delete('constructor');
  const group = ownNames([new THREE.Group()]);
  const added = [...new Set([...prototypes, ...ownNames([sample.solid, sample.wire])])]
    .filter((name) => !group.has(name));
  return {
    collisions: added.filter((name) => reserved.has(name) && !OVERRIDES.has(name)),
    derived: added.length,
  };
}
