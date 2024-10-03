import { Vector3D, Polygon } from "../openmaths/openmaths";
import * as THREE from "three";
export declare class OpenGeometry {
    polygons: Polygon[];
    constructor();
    setup(): Promise<void>;
}
export declare function OVector3D(x: number, y: number, z: number): Vector3D;
export declare class OPolygon extends THREE.Mesh {
    vertices: Vector3D[];
    polygon: Polygon;
    constructor();
    addVertex(x: number, y: number, z: number): void;
    generateMesh(): void;
    extrude(height: number): Float64Array;
    extrudeMesh(): void;
}
