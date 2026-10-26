export type OffsetRegion = {
  outer: { x: number; y: number; z: number }[];
  holes: { x: number; y: number; z: number }[][];
};

export type CurvedRegionEdge =
  | { kind: 'line'; from: [number, number]; to: [number, number] }
  | { kind: 'arc'; center: [number, number]; radius: number; start_angle: number; sweep_angle: number };

export type CurvedRegion2D = { outer: CurvedRegionEdge[]; holes: CurvedRegionEdge[][] };

export type RegionOperation = 'union' | 'intersection' | 'subtraction';
