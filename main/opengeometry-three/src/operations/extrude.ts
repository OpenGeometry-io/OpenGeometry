import * as OGKernel from "../../../opengeometry/pkg/opengeometry";

import type { FreeformSource } from "../freeform";

/* eslint-disable no-unused-vars */
type KernelExtrudeBrepFace = (...args: [string, number]) => string;
/* eslint-enable no-unused-vars */

/**
 * Sources accepted by the BRep-face extrusion helper.
 */
export type ExtrudeBrepSource = FreeformSource;

/**
 * Extrudes the first face or wire in a local BRep payload into a closed solid BRep.
 */
export function extrudeBrepFace(
  source: ExtrudeBrepSource,
  height: number
): string {
  const extrudeExport = (OGKernel as Record<string, unknown>).extrudeBrepFace;
  if (typeof extrudeExport !== "function") {
    throw new Error(
      "extrudeBrepFace is not available in the loaded wasm package. Rebuild opengeometry wasm bindings."
    );
  }

  const kernelFunction = extrudeExport as KernelExtrudeBrepFace;
  return kernelFunction(resolveLocalBrepSerialized(source), height);
}

function resolveLocalBrepSerialized(source: ExtrudeBrepSource): string {
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

function serializeBrepLike(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value);
}
