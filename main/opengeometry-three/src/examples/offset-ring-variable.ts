import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { Polyline } from "../primitives/polyline";
import { offsetRingVariable } from "../operations/offset-regions";

/** Close a kernel ring for Polyline rendering (repeat the first point). */
function closedRingPoints(ring: { x: number; y: number; z: number }[]): Vector3[] {
  const points = ring.map((point) => new Vector3(point.x, point.y, point.z));
  if (points.length > 0) {
    const first = ring[0];
    points.push(new Vector3(first.x, first.y, first.z));
  }
  return points;
}

/**
 * Variable (per-edge) ring inset: a rectangular "parcel" inset by front 6 /
 * side 1.5 / rear 3 setbacks, and a waisted (dumbbell) parcel whose uniform
 * 3 m inset SPLITS into two disjoint buildable envelopes — the case a uniform
 * offset cannot express.
 */
export function createOffsetRingVariableExample(scene: THREE.Scene) {
  const scale = 0.2; // metres → scene units, keeps both parcels in frame

  const parcel = [0, 0, 0, 20, 0, 0, 20, 0, 30, 0, 0, 30].map((v) => v * scale);
  const parcelSetbacks = [6, 1.5, 3, 1.5].map((v) => v * scale);

  const dumbbell = [
    0, 0, 0, 20, 0, 0, 20, 0, 14, 12, 0, 14, 12, 0, 26, 20, 0, 26, 20, 0, 40, 0, 0, 40, 0, 0, 26,
    8, 0, 26, 8, 0, 14, 0, 0, 14,
  ].map((v, index) => (index % 3 === 0 ? v * scale + 6 : v * scale)); // shifted +x
  const dumbbellSetbacks = new Array(12).fill(3 * scale);

  const parcelOutline = new Polyline({
    points: closedRingPoints(
      Array.from({ length: parcel.length / 3 }, (_, i) => ({
        x: parcel[i * 3],
        y: parcel[i * 3 + 1],
        z: parcel[i * 3 + 2],
      })),
    ),
    color: 0x1f2937,
  });
  scene.add(parcelOutline);

  const dumbbellOutline = new Polyline({
    points: closedRingPoints(
      Array.from({ length: dumbbell.length / 3 }, (_, i) => ({
        x: dumbbell[i * 3],
        y: dumbbell[i * 3 + 1],
        z: dumbbell[i * 3 + 2],
      })),
    ),
    color: 0x1f2937,
  });
  scene.add(dumbbellOutline);

  const envelopes: Polyline[] = [];
  for (const region of offsetRingVariable(parcel, parcelSetbacks)) {
    const envelope = new Polyline({ points: closedRingPoints(region.outer), color: 0x16a34a });
    envelopes.push(envelope);
    scene.add(envelope);
  }
  for (const region of offsetRingVariable(dumbbell, dumbbellSetbacks)) {
    const envelope = new Polyline({ points: closedRingPoints(region.outer), color: 0xdc2626 });
    envelopes.push(envelope);
    scene.add(envelope);
  }

  return { parcelOutline, dumbbellOutline, envelopes };
}
