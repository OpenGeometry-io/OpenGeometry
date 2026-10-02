export type NodeChange = {
  ogId: string;
  handle: number;
  generation: number;
  shapeId: string | null;
  shapeRevision: number | null;
};

export type ChangeSet = {
  revision: number;
  added: NodeChange[];
  changed: NodeChange[];
  removed: NodeChange[];
};
