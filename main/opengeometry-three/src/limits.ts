export const ACCURACY = { geometric: 1e-8, intersection: 1e-9, tessellation: 0.01, exchange: 1e-6 } as const;
export const MAX_TRIANGLES = 2_000_000 as const;
export const HYSTERESIS_KEEP = [0.75, 4] as const;
export const PIXELS_AT_REST = 0.5 as const;
export const PIXELS_MOVING = 2 as const;
export const EVALUATION_INTERVAL_MS = 100 as const;
export const MOTION_WINDOW_MS = 150 as const;
