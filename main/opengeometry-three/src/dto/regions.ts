export type OffsetRegion = {
  outer: { x: number; y: number; z: number }[];
  holes: { x: number; y: number; z: number }[][];
};

export type CurvedRegionEdge =
  | { kind: 'line'; from: [number, number]; to: [number, number] }
  | { kind: 'arc'; center: [number, number]; radius: number; start_angle: number; sweep_angle: number };

export type CurvedRegion2D = { outer: CurvedRegionEdge[]; holes: CurvedRegionEdge[][] };

export type RegionOperation = 'union' | 'intersection' | 'subtraction';

export type OffsetHole = { ring: readonly number[] | Float64Array; distances: readonly number[] | Float64Array };

export type OffsetPolylineOptions = { acuteThresholdDegrees?: number; bevel?: boolean; closed?: boolean };

export type OffsetPath2D = { edges: CurvedRegionEdge[]; bevelled: number[]; closed: boolean };
