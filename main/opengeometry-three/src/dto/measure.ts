export type Measure = { volume: number; area: number; errorBound: number; areaErrorBound: number; deflection: number };

export type Validity = {
  valid: boolean;
  closed: boolean;
  bodyType: 'Wire' | 'Sheet' | 'Solid' | null;
  error?: { code: string; message: string };
};
