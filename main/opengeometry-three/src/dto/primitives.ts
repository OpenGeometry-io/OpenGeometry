export type ProfileArc = { center: [number, number]; counterClockwise: boolean };
export type ProfileSegment = { to: [number, number]; arc?: ProfileArc };
export type ProfileParams = { start: [number, number]; segments: ProfileSegment[]; closed: boolean };
export type ArcParams = { radius: number; startAngle: number; sweepAngle: number };
export type CylinderSectorParams = { radius: number; height: number; startAngle: number; sweepAngle: number };
export type PolyhedronParams = { points: [number, number, number][]; faces: number[][] };
