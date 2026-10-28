export const OG_OPERATION_EXTRUDE = 'Extrude' as const;
export const OG_OPERATION_SWEEP = 'Sweep' as const;
export const OG_OPERATION_REVOLVE = 'Revolve' as const;
export const OG_OPERATION_UNION = 'Union' as const;
export const OG_OPERATION_SUBTRACT = 'Subtract' as const;
export const OG_OPERATION_INTERSECT = 'Intersect' as const;

export const OG_OPERATION_PARAMS_EXTRUDE = (
  value: { profile: { ogId: string }; holes?: { ogId: string }[]; distance: number },
): { profile: { ogId: string }; holes?: { ogId: string }[]; distance: number } => value;
export const OG_OPERATION_PARAMS_SWEEP = (
  value: { profile: { ogId: string }; path: { ogId: string }; caps?: boolean },
): { profile: { ogId: string }; path: { ogId: string }; caps?: boolean } => value;
export const OG_OPERATION_PARAMS_REVOLVE = (
  value: { profile: { ogId: string }; axis?: { point: [number, number]; direction: [number, number] } },
): { profile: { ogId: string }; axis?: { point: [number, number]; direction: [number, number] } } => value;
