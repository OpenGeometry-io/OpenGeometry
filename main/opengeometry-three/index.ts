import init, { Vector3D, Polygon, Triangle, get_tricut_vertices, triangulate_mesh } from "../openmaths/openmaths";
import * as THREE from "three";

export class OpenGeometry {
  polygons: Polygon[] = [];

  constructor() {
    // this.setup();
  }

  async setup() {
    await init();
    console.log("OpenMaths initialized");
  }

  // createPolygon(vertices?: Vector3D[]) {
  //   const polygon = new OPolygon(vertices);
  //   this.polygons.push(polygon.createPolygon());
  //   return polygon;
  // }
}

export function OVector3D(x: number, y: number, z: number) {
  return new Vector3D(x, y, z);
}

export class OPolygon extends THREE.Mesh {
  vertices: Vector3D[] = [];
  polygon: Polygon = new Polygon();

  constructor() {
    super();
  }

  addVertex(x: number, y: number, z: number) {
    const xVertex = x;
    const yVertex = y;
    const zVertex = z;
    const vertex = new Vector3D(xVertex, yVertex, zVertex);
    this.polygon.add_vertex(vertex);
    this.vertices.push(vertex);
  }

  generateMesh() {
    const oGeometry = this.polygon.earcut();
  }

  extrude(height: number) {
    console.log("Extruding polygon");
    if (!(typeof height === "number")) {
      throw new Error("Extrude height must be a number");
    }
    const polySolid = this.polygon.set_extrude(true);
    polySolid.set_extrude_height(height);

    const geometry = triangulate_mesh(polySolid);
    return geometry;
  }
  

  extrudeMesh() {}
}
