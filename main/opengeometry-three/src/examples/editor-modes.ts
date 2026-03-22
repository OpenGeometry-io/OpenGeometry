import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import {
  FreeformGeometry,
  FreeformEditResult,
} from "../freeform";
import { Polygon } from "../shapes/polygon";
import { Cuboid } from "../shapes/cuboid";

function topFaceId(freeform: FreeformGeometry): number | null {
  let bestFaceId: number | null = null;
  let bestY = Number.NEGATIVE_INFINITY;

  for (const face of freeform.getTopologyRenderData().faces) {
    const info = freeform.getFaceInfo(face.face_id);
    if (info.centroid.y > bestY) {
      bestY = info.centroid.y;
      bestFaceId = face.face_id;
    }
  }

  return bestFaceId;
}

/**
 * Demonstrates the intended editing flow for editor-controls:
 * parametric config/placement edits first, then explicit freeform conversion.
 */
export function createEditorModesExample(scene: THREE.Scene) {
  const polygon = new Polygon({
    vertices: [
      new Vector3(-3.2, 0.0, -0.8),
      new Vector3(-2.2, 0.0, 0.4),
      new Vector3(-1.0, 0.0, 0.0),
      new Vector3(-1.5, 0.0, -1.2),
    ],
    color: 0x2563eb,
    fatOutlines: true,
    outlineWidth: 4,
  });
  polygon.outline = true;

  const polygonCapabilities = polygon.getEditCapabilities();
  polygon.setConfig({
    vertices: [
      new Vector3(-3.4, 0.0, -0.9),
      new Vector3(-2.0, 0.0, 0.6),
      new Vector3(-0.8, 0.0, -0.1),
      new Vector3(-1.6, 0.0, -1.4),
    ],
  });
  polygon.setPlacement({
    translation: new Vector3(0.0, 0.0, -0.2),
  });

  const cuboid = new Cuboid({
    center: new Vector3(1.2, 0.8, 0.0),
    width: 1.2,
    height: 1.4,
    depth: 1.0,
    color: 0x10b981,
    fatOutlines: true,
    outlineWidth: 4,
  });
  cuboid.outline = true;

  const cuboidCapabilities = cuboid.getEditCapabilities();
  cuboid.setConfig({
    width: 1.6,
    height: 1.8,
  });
  cuboid.setPlacement({
    translation: new Vector3(0.2, 0.0, 0.0),
    rotation: new Vector3(0.0, Math.PI / 9.0, 0.0),
  });

  const freeform = cuboid.toFreeform(`${cuboid.ogid}-freeform`);
  const topFace = topFaceId(freeform);
  let freeformResult: FreeformEditResult | null = null;

  if (topFace !== null) {
    freeformResult = freeform.pushPullFace(topFace, 0.35, {
      includeTopologyRemap: true,
    });
  }

  scene.add(polygon);
  scene.add(cuboid);

  return {
    polygon,
    cuboid,
    polygonCapabilities,
    cuboidCapabilities,
    freeform,
    freeformResult,
  };
}
