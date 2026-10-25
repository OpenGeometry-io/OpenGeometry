import type { ArcParams, ProfileParams } from '../dto/primitives';

export const OG_PRIMITIVE_RECTANGLE = 'Rectangle' as const;
export const OG_PRIMITIVE_CIRCLE = 'Circle' as const;
export const OG_PRIMITIVE_POLYLINE = 'Polyline' as const;
export const OG_PRIMITIVE_CUBOID = 'Cuboid' as const;
export const OG_PRIMITIVE_CYLINDER = 'Cylinder' as const;
export const OG_PRIMITIVE_PROFILE = 'Profile' as const;
export const OG_PRIMITIVE_ARC = 'Arc' as const;

export const OG_PRIMITIVE_PARAMS_RECTANGLE = (
  value: { width: number; breadth: number },
): { width: number; breadth: number } => value;
export const OG_PRIMITIVE_PARAMS_CIRCLE = (value: { radius: number }): { radius: number } => value;
export const OG_PRIMITIVE_PARAMS_POLYLINE = (
  value: { points: [number, number, number][]; closed?: boolean },
): { points: [number, number, number][]; closed?: boolean } => value;
export const OG_PRIMITIVE_PARAMS_CUBOID = (
  value: { width: number; height: number; depth: number },
): { width: number; height: number; depth: number } => value;
export const OG_PRIMITIVE_PARAMS_CYLINDER = (
  value: { radius: number; height: number },
): { radius: number; height: number } => value;
export const OG_PRIMITIVE_PARAMS_PROFILE = (value: ProfileParams): ProfileParams => value;
export const OG_PRIMITIVE_PARAMS_ARC = (value: ArcParams): ArcParams => value;
