export const OG_OPERATION_EXTRUDE = 'Extrude' as const;
export const OG_OPERATION_SWEEP = 'Sweep' as const;
export const OG_OPERATION_UNION = 'Union' as const;
export const OG_OPERATION_SUBTRACT = 'Subtract' as const;
export const OG_OPERATION_INTERSECT = 'Intersect' as const;

export const OG_OPERATION_PARAMS_EXTRUDE = (
  value: { profile: { ogId: string }; holes?: { ogId: string }[]; distance: number },
): { profile: { ogId: string }; holes?: { ogId: string }[]; distance: number } => value;
export const OG_OPERATION_PARAMS_SWEEP = (
  value: { profile: { ogId: string }; path: { ogId: string } },
): { profile: { ogId: string }; path: { ogId: string } } => value;
