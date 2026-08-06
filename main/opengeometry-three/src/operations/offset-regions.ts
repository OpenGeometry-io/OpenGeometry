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

type KernelOffsetRingVariable = (
  ringFlat: Float64Array,
  distances: Float64Array,
) => KernelOffsetRegionsResult;

/**
 * Inset a CLOSED ring inward with one distance per edge (edge i =
 * `ring[i] → ring[i+1]`; an explicit closing duplicate point is accepted and
 * stripped, the distance count stays one per unique edge). Built for setback
 * envelopes: the result is the CLEARANCE-EXACT buildable region — the ring's
 * interior minus every point within `distances[i]` of edge i's SEGMENT, so
 * distant lot lines are honoured across notches and corners where distances
 * differ become circumscribed clearance arcs (conservative). There is no
 * `miterLimit` parameter, deliberately: miters and bevels only approximate the
 * clearance arc, and a bevel would claim points inside the required distance.
 *
 * Returns CW-outer / CCW-hole regions (canonical start vertex), possibly
 * SEVERAL when a deep inset splits a waisted ring into disjoint lobes, and an
 * EMPTY array when the inset collapses or the input ring is degenerate
 * (< 3 unique points, zero area, self-intersecting). THROWS on malformed
 * arguments: a distance count that does not match the edge count, or a
 * negative / non-finite distance — those are caller bugs, not geometry.
 *
 * @param ring closed ring points as `[x,y,z, …]` (Y is carried through).
 * @param distances inward inset distance per edge, metres, each >= 0
 *                  (0 = the edge stays in place, e.g. lot-line construction).
 */
export function offsetRingVariable(
  ring: number[] | Float64Array,
  distances: number[] | Float64Array,
): OffsetRegion[] {
  const buildExport = (OGKernel as Record<string, unknown>).offsetRingVariable;
  if (typeof buildExport !== "function") {
    throw new Error(
      "offsetRingVariable is not available in the loaded wasm package. It requires opengeometry >= 2.0.12 — rebuild or upgrade the opengeometry wasm bindings.",
    );
  }
  const flatRing = ring instanceof Float64Array ? ring : new Float64Array(ring);
  const flatDistances =
    distances instanceof Float64Array ? distances : new Float64Array(distances);
  const result = (buildExport as KernelOffsetRingVariable)(flatRing, flatDistances);
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
