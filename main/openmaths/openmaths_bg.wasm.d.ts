/* tslint:disable */
/* eslint-disable */
export const memory: WebAssembly.Memory;
export function __wbg_vector3d_free(a: number, b: number): void;
export function vector3d_create(a: number, b: number, c: number): number;
export function vector3d_update(a: number, b: number, c: number, d: number): void;
export function vector3d_add(a: number, b: number): number;
export function vector3d_subtract(a: number, b: number): number;
export function vector3d_add_scalar(a: number, b: number): number;
export function vector3d_add_extrude_in_up(a: number, b: number, c: number): number;
export function vector3d_cross(a: number, b: number): number;
export function __wbg_matrix3d_free(a: number, b: number): void;
export function __wbg_get_matrix3d_m22(a: number): number;
export function __wbg_set_matrix3d_m22(a: number, b: number): void;
export function __wbg_get_matrix3d_m23(a: number): number;
export function __wbg_set_matrix3d_m23(a: number, b: number): void;
export function __wbg_get_matrix3d_m31(a: number): number;
export function __wbg_set_matrix3d_m31(a: number, b: number): void;
export function __wbg_get_matrix3d_m32(a: number): number;
export function __wbg_set_matrix3d_m32(a: number, b: number): void;
export function __wbg_get_matrix3d_m33(a: number): number;
export function __wbg_set_matrix3d_m33(a: number, b: number): void;
export function matrix3d_set(a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number): number;
export function matrix3d_add(a: number, b: number): number;
export function matrix3d_subtract(a: number, b: number): number;
export function __wbg_colorrgba_free(a: number, b: number): void;
export function __wbg_get_colorrgba_r(a: number): number;
export function __wbg_set_colorrgba_r(a: number, b: number): void;
export function __wbg_get_colorrgba_g(a: number): number;
export function __wbg_set_colorrgba_g(a: number, b: number): void;
export function __wbg_get_colorrgba_b(a: number): number;
export function __wbg_set_colorrgba_b(a: number, b: number): void;
export function __wbg_get_colorrgba_a(a: number): number;
export function __wbg_set_colorrgba_a(a: number, b: number): void;
export function __wbg_mesh_free(a: number, b: number): void;
export function __wbg_get_mesh_position(a: number): number;
export function __wbg_set_mesh_position(a: number, b: number): void;
export function __wbg_get_mesh_position_matrix(a: number): number;
export function __wbg_set_mesh_position_matrix(a: number, b: number): void;
export function __wbg_get_mesh_rotation(a: number): number;
export function __wbg_set_mesh_rotation(a: number, b: number): void;
export function __wbg_get_mesh_rotation_matrix(a: number): number;
export function __wbg_set_mesh_rotation_matrix(a: number, b: number): void;
export function __wbg_get_mesh_scale(a: number): number;
export function __wbg_set_mesh_scale(a: number, b: number): void;
export function __wbg_get_mesh_scale_matrix(a: number): number;
export function __wbg_set_mesh_scale_matrix(a: number, b: number): void;
export function __wbg_get_mesh_color(a: number): number;
export function __wbg_set_mesh_color(a: number, b: number): void;
export function mesh_new(): number;
export function mesh_copy_poligon_vertices(a: number, b: number, c: number): void;
export function mesh_get_poligon_vertices(a: number, b: number): void;
export function mesh_add_buf_face(a: number, b: number): void;
export function mesh_remove_buf_face(a: number, b: number): void;
export function mesh_set_position(a: number, b: number): void;
export function mesh_get_position(a: number): number;
export function mesh_set_extrude_height(a: number, b: number): void;
export function mesh_get_geometry(a: number, b: number): void;
export function __wbg_polygon_free(a: number, b: number): void;
export function __wbg_get_polygon_position(a: number): number;
export function __wbg_set_polygon_position(a: number, b: number): void;
export function __wbg_get_polygon_extrude(a: number): number;
export function __wbg_set_polygon_extrude(a: number, b: number): void;
export function polygon_new(): number;
export function polygon_add_vertex(a: number, b: number): void;
export function polygon_remove_vertex(a: number, b: number): void;
export function polygon_update_vertex(a: number, b: number, c: number): void;
export function polygon_get_vertex(a: number, b: number): number;
export function polygon_vertex_count(a: number): number;
export function polygon_get_all_vertices(a: number, b: number): void;
export function polygon_clear_vertices(a: number): void;
export function polygon_set_position(a: number, b: number): void;
export function polygon_get_position(a: number): number;
export function polygon_set_extrude(a: number, b: number): number;
export function polygon_earcut(a: number, b: number): void;
export function __wbg_get_triangle_b(a: number): number;
export function __wbg_set_triangle_b(a: number, b: number): void;
export function __wbg_get_triangle_c(a: number): number;
export function __wbg_set_triangle_c(a: number, b: number): void;
export function triangle_new(): number;
export function triangle_set_vertices(a: number, b: number, c: number, d: number): void;
export function triangle_get_all_vertices(a: number, b: number): void;
export function triangle_is_point_in_triangle(a: number, b: number): number;
export function triangulate_mesh(a: number, b: number): void;
export function triangulate(a: number, b: number, c: number): void;
export function get_tricut_vertices(a: number): void;
export function __wbg_set_triangle_a(a: number, b: number): void;
export function __wbg_get_vector3d_x(a: number): number;
export function __wbg_get_vector3d_y(a: number): number;
export function __wbg_get_vector3d_z(a: number): number;
export function __wbg_get_matrix3d_m11(a: number): number;
export function __wbg_get_matrix3d_m12(a: number): number;
export function __wbg_get_matrix3d_m13(a: number): number;
export function __wbg_get_matrix3d_m21(a: number): number;
export function __wbg_get_triangle_a(a: number): number;
export function __wbg_set_vector3d_x(a: number, b: number): void;
export function __wbg_set_vector3d_y(a: number, b: number): void;
export function __wbg_set_vector3d_z(a: number, b: number): void;
export function __wbg_set_matrix3d_m11(a: number, b: number): void;
export function __wbg_set_matrix3d_m12(a: number, b: number): void;
export function __wbg_set_matrix3d_m13(a: number, b: number): void;
export function __wbg_set_matrix3d_m21(a: number, b: number): void;
export function __wbg_triangle_free(a: number, b: number): void;
export function __wbindgen_malloc(a: number, b: number): number;
export function __wbindgen_add_to_stack_pointer(a: number): number;
export function __wbindgen_free(a: number, b: number, c: number): void;