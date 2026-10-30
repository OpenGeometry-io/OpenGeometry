export { OpenGeometry } from './src/opengeometry';
export { OGMark } from './src/marks/og-mark';
export { SystemAssembly } from './src/bodies/system-assembly';
export { Wire } from './src/bodies/wire';
export { Solid } from './src/bodies/solid';
export { Sheet } from './src/bodies/sheet';
export type { PointClassification } from './src/bodies/node-methods';
export type { Measure, Validity } from './src/dto/measure';
export type { MirrorPlane } from './src/dto/mirror';
export { OGError } from './src/errors';
export type { ErrorDetails } from './src/errors';
export type { OperateErrorDetails } from './src/dto/boolean-report';
export type { AnalyticExchangeBodyV2, IfcPreparedValue } from './src/dto/ifc-body';
export * from './src/constants/index';
export type {
  ArcParams, ConeParams, CylinderSectorParams, PolyhedronParams, ProfileArc, ProfileParams, ProfileSegment,
  SphereParams, TorusParams,
} from './src/dto/primitives';
export type {
  CurvedRegion2D, CurvedRegionEdge, OffsetHole, OffsetPath2D, OffsetPolylineOptions, OffsetRegion,
} from './src/dto/regions';
