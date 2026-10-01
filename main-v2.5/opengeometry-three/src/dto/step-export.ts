export type StepBodyReport = {
  ogId: string;
  shapeId: string;
  shapeRevision: number;
  solids: number;
  faces: number;
  edges: number;
  cavityShells: number;
  collapsedChartUses: number;
  fittedCurves: number;
  pcurvelessEdges: number;
  exchangeErrorBound: number;
  geometricTolerance: number;
};

export type StepSkipped = { ogId: string; reason: string };

export type StepExportReport = {
  unit: string;
  upAxis: string;
  timestamp: string;
  products: number;
  solids: number;
  faces: number;
  edges: number;
  cavityShells: number;
  collapsedChartUses: number;
  fittedCurves: number;
  pcurvelessEdges: number;
  entities: number;
  bytes: number;
  exchangeErrorBound: number;
  geometricTolerance: number;
  validationLevel: string;
  bodies: StepBodyReport[];
  skipped: StepSkipped[];
};

export type StepExport = { text: string; report: StepExportReport };
