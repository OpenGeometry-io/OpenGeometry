import * as OGKernel from "../../../opengeometry/pkg/opengeometry";

/** A point of a returned offset region ring (kernel `Vector3` shape). */
export interface OffsetRegionPoint {
  x: number;
  y: number;
  z: number;
}

/** One offset region: a CW outer ring + its CCW inner-void hole rings. */
export interface OffsetRegion {
  /** Outer ring (CW, canonical start). */
  outer: OffsetRegionPoint[];
  /** Inner-void hole rings (CCW), e.g. the interior void of a closed loop. */
  holes: OffsetRegionPoint[][];
}

type KernelOffsetRegionsResult = {
  regionsSerialized: string;
};

type KernelOffsetPolylineRegions = (
  centreline: Float64Array,
  width: number,
  closed: boolean,
  miterLimit: number,
) => KernelOffsetRegionsResult;

/**
 * Offset a centreline by ±half-width into one or more filled REGIONS, via the
 * deterministic analytic offset — miters / flat bevels at `miterLimit`, concave
 * trims, and nonzero-winding resolution of reflex / closed loops. NO boolean union
 * (so it is deterministic, unlike the 3D-mesh CSG path). A simple centreline returns
 * one region; a self-crossing closed centreline (e.g. a figure-8) returns one per
 * simple sub-loop. Returns an EMPTY array for a genuinely unbuildable input (does
 * not throw, so it never looks like a kernel crash). Rings are CW-outer / CCW-holes.
 *
 * @param centrelineFlat centreline points as `[x,y,z, …]`.
 * @param width full stroke width (offset ±width/2 on each side).
 * @param miterLimit SVG stroke-miterlimit; corners sharper than this bevel. Default 4.
 */
export function offsetPolylineRegions(
  centrelineFlat: number[] | Float64Array,
  width: number,
  closed: boolean,
  miterLimit = 4,
): OffsetRegion[] {
  const buildExport = (OGKernel as Record<string, unknown>).offsetPolylineRegions;
  if (typeof buildExport !== "function") {
    throw new Error(
      "offsetPolylineRegions is not available in the loaded wasm package. Rebuild opengeometry wasm bindings.",
    );
  }
  const flat =
    centrelineFlat instanceof Float64Array ? centrelineFlat : new Float64Array(centrelineFlat);
  const result = (buildExport as KernelOffsetPolylineRegions)(flat, width, closed, miterLimit);
  return JSON.parse(result.regionsSerialized);
}

/** One polyline in an offset group: centreline `[x,y,z, …]`, stroke width, closed flag. */
export interface OffsetPolyline {
  centreline: number[];
  width: number;
  closed: boolean;
}

type KernelOffsetPolylineGroupRegions = (polylinesJson: string) => KernelOffsetRegionsResult;

/**
 * Merge a GROUP of separate polylines (a crossing T / X / L overlap) into one clean
 * region by nonzero-winding union of their mitered bands — the overlapping strokes
 * merge into a single region with mitered/bevelled corners, no internal edges.
 * Returns CW-outer / CCW-hole regions (one or more); empty if nothing is buildable.
 * No CSG. Used to render/extrude an overlapping crossing as one joined mass.
 */
export function offsetPolylineGroupRegions(polylines: OffsetPolyline[]): OffsetRegion[] {
  const buildExport = (OGKernel as Record<string, unknown>).offsetPolylineGroupRegions;
  if (typeof buildExport !== "function") {
    throw new Error(
      "offsetPolylineGroupRegions is not available in the loaded wasm package. Rebuild opengeometry wasm bindings.",
    );
  }
  const result = (buildExport as KernelOffsetPolylineGroupRegions)(JSON.stringify(polylines));
  return JSON.parse(result.regionsSerialized);
}
