export type GeometryQuality = { kind: 'Analytic' } | { kind: 'Approximate'; max_error: number };

export type FaceSource = { entity: string; body: string; key: string; face: number };

export type BrepVertex = {
  id: number;
  position: [number, number, number];
  outgoing_halfedge: number | null;
  tolerance: number;
};

export type BrepEdge = {
  id: number;
  geometry: unknown;
  halfedge: number;
  twin_halfedge: number | null;
  tolerance: number;
  chart_seam: boolean;
};

export type BrepHalfedge = {
  id: number;
  from: number;
  to: number;
  twin: number | null;
  next: number | null;
  prev: number | null;
  edge: number;
  face: number | null;
  loop_ref: number | null;
  wire_ref: number | null;
  geometry_use: unknown;
};

export type BrepLoop = { id: number; start_halfedge: number; face_ref: number; is_hole: boolean };

export type BrepFace = {
  id: number;
  key: string;
  surface: number;
  sense: 'Forward' | 'Reverse';
  trim: unknown;
  shell_ref: number | null;
  provenance: {
    sources: FaceSource[];
    role: 'Authored' | 'Preserved' | 'Split' | 'Cut' | 'Coincident';
    reversed: boolean;
  };
};

export type BrepWire = { id: number; start_halfedge: number; is_closed: boolean };

export type BrepShell = { id: number; faces: number[]; is_closed: boolean };

export type Brep = {
  schema_version: number;
  id: string;
  revision: number;
  geometry: { surfaces: unknown[]; curves: unknown[]; pcurves: unknown[]; intersections: unknown[] };
  topology: {
    vertices: BrepVertex[];
    edges: BrepEdge[];
    halfedges: BrepHalfedge[];
    loops: BrepLoop[];
    faces: BrepFace[];
    wires: BrepWire[];
    shells: BrepShell[];
  };
  solids: { outer_shell: number; cavity_shells: number[] }[];
  accuracy: { geometric: number; intersection: number; tessellation: number; exchange: number };
  quality: GeometryQuality;
};
