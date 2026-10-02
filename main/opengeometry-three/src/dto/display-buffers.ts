export type DisplayBuffers = {
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
  faceRanges: Uint32Array;
  outline: Float32Array;
  edgeIds: Uint32Array;
  origin: Float64Array;
  revision: number;
  bucket: number;
  achievedDeflection: number;
  triangles: number;
};
