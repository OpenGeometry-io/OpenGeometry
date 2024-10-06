import { Vector3D, BasePolygon } from "../opengeometry/pkg/opengeometry";
import * as THREE from "three";
export declare class OpenGeometry {
    constructor();
    setup(): Promise<void>;
}
export declare class BasePoly extends THREE.Mesh {
    polygon: BasePolygon | null;
    isTriangulated: boolean;
    constructor(vertices?: Vector3D[]);
    addVertex(vertex: Vector3D): void;
    private getBuf;
}
export { Vector3D };
