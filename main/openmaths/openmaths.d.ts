/* tslint:disable */
/* eslint-disable */
/**
* @param {Mesh} mesh
* @returns {Float64Array}
*/
export function triangulate_mesh(mesh: Mesh): Float64Array;
/**
* @param {(Vector3D)[]} vertices
* @returns {(Vector3D)[]}
*/
export function triangulate(vertices: (Vector3D)[]): (Vector3D)[];
/**
* @returns {string}
*/
export function get_tricut_vertices(): string;
/**
*/
export class ColorRGBA {
  free(): void;
/**
*/
  a: number;
/**
*/
  b: number;
/**
*/
  g: number;
/**
*/
  r: number;
}
/**
*/
export class Matrix3D {
  free(): void;
/**
* @param {number} m11
* @param {number} m12
* @param {number} m13
* @param {number} m21
* @param {number} m22
* @param {number} m23
* @param {number} m31
* @param {number} m32
* @param {number} m33
* @returns {Matrix3D}
*/
  static set(m11: number, m12: number, m13: number, m21: number, m22: number, m23: number, m31: number, m32: number, m33: number): Matrix3D;
/**
* @param {Matrix3D} other
* @returns {Matrix3D}
*/
  add(other: Matrix3D): Matrix3D;
/**
* @param {Matrix3D} other
* @returns {Matrix3D}
*/
  subtract(other: Matrix3D): Matrix3D;
/**
*/
  m11: number;
/**
*/
  m12: number;
/**
*/
  m13: number;
/**
*/
  m21: number;
/**
*/
  m22: number;
/**
*/
  m23: number;
/**
*/
  m31: number;
/**
*/
  m32: number;
/**
*/
  m33: number;
}
/**
*/
export class Mesh {
  free(): void;
/**
* @returns {Mesh}
*/
  static new(): Mesh;
/**
* @param {(Vector3D)[]} vertices
*/
  copy_poligon_vertices(vertices: (Vector3D)[]): void;
/**
* @returns {(Vector3D)[]}
*/
  get_poligon_vertices(): (Vector3D)[];
/**
* @param {Vector3D} vertex
*/
  add_buf_face(vertex: Vector3D): void;
/**
* @param {number} index
*/
  remove_buf_face(index: number): void;
/**
* @param {Vector3D} position
*/
  set_position(position: Vector3D): void;
/**
* @returns {Vector3D}
*/
  get_position(): Vector3D;
/**
* @param {number} height
*/
  set_extrude_height(height: number): void;
/**
* @returns {string}
*/
  get_geometry(): string;
/**
*/
  color: ColorRGBA;
/**
*/
  position: Vector3D;
/**
*/
  position_matrix: Matrix3D;
/**
*/
  rotation: Vector3D;
/**
*/
  rotation_matrix: Matrix3D;
/**
*/
  scale: Vector3D;
/**
*/
  scale_matrix: Matrix3D;
}
/**
*/
export class Polygon {
  free(): void;
/**
*/
  constructor();
/**
* @param {Vector3D} vertex
*/
  add_vertex(vertex: Vector3D): void;
/**
* @param {number} index
*/
  remove_vertex(index: number): void;
/**
* @param {number} index
* @param {Vector3D} vertex
*/
  update_vertex(index: number, vertex: Vector3D): void;
/**
* @param {number} index
* @returns {Vector3D | undefined}
*/
  get_vertex(index: number): Vector3D | undefined;
/**
* @returns {number}
*/
  vertex_count(): number;
/**
* @returns {(Vector3D)[]}
*/
  get_all_vertices(): (Vector3D)[];
/**
*/
  clear_vertices(): void;
/**
* @param {Vector3D} position
*/
  set_position(position: Vector3D): void;
/**
* @returns {Vector3D}
*/
  get_position(): Vector3D;
/**
* @param {boolean} extrude
* @returns {Mesh}
*/
  set_extrude(extrude: boolean): Mesh;
/**
* @returns {Float64Array}
*/
  earcut(): Float64Array;
/**
*/
  extrude: boolean;
/**
*/
  position: Vector3D;
}
/**
*/
export class Triangle {
  free(): void;
/**
*/
  constructor();
/**
* @param {Vector3D} a
* @param {Vector3D} b
* @param {Vector3D} c
*/
  set_vertices(a: Vector3D, b: Vector3D, c: Vector3D): void;
/**
* @returns {(Vector3D)[]}
*/
  get_all_vertices(): (Vector3D)[];
/**
* @param {Vector3D} p
* @returns {boolean}
*/
  is_point_in_triangle(p: Vector3D): boolean;
/**
*/
  a: Vector3D;
/**
*/
  b: Vector3D;
/**
*/
  c: Vector3D;
}
/**
*/
export class Vector3D {
  free(): void;
/**
* @param {number} x
* @param {number} y
* @param {number} z
*/
  constructor(x: number, y: number, z: number);
/**
* @param {number} x
* @param {number} y
* @param {number} z
*/
  update(x: number, y: number, z: number): void;
/**
* @param {Vector3D} other
* @returns {Vector3D}
*/
  add(other: Vector3D): Vector3D;
/**
* @param {Vector3D} other
* @returns {Vector3D}
*/
  subtract(other: Vector3D): Vector3D;
/**
* @param {number} scalar
* @returns {Vector3D}
*/
  add_scalar(scalar: number): Vector3D;
/**
* @param {number} height
* @param {Vector3D} up_vector
* @returns {Vector3D}
*/
  add_extrude_in_up(height: number, up_vector: Vector3D): Vector3D;
/**
* @param {Vector3D} other
* @returns {Vector3D}
*/
  cross(other: Vector3D): Vector3D;
/**
*/
  x: number;
/**
*/
  y: number;
/**
*/
  z: number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_vector3d_free: (a: number, b: number) => void;
  readonly vector3d_create: (a: number, b: number, c: number) => number;
  readonly vector3d_update: (a: number, b: number, c: number, d: number) => void;
  readonly vector3d_add: (a: number, b: number) => number;
  readonly vector3d_subtract: (a: number, b: number) => number;
  readonly vector3d_add_scalar: (a: number, b: number) => number;
  readonly vector3d_add_extrude_in_up: (a: number, b: number, c: number) => number;
  readonly vector3d_cross: (a: number, b: number) => number;
  readonly __wbg_matrix3d_free: (a: number, b: number) => void;
  readonly __wbg_get_matrix3d_m22: (a: number) => number;
  readonly __wbg_set_matrix3d_m22: (a: number, b: number) => void;
  readonly __wbg_get_matrix3d_m23: (a: number) => number;
  readonly __wbg_set_matrix3d_m23: (a: number, b: number) => void;
  readonly __wbg_get_matrix3d_m31: (a: number) => number;
  readonly __wbg_set_matrix3d_m31: (a: number, b: number) => void;
  readonly __wbg_get_matrix3d_m32: (a: number) => number;
  readonly __wbg_set_matrix3d_m32: (a: number, b: number) => void;
  readonly __wbg_get_matrix3d_m33: (a: number) => number;
  readonly __wbg_set_matrix3d_m33: (a: number, b: number) => void;
  readonly matrix3d_set: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => number;
  readonly matrix3d_add: (a: number, b: number) => number;
  readonly matrix3d_subtract: (a: number, b: number) => number;
  readonly __wbg_colorrgba_free: (a: number, b: number) => void;
  readonly __wbg_get_colorrgba_r: (a: number) => number;
  readonly __wbg_set_colorrgba_r: (a: number, b: number) => void;
  readonly __wbg_get_colorrgba_g: (a: number) => number;
  readonly __wbg_set_colorrgba_g: (a: number, b: number) => void;
  readonly __wbg_get_colorrgba_b: (a: number) => number;
  readonly __wbg_set_colorrgba_b: (a: number, b: number) => void;
  readonly __wbg_get_colorrgba_a: (a: number) => number;
  readonly __wbg_set_colorrgba_a: (a: number, b: number) => void;
  readonly __wbg_mesh_free: (a: number, b: number) => void;
  readonly __wbg_get_mesh_position: (a: number) => number;
  readonly __wbg_set_mesh_position: (a: number, b: number) => void;
  readonly __wbg_get_mesh_position_matrix: (a: number) => number;
  readonly __wbg_set_mesh_position_matrix: (a: number, b: number) => void;
  readonly __wbg_get_mesh_rotation: (a: number) => number;
  readonly __wbg_set_mesh_rotation: (a: number, b: number) => void;
  readonly __wbg_get_mesh_rotation_matrix: (a: number) => number;
  readonly __wbg_set_mesh_rotation_matrix: (a: number, b: number) => void;
  readonly __wbg_get_mesh_scale: (a: number) => number;
  readonly __wbg_set_mesh_scale: (a: number, b: number) => void;
  readonly __wbg_get_mesh_scale_matrix: (a: number) => number;
  readonly __wbg_set_mesh_scale_matrix: (a: number, b: number) => void;
  readonly __wbg_get_mesh_color: (a: number) => number;
  readonly __wbg_set_mesh_color: (a: number, b: number) => void;
  readonly mesh_new: () => number;
  readonly mesh_copy_poligon_vertices: (a: number, b: number, c: number) => void;
  readonly mesh_get_poligon_vertices: (a: number, b: number) => void;
  readonly mesh_add_buf_face: (a: number, b: number) => void;
  readonly mesh_remove_buf_face: (a: number, b: number) => void;
  readonly mesh_set_position: (a: number, b: number) => void;
  readonly mesh_get_position: (a: number) => number;
  readonly mesh_set_extrude_height: (a: number, b: number) => void;
  readonly mesh_get_geometry: (a: number, b: number) => void;
  readonly __wbg_polygon_free: (a: number, b: number) => void;
  readonly __wbg_get_polygon_position: (a: number) => number;
  readonly __wbg_set_polygon_position: (a: number, b: number) => void;
  readonly __wbg_get_polygon_extrude: (a: number) => number;
  readonly __wbg_set_polygon_extrude: (a: number, b: number) => void;
  readonly polygon_new: () => number;
  readonly polygon_add_vertex: (a: number, b: number) => void;
  readonly polygon_remove_vertex: (a: number, b: number) => void;
  readonly polygon_update_vertex: (a: number, b: number, c: number) => void;
  readonly polygon_get_vertex: (a: number, b: number) => number;
  readonly polygon_vertex_count: (a: number) => number;
  readonly polygon_get_all_vertices: (a: number, b: number) => void;
  readonly polygon_clear_vertices: (a: number) => void;
  readonly polygon_set_position: (a: number, b: number) => void;
  readonly polygon_get_position: (a: number) => number;
  readonly polygon_set_extrude: (a: number, b: number) => number;
  readonly polygon_earcut: (a: number, b: number) => void;
  readonly __wbg_get_triangle_b: (a: number) => number;
  readonly __wbg_set_triangle_b: (a: number, b: number) => void;
  readonly __wbg_get_triangle_c: (a: number) => number;
  readonly __wbg_set_triangle_c: (a: number, b: number) => void;
  readonly triangle_new: () => number;
  readonly triangle_set_vertices: (a: number, b: number, c: number, d: number) => void;
  readonly triangle_get_all_vertices: (a: number, b: number) => void;
  readonly triangle_is_point_in_triangle: (a: number, b: number) => number;
  readonly triangulate_mesh: (a: number, b: number) => void;
  readonly triangulate: (a: number, b: number, c: number) => void;
  readonly get_tricut_vertices: (a: number) => void;
  readonly __wbg_set_triangle_a: (a: number, b: number) => void;
  readonly __wbg_get_vector3d_x: (a: number) => number;
  readonly __wbg_get_vector3d_y: (a: number) => number;
  readonly __wbg_get_vector3d_z: (a: number) => number;
  readonly __wbg_get_matrix3d_m11: (a: number) => number;
  readonly __wbg_get_matrix3d_m12: (a: number) => number;
  readonly __wbg_get_matrix3d_m13: (a: number) => number;
  readonly __wbg_get_matrix3d_m21: (a: number) => number;
  readonly __wbg_get_triangle_a: (a: number) => number;
  readonly __wbg_set_vector3d_x: (a: number, b: number) => void;
  readonly __wbg_set_vector3d_y: (a: number, b: number) => void;
  readonly __wbg_set_vector3d_z: (a: number, b: number) => void;
  readonly __wbg_set_matrix3d_m11: (a: number, b: number) => void;
  readonly __wbg_set_matrix3d_m12: (a: number, b: number) => void;
  readonly __wbg_set_matrix3d_m13: (a: number, b: number) => void;
  readonly __wbg_set_matrix3d_m21: (a: number, b: number) => void;
  readonly __wbg_triangle_free: (a: number, b: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
