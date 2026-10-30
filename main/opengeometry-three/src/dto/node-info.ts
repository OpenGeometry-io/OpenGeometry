export type NodeInfo = {
  ogId: string;
  handle: number;
  generation: number;
  parent: string | null;
  children: string[];
  kind: string;
  shapeId: string | null;
  shapeRevision: number | null;
  bodyType: string | null;
};
