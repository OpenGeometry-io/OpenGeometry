import * as OGKernel from "../../../opengeometry/pkg/opengeometry";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";
import { getUUID } from "../utils/randomizer";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  sanitizeOutlineWidth,
  ShapeOutlineMesh,
} from "./outline-utils";
import {
  clonePlacement,
  createParametricEditCapabilities,
} from "../editor";
import { createFreeformGeometry } from "../freeform";
import { subtractShapeOperand } from "./boolean-subtract";
import type {
  ShapeSubtractOperands,
  ShapeSubtractOptions,
  ShapeSubtractResult,
} from "./boolean-subtract";

/**
 * Construction options for a swept solid.
 */
export interface ISweepOptions {
  ogid?: string;
  path: Vector3[];
  profile: Vector3[];
  color: number;
  capStart?: boolean;
  capEnd?: boolean;
  fatOutlines?: boolean;
  outlineWidth?: number;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Placement updates accepted by `Sweep`.
 */
export interface SweepPlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Partial config payload accepted by `Sweep.setConfig(...)`.
 */
export type SweepConfigUpdate = Partial<
  Omit<ISweepOptions, "translation" | "rotation" | "scale">
>;

/**
 * Alias for `Sweep` placement updates.
 */
export type SweepPlacementUpdate = SweepPlacementOptions;

/* eslint-disable no-unused-vars */
interface ISweepKernelInstance {
  set_config_with_caps: (
    ..._args: [Vector3[], Vector3[], boolean, boolean]
  ) => void;
  set_transform: (..._args: [Vector3, Vector3, Vector3]) => void;
  set_anchor: (..._args: [Vector3]) => void;
  reset_anchor: () => void;
  get_geometry_buffer(): Float64Array;
  get_brep_serialized(): string;
  get_local_brep_serialized(): string;
  get_outline_geometry_buffer(): Float64Array;
  get_anchor(): Vector3;
}

type SweepKernelConstructor = new (..._args: [string]) => ISweepKernelInstance;
/* eslint-enable no-unused-vars */

/**
 * Sweep wrapper backed by the kernel sweep primitive.
 */
export class Sweep extends THREE.Mesh {
  ogid: string;
  options: ISweepOptions = {
    path: [
      new Vector3(0, 0, 0),
      new Vector3(0, 1, 0),
    ],
    profile: [
      new Vector3(-0.25, 0, -0.25),
      new Vector3(0.25, 0, -0.25),
      new Vector3(0.25, 0, 0.25),
      new Vector3(-0.25, 0, 0.25),
    ],
    color: 0x00ff00,
    capStart: true,
    capEnd: true,
    fatOutlines: false,
    outlineWidth: 1,
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
  };

  private sweep: ISweepKernelInstance;
  #outlineMesh: ShapeOutlineMesh | null = null;
  private _outlineEnabled = false;
  private _fatOutlines = false;
  private _outlineWidth = 1;

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: ISweepOptions) {
    super();

    this.ogid = options?.ogid ?? getUUID();
    const sweepExport = (OGKernel as Record<string, unknown>)["OGSweep"];
    if (typeof sweepExport !== "function") {
      throw new Error(
        "OGSweep is not available in the loaded wasm package. Rebuild opengeometry wasm bindings."
      );
    }
    const SweepKernel = sweepExport as SweepKernelConstructor;
    this.sweep = new SweepKernel(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig({
      path: this.options.path.map((point) => point.clone()),
      profile: this.options.profile.map((point) => point.clone()),
      color: this.options.color,
      capStart: this.options.capStart,
      capEnd: this.options.capEnd,
      fatOutlines: this.options.fatOutlines,
      outlineWidth: this.options.outlineWidth,
    });
    this.setPlacement({
      translation: this.options.translation?.clone(),
      rotation: this.options.rotation?.clone(),
      scale: this.options.scale?.clone(),
    });
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Sweep");
    }

    if (this.options.path.length < 2) {
      throw new Error("Sweep path requires at least 2 points.");
    }

    if (this.options.profile.length < 3) {
      throw new Error("Sweep profile requires at least 3 points.");
    }
  }

  setConfig(options: SweepConfigUpdate) {
    const nextOptions = { ...this.options, ...options };
    const geometryChanged =
      "path" in options ||
      "profile" in options ||
      "capStart" in options ||
      "capEnd" in options;
    const colorChanged = "color" in options;
    const outlineStyleChanged =
      "fatOutlines" in options ||
      "outlineWidth" in options;

    this.options = nextOptions;
    this._fatOutlines = this.options.fatOutlines ?? false;
    this._outlineWidth = sanitizeOutlineWidth(this.options.outlineWidth);
    this.options.fatOutlines = this._fatOutlines;
    this.options.outlineWidth = this._outlineWidth;

    if (geometryChanged) {
      this.validateOptions();

      const path = this.options.path.map((point) => point.clone());
      const profile = this.options.profile.map((point) => point.clone());
      const capStart = this.options.capStart ?? true;
      const capEnd = this.options.capEnd ?? true;

      this.sweep.set_config_with_caps(path, profile, capStart, capEnd);
      this.generateGeometry();
      return;
    }

    if (colorChanged) {
      this.color = this.options.color;
    }

    if (outlineStyleChanged && this._outlineEnabled) {
      this.outline = true;
    }
  }

  getConfig() {
    return this.options;
  }

  getAnchor() {
    const anchor = this.sweep.get_anchor();
    return new Vector3(anchor.x, anchor.y, anchor.z);
  }

  setAnchor(anchor: Vector3) {
    this.sweep.set_anchor(anchor.clone());
    this.generateGeometry();
  }

  resetAnchor() {
    this.sweep.reset_anchor();
    this.generateGeometry();
  }

  setPlacement(placement: SweepPlacementUpdate) {
    this.options.translation = placement.translation?.clone() ?? this.options.translation;
    this.options.rotation = placement.rotation?.clone() ?? this.options.rotation;
    this.options.scale = placement.scale?.clone() ?? this.options.scale;

    this.sweep.set_transform(
      this.options.translation?.clone() ?? new Vector3(0, 0, 0),
      this.options.rotation?.clone() ?? new Vector3(0, 0, 0),
      this.options.scale?.clone() ?? new Vector3(1, 1, 1)
    );
    this.generateGeometry();
  }

  setTransform(translation: Vector3, rotation: Vector3, scale: Vector3) {
    this.setPlacement({
      translation,
      rotation,
      scale,
    });
  }

  setTranslation(translation: Vector3) {
    this.setPlacement({ translation });
  }

  setRotation(rotation: Vector3) {
    this.setPlacement({ rotation });
  }

  setScale(scale: Vector3) {
    this.setPlacement({ scale });
  }

  getPlacement() {
    return clonePlacement({
      translation: this.options.translation,
      rotation: this.options.rotation,
      scale: this.options.scale,
    });
  }

  getEditCapabilities() {
    return createParametricEditCapabilities("sweep", "sweep");
  }

  canConvertToFreeform() {
    return true;
  }

  toFreeform(id: string = this.ogid) {
    if (!this.canConvertToFreeform()) {
      throw new Error("This entity cannot be converted to freeform.");
    }
    return createFreeformGeometry(this.sweep.get_local_brep_serialized(), {
      id,
      placement: this.getPlacement(),
    });
  }

  cleanGeometry() {
    this.geometry.dispose();
    if (Array.isArray(this.material)) {
      this.material.forEach((mat) => mat.dispose());
    } else {
      this.material.dispose();
    }
  }

  generateGeometry() {
    const bufferData = Array.from(this.sweep.get_geometry_buffer());

    this.writePositionsToGeometry(this.geometry, bufferData);

    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(this.options.color);
      this.material.transparent = true;
      this.material.opacity = 0.6;
    } else {
      if (Array.isArray(this.material)) {
        this.material.forEach((material) => material.dispose());
      } else {
        this.material.dispose();
      }

      this.material = new THREE.MeshStandardMaterial({
        color: this.options.color,
        transparent: true,
        opacity: 0.6,
      });
    }

    this.geometry.computeVertexNormals();
    this.geometry.computeBoundingBox();
    this.geometry.computeBoundingSphere();

    if (!this._outlineEnabled) {
      return;
    }

    const outlineData = Array.from(this.sweep.get_outline_geometry_buffer());
    if (!this.#outlineMesh) {
      this.outline = true;
      return;
    }

    if (this.#outlineMesh instanceof THREE.LineSegments) {
      this.writePositionsToGeometry(this.#outlineMesh.geometry, outlineData);
      this.#outlineMesh.geometry.computeBoundingBox();
      this.#outlineMesh.geometry.computeBoundingSphere();
      return;
    }

    const fatGeometry = this.#outlineMesh.geometry as LineSegmentsGeometry;
    fatGeometry.setPositions(outlineData);

    const fatOutline = this.#outlineMesh as ShapeOutlineMesh & {
      computeLineDistances?: () => void;
    };
    fatOutline.computeLineDistances?.();
  }

  getBrep() {
    const brepData = this.sweep.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for Sweep.");
    }
    return JSON.parse(brepData);
  }

  /**
   * Subtracts one or more boolean operands, such as Opening volumes, from this sweep.
   */
  subtract(
    operands: ShapeSubtractOperands,
    options?: ShapeSubtractOptions
  ): ShapeSubtractResult {
    return subtractShapeOperand(this, operands, options);
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (enable) {
      const outlineData = Array.from(this.sweep.get_outline_geometry_buffer());
      this.#outlineMesh = createShapeOutlineMesh({
        positions: outlineData,
        color: 0x000000,
        fatOutlines: this._fatOutlines,
        outlineWidth: this._outlineWidth,
      });

      this.add(this.#outlineMesh);
    }
  }

  get outline() {
    return this._outlineEnabled;
  }

  set fatOutlines(value: boolean) {
    this._fatOutlines = value;
    this.options.fatOutlines = value;
    if (this._outlineEnabled) {
      this.outline = true;
    }
  }

  get fatOutlines() {
    return this._fatOutlines;
  }

  set outlineWidth(value: number) {
    this._outlineWidth = sanitizeOutlineWidth(value);
    this.options.outlineWidth = this._outlineWidth;
    if (this._outlineEnabled) {
      this.outline = true;
    }
  }

  get outlineWidth() {
    return this._outlineWidth;
  }

  private clearOutlineMesh() {
    if (!this.#outlineMesh) {
      return;
    }
    this.remove(this.#outlineMesh);
    disposeShapeOutlineMesh(this.#outlineMesh);
    this.#outlineMesh = null;
  }

  discardGeometry() {
    this.geometry.dispose();
  }

  private writePositionsToGeometry(
    geometry: THREE.BufferGeometry,
    positions: number[]
  ) {
    const existing = geometry.getAttribute("position");
    if (
      !(existing instanceof THREE.BufferAttribute) ||
      existing.itemSize !== 3 ||
      existing.count !== positions.length / 3
    ) {
      geometry.setAttribute(
        "position",
        new THREE.Float32BufferAttribute(positions, 3)
      );
      return;
    }

    const array = existing.array as Float32Array;
    array.set(positions);
    existing.needsUpdate = true;
  }
}
