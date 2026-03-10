export const OPEN_GEOMETRY_THREE_VERSION = '0.0.6';

export enum AXISMODE {
  ZFRONT = "z-front",
  ZUP = "z-up",
}

export interface OpenGeometryOptions {
  wasmURL?: string;
  axisMode?: AXISMODE;
}
