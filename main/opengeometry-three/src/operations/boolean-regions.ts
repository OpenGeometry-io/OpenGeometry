import * as OGKernel from "../../../opengeometry/pkg/opengeometry";
import type { OffsetRegion, OffsetRegionPoint } from "./offset-regions";

export type PlanarBooleanOperation = "union" | "intersection" | "subtraction";

export type CurvedRegionEdge =
  | { kind: "line"; from: [number, number]; to: [number, number] }
  | { kind: "arc"; center: [number, number]; radius: number; start_angle: number; sweep_angle: number };

export interface CurvedRegion2D {
  outer: CurvedRegionEdge[];
  holes: CurvedRegionEdge[][];
}

type KernelRegionsResult = { regionsSerialized: string };
type KernelBooleanRegions = (
  aJson: string,
  bJson: string,
  operation: PlanarBooleanOperation,
) => KernelRegionsResult;

export function booleanRegions2D(
  a: readonly OffsetRegion[],
  b: readonly OffsetRegion[],
  operation: PlanarBooleanOperation,
): OffsetRegion[] {
  const kernelExport = (OGKernel as Record<string, unknown>).booleanRegions2D;
  if (typeof kernelExport !== "function") {
    throw new Error("booleanRegions2D is unavailable in the loaded WASM package");
  }
  const point = (value: OffsetRegionPoint): [number, number, number] => [
    value.x,
    value.y,
    value.z,
  ];
  const encode = (regions: readonly OffsetRegion[]) =>
    JSON.stringify(
      regions.map((region) => ({
        outer: region.outer.map(point),
        holes: region.holes.map((hole) => hole.map(point)),
      })),
    );
  const result = (kernelExport as KernelBooleanRegions)(encode(a), encode(b), operation);
  return JSON.parse(result.regionsSerialized) as OffsetRegion[];
}

/** Analytic line/arc region Boolean; output arcs retain their exact circles. */
export function booleanCurvedRegions2D(
  a: readonly CurvedRegion2D[],
  b: readonly CurvedRegion2D[],
  operation: PlanarBooleanOperation,
): CurvedRegion2D[] {
  const kernelExport = (OGKernel as Record<string, unknown>).booleanCurvedRegions2D;
  if (typeof kernelExport !== "function") {
    throw new Error("booleanCurvedRegions2D is unavailable in the loaded WASM package");
  }
  const result = (kernelExport as (a: string, b: string, operation: PlanarBooleanOperation) => string)(
    JSON.stringify(a), JSON.stringify(b), operation,
  );
  return JSON.parse(result) as CurvedRegion2D[];
}
