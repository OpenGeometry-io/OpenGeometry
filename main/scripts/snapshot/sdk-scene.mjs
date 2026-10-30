import * as THREE from 'three';
import {
  OpenGeometry, SystemAssembly, Wire, Solid,
  OG_PRIMITIVE_RECTANGLE, OG_PRIMITIVE_PARAMS_RECTANGLE,
  OG_PRIMITIVE_CIRCLE, OG_PRIMITIVE_PARAMS_CIRCLE,
  OG_PRIMITIVE_POLYLINE, OG_PRIMITIVE_PARAMS_POLYLINE,
  OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_PARAMS_CUBOID,
  OG_OPERATION_EXTRUDE, OG_OPERATION_PARAMS_EXTRUDE,
  OG_OPERATION_SWEEP, OG_OPERATION_PARAMS_SWEEP,
  OG_OPERATION_SUBTRACT,
  OG_TRANSFORM_TRANSLATE, OG_TRANSFORM_ROTATE, OG_TRANSFORM_PLACE,
} from '../../dist/index.js';
import { outcome } from './stable-output.mjs';

const PICK_ORIGIN = [-1.5, 5, 5];
const PICK_DIRECTION = [0, 0, -1];

export function stepLog() {
  const entries = [];
  return {
    entries,
    run(step, fn) {
      const result = outcome(fn);
      entries.push({ step, ...result, ok: !('error' in result) });
      return result.value;
    },
  };
}

function wallParams(profile, distance) {
  return OG_OPERATION_PARAMS_EXTRUDE({ profile, distance });
}

export function buildScene(log) {
  const scene = new THREE.Scene();
  const level = new SystemAssembly({ ogId: 'level-1' });
  const profile = new Wire(OG_PRIMITIVE_RECTANGLE, OG_PRIMITIVE_PARAMS_RECTANGLE({ width: 6, breadth: 0.2 }));
  const wall = new Solid(OG_OPERATION_EXTRUDE, wallParams(profile, 3), { ogId: 'wall-1' });
  const cutterParams = OG_PRIMITIVE_PARAMS_CUBOID({ width: 0.9, height: 2.1, depth: 0.4 });
  const cutter = new Solid(OG_PRIMITIVE_CUBOID, cutterParams, { ogId: 'door-cutter' });
  log.run('translate cutter', () => cutter.transform(OG_TRANSFORM_TRANSLATE, { offset: [2, 0, 0] }));
  log.run('door cut', () => wall.operate(OG_OPERATION_SUBTRACT, { tools: [cutter] }));
  const path = new Wire(OG_PRIMITIVE_POLYLINE, OG_PRIMITIVE_PARAMS_POLYLINE({
    points: [[0, 1, 1], [4, 1, 1], [4, 1, 4]], closed: false,
  }));
  const disc = new Wire(OG_PRIMITIVE_CIRCLE, OG_PRIMITIVE_PARAMS_CIRCLE({ radius: 0.05 }), {
    plane: { origin: [0, 1, 1], normal: [1, 0, 0], xDirection: [0, 0, 1] },
  });
  const rail = new Solid(OG_OPERATION_SWEEP, OG_OPERATION_PARAMS_SWEEP({ profile: disc, path }), { ogId: 'rail-1' });
  log.run('add members', () => level.addChild([profile, wall, cutter, path, disc, rail]));
  scene.add(wall, rail);
  return { scene, level, profile, wall, cutter, path, disc, rail };
}

export function editScene(log, parts) {
  const { level, profile, wall, cutter, rail } = parts;
  log.run('place level', () => level.transform(OG_TRANSFORM_PLACE, { origin: [0, 3, 0] }));
  log.run('rotate rail', () => rail.transform(OG_TRANSFORM_ROTATE, { axis: [0, 1, 0], degrees: 90 }));
  log.run('rebuild wall', () => wall.rebuild(OG_OPERATION_EXTRUDE, wallParams(profile, 4)));
  log.run('re-cut', () => wall.operate(OG_OPERATION_SUBTRACT, { tools: [cutter] }));
  log.run('rail minus wall', () => rail.operate(OG_OPERATION_SUBTRACT, { tools: [wall] }));
}

export function trialEdit(log, parts) {
  const { profile, wall } = parts;
  const mark = OpenGeometry.mark();
  log.run('mark stats with mark', () => OpenGeometry.markStats());
  log.run('trial rebuild', () => wall.rebuild(OG_OPERATION_EXTRUDE, wallParams(profile, 5)));
  log.run('rollback', () => mark.rollback());
  log.run('release', () => mark.release());
  log.run('release again', () => mark.release());
  const dryRun = () => wall.rebuild(OG_OPERATION_EXTRUDE, wallParams(profile, 6));
  log.run('dry-run transaction', () => OpenGeometry.transaction(dryRun, { dryRun: true }));
  log.run('mark stats after release', () => OpenGeometry.markStats());
}

export function copyAndDispose(log, parts) {
  const copy = parts.rail.instance({ ogId: 'rail-2' });
  log.run('translate copy', () => copy.transform(OG_TRANSFORM_TRANSLATE, { offset: [0, 0, 2] }));
  parts.scene.add(copy);
  log.run('dispose cutter', () => parts.cutter.dispose());
  return copy;
}

export function pickWall(parts) {
  parts.scene.updateMatrixWorld(true);
  const raycaster = new THREE.Raycaster(new THREE.Vector3(...PICK_ORIGIN), new THREE.Vector3(...PICK_DIRECTION));
  const hits = raycaster.intersectObjects(parts.scene.children, true);
  return outcome(() => {
    const hit = hits[0];
    if (!hit) throw new Error('the pick ray hit nothing');
    return {
      origin: PICK_ORIGIN,
      direction: PICK_DIRECTION,
      distance: hit.distance,
      point: hit.point.toArray(),
      faceIndex: hit.faceIndex,
      resolved: OpenGeometry.resolveHit(hit),
    };
  });
}
