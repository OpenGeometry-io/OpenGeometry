import * as THREE from "three";
import { OGPolygon, Vector3D } from "../../../opengeometry/pkg/opengeometry";
import { getUUID } from "../utils/randomizer";

export class Polygon extends THREE.Mesh {
  ogid: string;
  layerVertices: Vector3D[] = [];
  layerBackVertices: Vector3D[] = [];

  polygon: OGPolygon | null = null;
  isTriangulated: boolean = false;

  #outlineMesh: THREE.Line | null = null;
  private _geometryCenterOffset = new THREE.Vector3();

  constructor(vertices?: Vector3D[]) {
    super();
    this.ogid = getUUID();
    this.polygon = new OGPolygon(this.ogid);
    
    if (vertices) {
      this.polygon.add_vertices(vertices);

      // Triangulate the polygon - WORKS
      this.polygon?.triangulate();
      const bufFlush = this.polygon?.get_buffer_flush();
      this.addFlushBufferToScene(bufFlush);
      
      // Testing New Triangulation - FAILING
      // const triResult = JSON.parse(this.polygon.new_triangulate());
      // console.log(triResult);
    }

    // THIS MIGHT HELP WITH SHARING THE POSITION with KERNEL when something is changed
    // const originalSet = this.position.set.bind(this.position);
    // this.position.set = (x: number, y: number, z: number) => {
    //   console.log(`Position set to (${x}, ${y}, ${z})`);
    //   // your custom logic here (e.g., notify OpenGeometry)
    //   return originalSet(x, y, z);
    // };

    // // Optional: Override copy if you're using .copy() too
    // const originalCopy = this.position.copy.bind(this.position);
    // this.position.copy = (v: THREE.Vector3) => {
    //   console.log(`Position copied from ${v.x}, ${v.y}, ${v.z}`);
    //   return originalCopy(v);
    // };
  }

  addVertices(vertices: Vector3D[]) {
    if (!this.polygon) return;
    this.polygon.add_vertices(vertices);
    this.polygon?.triangulate();
    const bufFlush = this.polygon?.get_buffer_flush();
    this.addFlushBufferToScene(bufFlush);
  }

  resetVertices() {
    if (!this.polygon) return;
    this.layerVertices = [];
    this.geometry.dispose();
    this.polygon?.reset_polygon();
    this.isTriangulated = false;
  }

  addVertex(threeVertex: Vector3D) {
    if (this.isTriangulated) {
      this.layerVertices = [];
      this.geometry.dispose();
      this.polygon?.reset_polygon();
      this.isTriangulated = false;

      for (const vertex of this.layerBackVertices) {
        this.layerVertices.push(vertex.clone());
      }

    };

    const backupVertex = new Vector3D(
      parseFloat(threeVertex.x.toFixed(2)),
      0,
      parseFloat(threeVertex.z.toFixed(2))
    );
    this.layerBackVertices.push(backupVertex);

    const vertex = new Vector3D(
      parseFloat(threeVertex.x.toFixed(2)),
      // when doing the parse operation getting -0 instead of 0
      0,
      parseFloat(threeVertex.z.toFixed(2))
    );
    this.layerVertices.push(vertex);

    if (this.layerVertices.length > 3) {
      this.polygon?.add_vertices(this.layerVertices);
      const bufFlush = this.polygon?.triangulate();
      
      if (!bufFlush) {
        return;
      }
      this.addFlushBufferToScene(bufFlush);

      this.isTriangulated = true;
    }
  }

  addHole(holeVertices: Vector3D[]) {
    if (!this.polygon) return;
    this.polygon.add_holes(holeVertices);
    const triResult = JSON.parse(this.polygon.triangulate_with_holes());
    console.log(triResult);
    const newBufferFlush = triResult.new_buffer;
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(newBufferFlush), 3));
    this.geometry = geometry;

    // const bufFlush = this.polygon.get_buffer_flush();
    // this.addFlushBufferToScene(bufFlush);
  }

  addFlushBufferToScene(flush: string) {
    const flushBuffer = JSON.parse(flush);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    geometry.computeVertexNormals();

    const material = new THREE.MeshStandardMaterial({
      color: 0x00ff00, 
      // side: THREE.DoubleSide, 
      transparent: true, 
      opacity: 0.5, 
      // wireframe: true
    });
    
    this.geometry = geometry;
    this.material = material;
  }

  extrude(height: number) {
    if (!this.polygon) return;
    const extruded_buff = this.polygon.extrude_by_height(height);
    console.log(extruded_buff);
    this.generateExtrudedGeometry(extruded_buff);
  }

  generateExtrudedGeometry(extruded_buff: string) {
    // THIS WORKS
    const flushBuffer = JSON.parse(extruded_buff);
    console.log(flushBuffer);

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    geometry.computeVertexNormals();
    this.geometry = geometry;

    // const material = new THREE.MeshStandardMaterial({
    //   color: 0x3a86ff,
    // });
    // // material.side = THREE.DoubleSide;
    // this.material = material;
  }

  getBrepData() {
    if (!this.polygon) return;
    const brepData = this.polygon.get_brep_data();
    const parsedData = JSON.parse(brepData);
    console.log(parsedData); 
  }

  set outline(enable: boolean) {
    if (enable && !this.#outlineMesh && this.polygon) {
      const outline_buff = this.polygon.outline_edges();
      const outline_buf = JSON.parse(outline_buff);

      const outlineGeometry = new THREE.BufferGeometry();
      outlineGeometry.setAttribute(
        "position",
        new THREE.Float32BufferAttribute(outline_buf, 3)
      );

      // TODO: Fix the outline position
      // outlineGeometry.translate(
      //   -this._geometryCenterOffset.x,
      //   -this._geometryCenterOffset.y,
      //   -this._geometryCenterOffset.z
      // );

      const outlineMaterial = new THREE.LineBasicMaterial({ color: 0x000000 });
      this.#outlineMesh = new THREE.LineSegments(
        outlineGeometry,
        outlineMaterial
      );

      this.add(this.#outlineMesh);
    }

    if (!enable && this.#outlineMesh) {
      this.remove(this.#outlineMesh);
      this.#outlineMesh.geometry.dispose();
      this.#outlineMesh = null;
    }
  }

  get outline() {
    if (this.#outlineMesh) {
      return true;
    }
    return false;
  }
}