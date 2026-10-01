import type { FaceSource, GeometryQuality } from './brep.js';

export type FaceMapping = { source: FaceSource; result_faces: number[] };

export type BooleanReport = {
  operation: 'union' | 'intersection' | 'subtraction';
  quality: GeometryQuality;
  contacts: [number, number, number][];
  coincident: boolean;
  face_mappings: FaceMapping[];
};

export type ShapeReport = { report: BooleanReport; handlers: string[]; revision: number };
