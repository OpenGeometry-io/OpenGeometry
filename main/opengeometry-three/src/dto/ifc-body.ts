export type IfcPreparedValue =
  | { kind: 'Ref'; id: number }
  | { kind: 'List'; values: IfcPreparedValue[] }
  | { kind: 'Real'; value: number }
  | { kind: 'Integer'; value: number }
  | { kind: 'Text' | 'Enum'; value: string }
  | { kind: 'Bool'; value: boolean }
  | { kind: 'Null' | 'Derived' };

export type AnalyticExchangeBodyV2 = {
  schema_version: 2;
  revision: string;
  length_unit: 'metre';
  up_axis: 'Z';
  coordinate_space: 'world';
  representation_type: 'AdvancedBrep' | 'CSG';
  quality: { kind: 'Analytic' } | { kind: 'Approximate'; max_error: number };
  geometric_tolerance: number;
  exchange_error_bound: number;
  validation_level: string;
  entities: { id: number; entity_type: string; attributes: IfcPreparedValue[] }[];
  solids: number[];
  faces: {
    face: number;
    key: string;
    entity: number;
    provenance: {
      sources: { entity: string; body: string; key: string; face: number }[];
      role: 'Authored' | 'Preserved' | 'Split' | 'Cut' | 'Coincident';
      reversed: boolean;
    };
  }[];
};
