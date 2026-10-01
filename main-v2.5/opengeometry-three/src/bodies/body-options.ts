import type { SystemAssembly } from './system-assembly.js';

export type BodyOptions = {
  ogId?: string;
  parent?: SystemAssembly;
  plane?: {
    origin?: [number, number, number];
    normal?: [number, number, number];
    xDirection?: [number, number, number];
  };
  appearance?: Partial<Appearance>;
};
export type Appearance = {
  color: number;
  opacity: number;
  outline: boolean;
  pickOutline: boolean;
  deflection?: number;
};
