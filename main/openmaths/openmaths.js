let wasm;

const cachedTextDecoder = (typeof TextDecoder !== 'undefined' ? new TextDecoder('utf-8', { ignoreBOM: true, fatal: true }) : { decode: () => { throw Error('TextDecoder not available') } } );

if (typeof TextDecoder !== 'undefined') { cachedTextDecoder.decode(); };

let cachedUint8ArrayMemory0 = null;

function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const heap = new Array(128).fill(undefined);

heap.push(undefined, null, true, false);

let heap_next = heap.length;

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

function getObject(idx) { return heap[idx]; }

function dropObject(idx) {
    if (idx < 132) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

function takeObject(idx) {
    const ret = getObject(idx);
    dropObject(idx);
    return ret;
}

function _assertClass(instance, klass) {
    if (!(instance instanceof klass)) {
        throw new Error(`expected instance of ${klass.name}`);
    }
    return instance.ptr;
}

let cachedDataViewMemory0 = null;

function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let WASM_VECTOR_LEN = 0;

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    const mem = getDataViewMemory0();
    for (let i = 0; i < array.length; i++) {
        mem.setUint32(ptr + 4 * i, addHeapObject(array[i]), true);
    }
    WASM_VECTOR_LEN = array.length;
    return ptr;
}

function getArrayJsValueFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    const mem = getDataViewMemory0();
    const result = [];
    for (let i = ptr; i < ptr + 4 * len; i += 4) {
        result.push(takeObject(mem.getUint32(i, true)));
    }
    return result;
}

let cachedFloat64ArrayMemory0 = null;

function getFloat64ArrayMemory0() {
    if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) {
        cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
    }
    return cachedFloat64ArrayMemory0;
}

function getArrayF64FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat64ArrayMemory0().subarray(ptr / 8, ptr / 8 + len);
}
/**
* @param {Mesh} mesh
* @returns {Float64Array}
*/
export function triangulate_mesh(mesh) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        _assertClass(mesh, Mesh);
        var ptr0 = mesh.__destroy_into_raw();
        wasm.triangulate_mesh(retptr, ptr0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var v2 = getArrayF64FromWasm0(r0, r1).slice();
        wasm.__wbindgen_free(r0, r1 * 8, 8);
        return v2;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
* @param {(Vector3D)[]} vertices
* @returns {(Vector3D)[]}
*/
export function triangulate(vertices) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArrayJsValueToWasm0(vertices, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.triangulate(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var v2 = getArrayJsValueFromWasm0(r0, r1).slice();
        wasm.__wbindgen_free(r0, r1 * 4, 4);
        return v2;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
* @returns {string}
*/
export function get_tricut_vertices() {
    let deferred1_0;
    let deferred1_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.get_tricut_vertices(retptr);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred1_0 = r0;
        deferred1_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

const ColorRGBAFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_colorrgba_free(ptr >>> 0, 1));
/**
*/
export class ColorRGBA {

    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(ColorRGBA.prototype);
        obj.__wbg_ptr = ptr;
        ColorRGBAFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }

    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ColorRGBAFinalization.unregister(this);
        return ptr;
    }

    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_colorrgba_free(ptr, 0);
    }
    /**
    * @returns {number}
    */
    get r() {
        const ret = wasm.__wbg_get_colorrgba_r(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set r(arg0) {
        wasm.__wbg_set_colorrgba_r(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get g() {
        const ret = wasm.__wbg_get_colorrgba_g(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set g(arg0) {
        wasm.__wbg_set_colorrgba_g(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get b() {
        const ret = wasm.__wbg_get_colorrgba_b(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set b(arg0) {
        wasm.__wbg_set_colorrgba_b(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get a() {
        const ret = wasm.__wbg_get_colorrgba_a(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set a(arg0) {
        wasm.__wbg_set_colorrgba_a(this.__wbg_ptr, arg0);
    }
}

const Matrix3DFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_matrix3d_free(ptr >>> 0, 1));
/**
*/
export class Matrix3D {

    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(Matrix3D.prototype);
        obj.__wbg_ptr = ptr;
        Matrix3DFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }

    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        Matrix3DFinalization.unregister(this);
        return ptr;
    }

    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_matrix3d_free(ptr, 0);
    }
    /**
    * @returns {number}
    */
    get m11() {
        const ret = wasm.__wbg_get_colorrgba_r(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set m11(arg0) {
        wasm.__wbg_set_colorrgba_r(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get m12() {
        const ret = wasm.__wbg_get_colorrgba_g(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set m12(arg0) {
        wasm.__wbg_set_colorrgba_g(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get m13() {
        const ret = wasm.__wbg_get_colorrgba_b(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set m13(arg0) {
        wasm.__wbg_set_colorrgba_b(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get m21() {
        const ret = wasm.__wbg_get_colorrgba_a(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set m21(arg0) {
        wasm.__wbg_set_colorrgba_a(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get m22() {
        const ret = wasm.__wbg_get_matrix3d_m22(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set m22(arg0) {
        wasm.__wbg_set_matrix3d_m22(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get m23() {
        const ret = wasm.__wbg_get_matrix3d_m23(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set m23(arg0) {
        wasm.__wbg_set_matrix3d_m23(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get m31() {
        const ret = wasm.__wbg_get_matrix3d_m31(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set m31(arg0) {
        wasm.__wbg_set_matrix3d_m31(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get m32() {
        const ret = wasm.__wbg_get_matrix3d_m32(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set m32(arg0) {
        wasm.__wbg_set_matrix3d_m32(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get m33() {
        const ret = wasm.__wbg_get_matrix3d_m33(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set m33(arg0) {
        wasm.__wbg_set_matrix3d_m33(this.__wbg_ptr, arg0);
    }
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
    static set(m11, m12, m13, m21, m22, m23, m31, m32, m33) {
        const ret = wasm.matrix3d_set(m11, m12, m13, m21, m22, m23, m31, m32, m33);
        return Matrix3D.__wrap(ret);
    }
    /**
    * @param {Matrix3D} other
    * @returns {Matrix3D}
    */
    add(other) {
        _assertClass(other, Matrix3D);
        const ret = wasm.matrix3d_add(this.__wbg_ptr, other.__wbg_ptr);
        return Matrix3D.__wrap(ret);
    }
    /**
    * @param {Matrix3D} other
    * @returns {Matrix3D}
    */
    subtract(other) {
        _assertClass(other, Matrix3D);
        const ret = wasm.matrix3d_subtract(this.__wbg_ptr, other.__wbg_ptr);
        return Matrix3D.__wrap(ret);
    }
}

const MeshFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_mesh_free(ptr >>> 0, 1));
/**
*/
export class Mesh {

    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(Mesh.prototype);
        obj.__wbg_ptr = ptr;
        MeshFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }

    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        MeshFinalization.unregister(this);
        return ptr;
    }

    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_mesh_free(ptr, 0);
    }
    /**
    * @returns {Vector3D}
    */
    get position() {
        const ret = wasm.__wbg_get_mesh_position(this.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {Vector3D} arg0
    */
    set position(arg0) {
        _assertClass(arg0, Vector3D);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_mesh_position(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {Matrix3D}
    */
    get position_matrix() {
        const ret = wasm.__wbg_get_mesh_position_matrix(this.__wbg_ptr);
        return Matrix3D.__wrap(ret);
    }
    /**
    * @param {Matrix3D} arg0
    */
    set position_matrix(arg0) {
        _assertClass(arg0, Matrix3D);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_mesh_position_matrix(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {Vector3D}
    */
    get rotation() {
        const ret = wasm.__wbg_get_mesh_rotation(this.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {Vector3D} arg0
    */
    set rotation(arg0) {
        _assertClass(arg0, Vector3D);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_mesh_rotation(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {Matrix3D}
    */
    get rotation_matrix() {
        const ret = wasm.__wbg_get_mesh_rotation_matrix(this.__wbg_ptr);
        return Matrix3D.__wrap(ret);
    }
    /**
    * @param {Matrix3D} arg0
    */
    set rotation_matrix(arg0) {
        _assertClass(arg0, Matrix3D);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_mesh_rotation_matrix(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {Vector3D}
    */
    get scale() {
        const ret = wasm.__wbg_get_mesh_scale(this.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {Vector3D} arg0
    */
    set scale(arg0) {
        _assertClass(arg0, Vector3D);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_mesh_scale(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {Matrix3D}
    */
    get scale_matrix() {
        const ret = wasm.__wbg_get_mesh_scale_matrix(this.__wbg_ptr);
        return Matrix3D.__wrap(ret);
    }
    /**
    * @param {Matrix3D} arg0
    */
    set scale_matrix(arg0) {
        _assertClass(arg0, Matrix3D);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_mesh_scale_matrix(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {ColorRGBA}
    */
    get color() {
        const ret = wasm.__wbg_get_mesh_color(this.__wbg_ptr);
        return ColorRGBA.__wrap(ret);
    }
    /**
    * @param {ColorRGBA} arg0
    */
    set color(arg0) {
        _assertClass(arg0, ColorRGBA);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_mesh_color(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {Mesh}
    */
    static new() {
        const ret = wasm.mesh_new();
        return Mesh.__wrap(ret);
    }
    /**
    * @param {(Vector3D)[]} vertices
    */
    copy_poligon_vertices(vertices) {
        const ptr0 = passArrayJsValueToWasm0(vertices, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.mesh_copy_poligon_vertices(this.__wbg_ptr, ptr0, len0);
    }
    /**
    * @returns {(Vector3D)[]}
    */
    get_poligon_vertices() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.mesh_get_poligon_vertices(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayJsValueFromWasm0(r0, r1).slice();
            wasm.__wbindgen_free(r0, r1 * 4, 4);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
    * @param {Vector3D} vertex
    */
    add_buf_face(vertex) {
        _assertClass(vertex, Vector3D);
        var ptr0 = vertex.__destroy_into_raw();
        wasm.mesh_add_buf_face(this.__wbg_ptr, ptr0);
    }
    /**
    * @param {number} index
    */
    remove_buf_face(index) {
        wasm.mesh_remove_buf_face(this.__wbg_ptr, index);
    }
    /**
    * @param {Vector3D} position
    */
    set_position(position) {
        _assertClass(position, Vector3D);
        var ptr0 = position.__destroy_into_raw();
        wasm.mesh_set_position(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {Vector3D}
    */
    get_position() {
        const ret = wasm.mesh_get_position(this.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {number} height
    */
    set_extrude_height(height) {
        wasm.mesh_set_extrude_height(this.__wbg_ptr, height);
    }
    /**
    * @returns {string}
    */
    get_geometry() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.mesh_get_geometry(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}

const PolygonFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_polygon_free(ptr >>> 0, 1));
/**
*/
export class Polygon {

    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        PolygonFinalization.unregister(this);
        return ptr;
    }

    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_polygon_free(ptr, 0);
    }
    /**
    * @returns {Vector3D}
    */
    get position() {
        const ret = wasm.__wbg_get_polygon_position(this.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {Vector3D} arg0
    */
    set position(arg0) {
        _assertClass(arg0, Vector3D);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_polygon_position(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {boolean}
    */
    get extrude() {
        const ret = wasm.__wbg_get_polygon_extrude(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
    * @param {boolean} arg0
    */
    set extrude(arg0) {
        wasm.__wbg_set_polygon_extrude(this.__wbg_ptr, arg0);
    }
    /**
    */
    constructor() {
        const ret = wasm.polygon_new();
        this.__wbg_ptr = ret >>> 0;
        PolygonFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
    * @param {Vector3D} vertex
    */
    add_vertex(vertex) {
        _assertClass(vertex, Vector3D);
        var ptr0 = vertex.__destroy_into_raw();
        wasm.polygon_add_vertex(this.__wbg_ptr, ptr0);
    }
    /**
    * @param {number} index
    */
    remove_vertex(index) {
        wasm.polygon_remove_vertex(this.__wbg_ptr, index);
    }
    /**
    * @param {number} index
    * @param {Vector3D} vertex
    */
    update_vertex(index, vertex) {
        _assertClass(vertex, Vector3D);
        var ptr0 = vertex.__destroy_into_raw();
        wasm.polygon_update_vertex(this.__wbg_ptr, index, ptr0);
    }
    /**
    * @param {number} index
    * @returns {Vector3D | undefined}
    */
    get_vertex(index) {
        const ret = wasm.polygon_get_vertex(this.__wbg_ptr, index);
        return ret === 0 ? undefined : Vector3D.__wrap(ret);
    }
    /**
    * @returns {number}
    */
    vertex_count() {
        const ret = wasm.polygon_vertex_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
    * @returns {(Vector3D)[]}
    */
    get_all_vertices() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.polygon_get_all_vertices(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayJsValueFromWasm0(r0, r1).slice();
            wasm.__wbindgen_free(r0, r1 * 4, 4);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
    */
    clear_vertices() {
        wasm.polygon_clear_vertices(this.__wbg_ptr);
    }
    /**
    * @param {Vector3D} position
    */
    set_position(position) {
        _assertClass(position, Vector3D);
        var ptr0 = position.__destroy_into_raw();
        wasm.polygon_set_position(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {Vector3D}
    */
    get_position() {
        const ret = wasm.polygon_get_position(this.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {boolean} extrude
    * @returns {Mesh}
    */
    set_extrude(extrude) {
        const ret = wasm.polygon_set_extrude(this.__wbg_ptr, extrude);
        return Mesh.__wrap(ret);
    }
    /**
    * @returns {Float64Array}
    */
    earcut() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.polygon_earcut(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayF64FromWasm0(r0, r1).slice();
            wasm.__wbindgen_free(r0, r1 * 8, 8);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}

const TriangleFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_triangle_free(ptr >>> 0, 1));
/**
*/
export class Triangle {

    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        TriangleFinalization.unregister(this);
        return ptr;
    }

    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_triangle_free(ptr, 0);
    }
    /**
    * @returns {Vector3D}
    */
    get a() {
        const ret = wasm.__wbg_get_polygon_position(this.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {Vector3D} arg0
    */
    set a(arg0) {
        _assertClass(arg0, Vector3D);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_polygon_position(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {Vector3D}
    */
    get b() {
        const ret = wasm.__wbg_get_triangle_b(this.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {Vector3D} arg0
    */
    set b(arg0) {
        _assertClass(arg0, Vector3D);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_triangle_b(this.__wbg_ptr, ptr0);
    }
    /**
    * @returns {Vector3D}
    */
    get c() {
        const ret = wasm.__wbg_get_triangle_c(this.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {Vector3D} arg0
    */
    set c(arg0) {
        _assertClass(arg0, Vector3D);
        var ptr0 = arg0.__destroy_into_raw();
        wasm.__wbg_set_triangle_c(this.__wbg_ptr, ptr0);
    }
    /**
    */
    constructor() {
        const ret = wasm.triangle_new();
        this.__wbg_ptr = ret >>> 0;
        TriangleFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
    * @param {Vector3D} a
    * @param {Vector3D} b
    * @param {Vector3D} c
    */
    set_vertices(a, b, c) {
        _assertClass(a, Vector3D);
        var ptr0 = a.__destroy_into_raw();
        _assertClass(b, Vector3D);
        var ptr1 = b.__destroy_into_raw();
        _assertClass(c, Vector3D);
        var ptr2 = c.__destroy_into_raw();
        wasm.triangle_set_vertices(this.__wbg_ptr, ptr0, ptr1, ptr2);
    }
    /**
    * @returns {(Vector3D)[]}
    */
    get_all_vertices() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.triangle_get_all_vertices(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayJsValueFromWasm0(r0, r1).slice();
            wasm.__wbindgen_free(r0, r1 * 4, 4);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
    * @param {Vector3D} p
    * @returns {boolean}
    */
    is_point_in_triangle(p) {
        _assertClass(p, Vector3D);
        var ptr0 = p.__destroy_into_raw();
        const ret = wasm.triangle_is_point_in_triangle(this.__wbg_ptr, ptr0);
        return ret !== 0;
    }
}

const Vector3DFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_vector3d_free(ptr >>> 0, 1));
/**
*/
export class Vector3D {

    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(Vector3D.prototype);
        obj.__wbg_ptr = ptr;
        Vector3DFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }

    static __unwrap(jsValue) {
        if (!(jsValue instanceof Vector3D)) {
            return 0;
        }
        return jsValue.__destroy_into_raw();
    }

    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        Vector3DFinalization.unregister(this);
        return ptr;
    }

    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_vector3d_free(ptr, 0);
    }
    /**
    * @returns {number}
    */
    get x() {
        const ret = wasm.__wbg_get_colorrgba_r(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set x(arg0) {
        wasm.__wbg_set_colorrgba_r(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get y() {
        const ret = wasm.__wbg_get_colorrgba_g(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set y(arg0) {
        wasm.__wbg_set_colorrgba_g(this.__wbg_ptr, arg0);
    }
    /**
    * @returns {number}
    */
    get z() {
        const ret = wasm.__wbg_get_colorrgba_b(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} arg0
    */
    set z(arg0) {
        wasm.__wbg_set_colorrgba_b(this.__wbg_ptr, arg0);
    }
    /**
    * @param {number} x
    * @param {number} y
    * @param {number} z
    */
    constructor(x, y, z) {
        const ret = wasm.vector3d_create(x, y, z);
        this.__wbg_ptr = ret >>> 0;
        Vector3DFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
    * @param {number} x
    * @param {number} y
    * @param {number} z
    */
    update(x, y, z) {
        wasm.vector3d_update(this.__wbg_ptr, x, y, z);
    }
    /**
    * @param {Vector3D} other
    * @returns {Vector3D}
    */
    add(other) {
        _assertClass(other, Vector3D);
        const ret = wasm.vector3d_add(this.__wbg_ptr, other.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {Vector3D} other
    * @returns {Vector3D}
    */
    subtract(other) {
        _assertClass(other, Vector3D);
        const ret = wasm.vector3d_subtract(this.__wbg_ptr, other.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {number} scalar
    * @returns {Vector3D}
    */
    add_scalar(scalar) {
        const ret = wasm.vector3d_add_scalar(this.__wbg_ptr, scalar);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {number} height
    * @param {Vector3D} up_vector
    * @returns {Vector3D}
    */
    add_extrude_in_up(height, up_vector) {
        _assertClass(up_vector, Vector3D);
        var ptr0 = up_vector.__destroy_into_raw();
        const ret = wasm.vector3d_add_extrude_in_up(this.__wbg_ptr, height, ptr0);
        return Vector3D.__wrap(ret);
    }
    /**
    * @param {Vector3D} other
    * @returns {Vector3D}
    */
    cross(other) {
        _assertClass(other, Vector3D);
        const ret = wasm.vector3d_cross(this.__wbg_ptr, other.__wbg_ptr);
        return Vector3D.__wrap(ret);
    }
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);

            } catch (e) {
                if (module.headers.get('Content-Type') != 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else {
                    throw e;
                }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);

    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };

        } else {
            return instance;
        }
    }
}

function __wbg_get_imports() {
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbg_vector3d_unwrap = function(arg0) {
        const ret = Vector3D.__unwrap(takeObject(arg0));
        return ret;
    };
    imports.wbg.__wbg_vector3d_new = function(arg0) {
        const ret = Vector3D.__wrap(arg0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_throw = function(arg0, arg1) {
        throw new Error(getStringFromWasm0(arg0, arg1));
    };

    return imports;
}

function __wbg_init_memory(imports, memory) {

}

function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    __wbg_init.__wbindgen_wasm_module = module;
    cachedDataViewMemory0 = null;
    cachedFloat64ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;



    return wasm;
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (typeof module !== 'undefined' && Object.getPrototypeOf(module) === Object.prototype)
    ({module} = module)
    else
    console.warn('using deprecated parameters for `initSync()`; pass a single object instead')

    const imports = __wbg_get_imports();

    __wbg_init_memory(imports);

    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }

    const instance = new WebAssembly.Instance(module, imports);

    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (typeof module_or_path !== 'undefined' && Object.getPrototypeOf(module_or_path) === Object.prototype)
    ({module_or_path} = module_or_path)
    else
    console.warn('using deprecated parameters for the initialization function; pass a single object instead')

    if (typeof module_or_path === 'undefined') {
        module_or_path = new URL('openmaths_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    __wbg_init_memory(imports);

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync };
export default __wbg_init;
