import init, { Vector3D, BasePolygon } from "../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "./src/utils/randomizer";

export class OpenGeometry {
  constructor() {
    // this.setup();
  }

  async setup() {
    await init();
    console.log("OpenGeometry Kernel 0.0.1");
  }
}

export class BasePoly extends THREE.Mesh {
  polygon: BasePolygon | null = null;
  isTriangulated: boolean = false;

  constructor(vertices?: Vector3D[]) {
    super();
    this.polygon = new BasePolygon(getUUID());
    
    if (vertices) {
      this.polygon.add_vertices(vertices);

      // Triangulate the polygon
    }

    this.getBuf();
  }

  addVertex(vertex: Vector3D) {
    this.polygon?.add_vertex(vertex);
  }

  private getBuf() {
    console.log(this.polygon?.get_buffer());
  }
}

export {
  Vector3D
}
