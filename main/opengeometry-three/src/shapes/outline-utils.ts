import * as THREE from "three";
import { LineMaterial } from "three/examples/jsm/lines/LineMaterial.js";
import { LineSegments2 } from "three/examples/jsm/lines/LineSegments2.js";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";

/**
 * Outline mesh variants used by shape wrappers.
 */
export type ShapeOutlineMesh = THREE.LineSegments | LineSegments2;

/**
 * Options for building a shape outline mesh.
 */
export interface ShapeOutlineMeshOptions {
  positions: number[];
  color?: number;
  fatOutlines?: boolean;
  outlineWidth?: number;
}

const DEFAULT_OUTLINE_COLOR = 0x000000;
const DEFAULT_OUTLINE_WIDTH = 1;

function getOutlineResolution() {
  if (typeof window === "undefined") {
    return new THREE.Vector2(1, 1);
  }
  return new THREE.Vector2(window.innerWidth, window.innerHeight);
}

/**
 * Clamps outline width inputs to the supported positive finite range.
 */
export function sanitizeOutlineWidth(width?: number) {
  if (!Number.isFinite(width) || typeof width !== "number" || width <= 0) {
    return DEFAULT_OUTLINE_WIDTH;
  }
  return width;
}

/**
 * Creates either a thin or fat outline mesh from line segment positions.
 */
export function createShapeOutlineMesh({
  positions,
  color = DEFAULT_OUTLINE_COLOR,
  fatOutlines = false,
  outlineWidth = DEFAULT_OUTLINE_WIDTH,
}: ShapeOutlineMeshOptions): ShapeOutlineMesh {
  if (fatOutlines) {
    const fatGeometry = new LineSegmentsGeometry();
    fatGeometry.setPositions(positions);
    const fatMaterial = new LineMaterial({
      color,
      linewidth: sanitizeOutlineWidth(outlineWidth),
      resolution: getOutlineResolution(),
    });
    const fatMesh = new LineSegments2(fatGeometry, fatMaterial);
    fatMesh.computeLineDistances();
    return fatMesh;
  }

  const outlineGeometry = new THREE.BufferGeometry();
  outlineGeometry.setAttribute(
    "position",
    new THREE.Float32BufferAttribute(positions, 3)
  );

  const outlineMaterial = new THREE.LineBasicMaterial({ color });
  return new THREE.LineSegments(outlineGeometry, outlineMaterial);
}

/**
 * Disposes the geometry and material owned by an outline mesh.
 */
export function disposeShapeOutlineMesh(mesh: ShapeOutlineMesh | null) {
  if (!mesh) {
    return;
  }

  mesh.geometry.dispose();
  if (Array.isArray(mesh.material)) {
    mesh.material.forEach((material) => material.dispose());
    return;
  }
  mesh.material.dispose();
}

/**
 * Updates the color of an existing outline mesh in place.
 */
export function setShapeOutlineColor(mesh: ShapeOutlineMesh | null, color: number) {
  if (!mesh) {
    return;
  }

  if (mesh.material instanceof THREE.LineBasicMaterial) {
    mesh.material.color.set(color);
    return;
  }

  if (mesh.material instanceof LineMaterial) {
    mesh.material.color.set(color);
  }
}

/**
 * Reads the current outline color, falling back when no outline exists.
 */
export function getShapeOutlineColor(
  mesh: ShapeOutlineMesh | null,
  fallback: number = DEFAULT_OUTLINE_COLOR
) {
  if (!mesh) {
    return fallback;
  }

  if (mesh.material instanceof THREE.LineBasicMaterial) {
    return mesh.material.color.getHex();
  }

  if (mesh.material instanceof LineMaterial) {
    return mesh.material.color.getHex();
  }

  return fallback;
}
