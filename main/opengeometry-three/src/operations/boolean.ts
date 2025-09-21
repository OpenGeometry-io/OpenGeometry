import { OGBooleanEngine, OGBrep } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";

// Define compatible object interface
export interface BooleanCompatible {
    getOGBrep(): OGBrep;
    generate_geometry?(): void;
}

export interface BooleanResult {
    geometry: THREE.BufferGeometry;
    material: THREE.Material;
    mesh: THREE.Mesh;
    ogBrep: OGBrep;
}

export interface GeometryData {
    vertices: number[];
    indices: number[];
    triangleCount: number;
    vertexCount: number;
    geometryType: string;
}

export interface EnhancedMesh extends THREE.Mesh {
    ogBrep: OGBrep;
    geometryType: string;
    triangleCount: number;
    booleanOperation: string;
}

export interface MeshPrimitive {
    getOGBrep(): OGBrep;
    getBooleanResult(): BooleanResult;
    refresh(): void;
}

export class BooleanOperations {
    private engine: OGBooleanEngine;

    constructor(tolerance: number = 1e-6) {
        this.engine = new OGBooleanEngine(tolerance);
    }

    /**
     * Performs a union boolean operation on two objects
     */
    union(a: BooleanCompatible, b: BooleanCompatible): BooleanResult {
        this.validateInputs(a, b, "union");
        
        try {
            const resultBrep = this.engine.union(a.getOGBrep(), b.getOGBrep());
            return this.createBooleanResult(resultBrep, "union");
        } catch (error) {
            throw new Error(`Boolean union failed: ${error}`);
        }
    }

    /**
     * Performs an intersection boolean operation on two objects
     */
    intersection(a: BooleanCompatible, b: BooleanCompatible): BooleanResult {
        this.validateInputs(a, b, "intersection");
        
        try {
            const resultBrep = this.engine.intersection(a.getOGBrep(), b.getOGBrep());
            return this.createBooleanResult(resultBrep, "intersection");
        } catch (error) {
            throw new Error(`Boolean intersection failed: ${error}`);
        }
    }

    /**
     * Performs a difference boolean operation (A - B)
     */
    difference(a: BooleanCompatible, b: BooleanCompatible): BooleanResult {
        this.validateInputs(a, b, "difference");
        
        try {
            const resultBrep = this.engine.difference(a.getOGBrep(), b.getOGBrep());
            return this.createBooleanResult(resultBrep, "difference");
        } catch (error) {
            throw new Error(`Boolean difference failed: ${error}`);
        }
    }

    /**
     * Performs a symmetric difference boolean operation (A XOR B)
     */
    symmetricDifference(a: BooleanCompatible, b: BooleanCompatible): BooleanResult {
        this.validateInputs(a, b, "symmetric_difference");
        
        try {
            const resultBrep = this.engine.symmetric_difference(a.getOGBrep(), b.getOGBrep());
            return this.createBooleanResult(resultBrep, "symmetric_difference");
        } catch (error) {
            throw new Error(`Boolean symmetric difference failed: ${error}`);
        }
    }

    /**
     * Gets the tolerance used by the boolean engine
     */
    get tolerance(): number {
        return this.engine.tolerance;
    }

    private validateInputs(a: BooleanCompatible, b: BooleanCompatible, operation: string): void {
        if (!a) {
            throw new Error(`First object is null or undefined for ${operation} operation`);
        }
        
        if (!b) {
            throw new Error(`Second object is null or undefined for ${operation} operation`);
        }

        if (!a.getOGBrep || typeof a.getOGBrep !== 'function') {
            throw new Error(`First object must have getOGBrep() method for ${operation} operation`);
        }

        if (!b.getOGBrep || typeof b.getOGBrep !== 'function') {
            throw new Error(`Second object must have getOGBrep() method for ${operation} operation`);
        }

        // Validate that objects are triangulated
        const brepA = a.getOGBrep();
        console.log(`Brep A: triangles=${brepA.triangle_count()}, type=${brepA.geometry_type}`); // Debug log
        const brepB = b.getOGBrep();

        if (!brepA.is_triangulated()) {
            throw new Error(`First object is not triangulated. Call generate_geometry() first.`);
        }

        if (!brepB.is_triangulated()) {
            throw new Error(`Second object is not triangulated. Call generate_geometry() first.`);
        }

        // Validate Brep data
        try {
            brepA.validate();
        } catch (error) {
            throw new Error(`First object has invalid geometry: ${error}`);
        }

        try {
            brepB.validate();
        } catch (error) {
            throw new Error(`Second object has invalid geometry: ${error}`);
        }
    }

    private createBooleanResult(ogBrep: OGBrep, operation: string): BooleanResult {
        // Get serialized geometry data from the Brep
        const geometryDataStr = ogBrep.get_geometry_serialized();
        const geometryData: GeometryData = JSON.parse(geometryDataStr);

        // Create Three.js BufferGeometry
        const geometry = new THREE.BufferGeometry();
        
        // Set vertex positions
        const vertices = new Float32Array(geometryData.vertices);
        geometry.setAttribute('position', new THREE.BufferAttribute(vertices, 3));
        
        // Set indices
        const indices = new Uint32Array(geometryData.indices);
        geometry.setIndex(new THREE.BufferAttribute(indices, 1));
        
        // Compute normals for proper shading
        geometry.computeVertexNormals();
        
        // Compute bounding box and sphere
        geometry.computeBoundingBox();
        geometry.computeBoundingSphere();

        // Create material based on operation type
        const material = this.createMaterialForOperation(operation);
        
        // Create mesh
        const mesh = new THREE.Mesh(geometry, material) as EnhancedMesh;
        mesh.name = `boolean_${operation}_result`;
        
        // Add custom properties
        mesh.ogBrep = ogBrep;
        mesh.geometryType = geometryData.geometryType;
        mesh.triangleCount = geometryData.triangleCount;
        mesh.booleanOperation = operation;

        return {
            geometry,
            material,
            mesh,
            ogBrep
        };
    }

    private createMaterialForOperation(operation: string): THREE.Material {
        const colors: Record<string, number> = {
            union: 0x00ff00,           // Green
            intersection: 0x0080ff,    // Blue
            difference: 0xff8000,      // Orange
            symmetric_difference: 0xff0080, // Pink
        };

        const color = colors[operation] || 0x888888;

        return new THREE.MeshPhongMaterial({
            color: color,
            side: THREE.DoubleSide,
            transparent: false,
            opacity: 1.0,
            shininess: 30,
            specular: 0x111111,
        });
    }

    /**
     * Creates a mesh primitive from boolean result for integration with OpenGeometry system
     */
    static createMeshPrimitive(result: BooleanResult): MeshPrimitive & THREE.Mesh {
        // This would integrate with your existing primitive system
        // For now, return the mesh with OpenGeometry-compatible methods
        const meshPrimitive = result.mesh as MeshPrimitive & THREE.Mesh;
        
        // Add OpenGeometry-compatible methods
        meshPrimitive.getOGBrep = () => result.ogBrep;
        meshPrimitive.getBooleanResult = () => result;
        meshPrimitive.refresh = () => {
            // Refresh geometry if needed
            result.geometry.computeVertexNormals();
            result.geometry.computeBoundingBox();
            result.geometry.computeBoundingSphere();
        };

        return meshPrimitive;
    }

    /**
     * Utility method to check if an object supports boolean operations
     */
    static isCompatible(obj: unknown): obj is BooleanCompatible {
        return obj !== null && 
               obj !== undefined &&
               typeof obj === 'object' &&
               'getOGBrep' in obj &&
               typeof (obj as BooleanCompatible).getOGBrep === 'function' && 
               (obj as BooleanCompatible).getOGBrep() &&
               (obj as BooleanCompatible).getOGBrep().is_triangulated();
    }

    /**
     * Gets information about an object's boolean compatibility
     */
    static getCompatibilityInfo(obj: unknown): {
        compatible: boolean;
        reason?: string;
        triangleCount?: number;
        geometryType?: string;
    } {
        if (!obj || typeof obj !== 'object') {
            return { compatible: false, reason: "Object is null or undefined" };
        }

        if (!('getOGBrep' in obj) || typeof (obj as BooleanCompatible).getOGBrep !== 'function') {
            return { compatible: false, reason: "Object does not have getOGBrep() method" };
        }

        const compatibleObj = obj as BooleanCompatible;
        const brep = compatibleObj.getOGBrep();
        if (!brep) {
            return { compatible: false, reason: "Object's getOGBrep() returned null" };
        }

        if (!brep.is_triangulated()) {
            return { 
                compatible: false, 
                reason: "Object is not triangulated. Call generate_geometry() first.",
                geometryType: brep.geometry_type
            };
        }

        try {
            brep.validate();
        } catch (error) {
            return { 
                compatible: false, 
                reason: `Object has invalid geometry: ${error}`,
                geometryType: brep.geometry_type
            };
        }

        return {
            compatible: true,
            triangleCount: brep.triangle_count(),
            geometryType: brep.geometry_type
        };
    }
}