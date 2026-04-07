import {
  OGFreeformGeometry,
  Vector3,
} from "../../../opengeometry/pkg/opengeometry";

import type {
  CreateFreeformGeometryOptions,
  FreeformSource,
  ObjectTransformation,
} from "./types";

function parseJson<T>(payload: string): T {
  return JSON.parse(payload) as T;
}

function toVector3(value: { x: number; y: number; z: number }): Vector3 {
  return new Vector3(value.x, value.y, value.z);
}

function toPlainVector3(value: { x: number; y: number; z: number }) {
  return {
    x: value.x,
    y: value.y,
    z: value.z,
  };
}

function cloneVector(
  vector: Vector3 | undefined,
  fallback: [number, number, number]
): Vector3 {
  return vector?.clone() ?? new Vector3(...fallback);
}

function normalizePlacement(
  placement: ObjectTransformation
): ObjectTransformation {
  return {
    anchor: toVector3(placement.anchor),
    translation: toVector3(placement.translation),
    rotation: toVector3(placement.rotation),
    scale: toVector3(placement.scale),
  };
}

function toObjectTransformation(
  placement: NonNullable<CreateFreeformGeometryOptions["placement"]>
): ObjectTransformation {
  return {
    anchor: cloneVector(placement.anchor, [0, 0, 0]),
    translation: cloneVector(placement.translation, [0, 0, 0]),
    rotation: cloneVector(placement.rotation, [0, 0, 0]),
    scale: cloneVector(placement.scale, [1, 1, 1]),
  };
}

function toPlainObjectTransformation(transform: ObjectTransformation) {
  return {
    anchor: toPlainVector3(transform.anchor),
    translation: toPlainVector3(transform.translation),
    rotation: toPlainVector3(transform.rotation),
    scale: toPlainVector3(transform.scale),
  };
}

function serializeBrepLike(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value);
}

function normalizeFreeformSerialized(source: FreeformSource): string {
  if (typeof source === "string") {
    return source;
  }

  if (typeof source.getLocalBrepSerialized === "function") {
    return source.getLocalBrepSerialized();
  }

  if (typeof source.getLocalBrepData === "function") {
    return serializeBrepLike(source.getLocalBrepData());
  }

  if (typeof source.getBrepSerialized === "function") {
    return source.getBrepSerialized();
  }

  if (typeof source.getBrepData === "function") {
    return serializeBrepLike(source.getBrepData());
  }

  if (typeof source.getBrep === "function") {
    return serializeBrepLike(source.getBrep());
  }

  return JSON.stringify(source);
}

/**
 * Thin client-side wrapper around the kernel `OGFreeformGeometry` object.
 */
export class FreeformGeometry {
  private readonly geometry: OGFreeformGeometry;

  constructor(geometry: OGFreeformGeometry) {
    this.geometry = geometry;
  }

  /**
   * Returns the stable kernel-side id for this freeform geometry object.
   */
  getId(): string {
    return this.geometry.id;
  }

  /**
   * Returns the world-space serialized BRep payload.
   */
  getBrepSerialized(): string {
    return this.geometry.getBrepSerialized();
  }

  /**
   * Returns the local-space serialized BRep payload.
   */
  getLocalBrepSerialized(): string {
    return this.geometry.getLocalBrepSerialized();
  }

  /**
   * Returns the serialized triangle position buffer emitted by the kernel.
   */
  getGeometrySerialized(): string {
    return this.geometry.getGeometrySerialized();
  }

  /**
   * Returns world-space triangle positions as a plain number array.
   */
  getGeometryBuffer(): number[] {
    return Array.from(this.geometry.getGeometryBuffer());
  }

  /**
   * Returns local-space triangle positions as a plain number array.
   */
  getLocalGeometryBuffer(): number[] {
    return Array.from(this.geometry.getLocalGeometryBuffer());
  }

  /**
   * Returns the serialized outline buffer emitted by the kernel.
   */
  getOutlineGeometrySerialized(): string {
    return this.geometry.getOutlineGeometrySerialized();
  }

  /**
   * Returns world-space outline segment positions as a plain number array.
   */
  getOutlineGeometryBuffer(): number[] {
    return Array.from(this.geometry.getOutlineGeometryBuffer());
  }

  /**
   * Returns local-space outline segment positions as a plain number array.
   */
  getLocalOutlineGeometryBuffer(): number[] {
    return Array.from(this.geometry.getLocalOutlineGeometryBuffer());
  }

  /**
   * Returns the current normalized placement for this freeform geometry.
   */
  getPlacement(): ObjectTransformation {
    return normalizePlacement(
      parseJson<ObjectTransformation>(this.geometry.getPlacementSerialized())
    );
  }

  /**
   * Replaces the full placement transform in one call.
   */
  setPlacement(transform: ObjectTransformation): void {
    this.geometry.setPlacementSerialized(
      JSON.stringify(toPlainObjectTransformation(transform))
    );
  }

  /**
   * Sets translation, rotation, and scale without changing the current anchor.
   */
  setTransform(translation: Vector3, rotation: Vector3, scale: Vector3): void {
    this.geometry.setTransform(translation, rotation, scale);
  }

  /**
   * Updates only the translation component of the placement.
   */
  setTranslation(translation: Vector3): void {
    this.geometry.setTranslation(translation);
  }

  /**
   * Updates only the rotation component of the placement.
   */
  setRotation(rotation: Vector3): void {
    this.geometry.setRotation(rotation);
  }

  /**
   * Updates only the scale component of the placement.
   */
  setScale(scale: Vector3): void {
    this.geometry.setScale(scale);
  }

  /**
   * Updates the explicit modeling anchor used by downstream transforms.
   */
  setAnchor(anchor: Vector3): void {
    this.geometry.setAnchor(anchor);
  }

  /** @internal */
  getKernelGeometry(): OGFreeformGeometry {
    return this.geometry;
  }
}

/**
 * Creates a `FreeformGeometry` wrapper from serialized BRep JSON, a parsed
 * BRep object, or another wrapper exposing BRep accessors.
 */
export function createFreeformGeometry(
  source: FreeformSource,
  options?: CreateFreeformGeometryOptions
): FreeformGeometry {
  const id = options?.id ?? `freeform-${Date.now()}`;
  const localBrepSerialized = normalizeFreeformSerialized(source);
  const geometry = new FreeformGeometry(
    new OGFreeformGeometry(id, localBrepSerialized)
  );

  if (options?.placement) {
    geometry.setPlacement(toObjectTransformation(options.placement));
  }

  return geometry;
}
