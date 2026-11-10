export type ProfileArc = { center: [number, number]; counterClockwise: boolean };
export type ProfileSegment = { to: [number, number]; arc?: ProfileArc };
export type ProfileParams = { start: [number, number]; segments: ProfileSegment[]; closed: boolean };
export type ArcParams = { radius: number; startAngle: number; sweepAngle: number };
export type CylinderSectorParams = { radius: number; height: number; startAngle: number; sweepAngle: number };
export type PolyhedronParams = { points: [number, number, number][]; faces: number[][] };
export type SphereParams = { radius: number };
export type ConeParams = { radiusBottom: number; radiusTop: number; height: number };
export type TorusParams = { majorRadius: number; minorRadius: number };
export type SlotPrismParams = { length: number; width: number; height: number };
export type RoundedBoxParams = { width: number; height: number; depth: number; radius: number };
export type PipeSegmentParams = {
  outerRadius: number; innerRadius: number; height: number; startAngle: number; sweepAngle: number;
};
export type PolysolidParams = {
  path: ProfileParams; width: number; height: number; justify?: 'centre' | 'left' | 'right';
};
export type RegularPolygonParams = { sides: number; radius: number };
export type SlotParams = { length: number; width: number };
export type RoundedRectangleParams = { width: number; breadth: number; radius: number };
