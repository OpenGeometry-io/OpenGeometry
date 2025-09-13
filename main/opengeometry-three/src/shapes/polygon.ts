import * as THREE from "three";
import { OGPolygon, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { getUUID } from "../utils/randomizer";

interface IPolygonOptions {
  vertices: Vector3[];
}

export class Polygon extends THREE.Mesh {
  ogid: string;
  options: IPolygonOptions = { vertices: [] };
  polygon: OGPolygon;
  #outlineMesh: THREE.Line | null = null;

  #color: number = 0x00ff00;

  transformationMatrix: THREE.Matrix4 = new THREE.Matrix4();

  set color(color: number) {
    this.#color = color;
    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(color);
    }
  }

  layerVertices: Vector3[] = [];
  layerBackVertices: Vector3[] = [];

  isTriangulated: boolean = false;

  private _placement: THREE.Vector3 = new THREE.Vector3(0, 0, 0);
  private _yaw: number = 0;

  // TODO: Make Options Optional
  // constructor(vertices?: Vector3[]) // If no vertices are provided, it will be an empty polygon
  constructor(options?: IPolygonOptions) {
    super();
    this.ogid = getUUID();
    if (options) {
      this.options = options;
      console.log("Polygon options set:", this.options);
    }
    this.polygon = new OGPolygon(this.ogid);
    
    this.setConfig();
    this.generateGeometry();

    // TODO: THIS MIGHT HELP WITH SHARING THE POSITION with KERNEL when something is changed
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

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Polygon");
    }
  }

  setConfig() {
    this.validateOptions();
    const { vertices } = this.options;
    this.polygon.set_config(vertices);
  }

  /**
   * Sets the placement of the polygon in 3D space.
   * @param x X-coordinate
   * @param y Y-coordinate
   * @param z Z-coordinate
   */
  placement(x: number, y: number, z: number) {
    this._placement.set(x, y, z);
    
    // Do the recalculation of position based on the placement
    const clonedObject = this.clone();
    clonedObject.rotation.set(0, 0, 0);

    clonedObject.geometry.computeBoundingBox();
    if (!clonedObject.geometry.boundingBox) return;

    const center = new THREE.Vector3();
    clonedObject.geometry.boundingBox.getCenter(center);
    const min = clonedObject.geometry.boundingBox.min;
    this.position.set(
      center.x + this._placement.x - min.x,
      // this._placement.y,
      0.01, // Set Y to a small value to avoid z-fighting
      center.z + this._placement.z - min.z
    );
  }

  positionToPlacement() {
    const clonedObject = this.clone();
    clonedObject.rotation.set(0, 0, 0);
    clonedObject.geometry.computeBoundingBox();
    if (!clonedObject.geometry.boundingBox) return;
    const min = clonedObject.geometry.boundingBox.min;

    this._placement.set(
      this.position.x + min.x,
      this.position.y + min.y,
      this.position.z + min.z
    );

    // console.log("Placement set to:", this._placement.x, this._placement.y, this._placement.z);
  }

  /**
   * Rotates the polygon around the Y-axis.
   * @param angle Rotation angle in Degrees
   */
  set yaw(angle: number) {
    this._yaw = angle;
    
    this.rotation.set(0, 0, 0);
    this.rotation.y = THREE.MathUtils.degToRad(this._yaw);
  }

  get yaw() {
    return this._yaw;
  }

  generateGeometry() {

    // this.updateMatrix();
    // this.transformationMatrix.copy(this.matrix);
    // console.log("Transformation matrix set for polygon:", this.transformationMatrix.elements);

    this.polygon.generate_geometry();
    const geometryData = this.polygon.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);
    console.log("Buffer data for polygon geometry:", bufferData);

    // TODO: If The Geometry is empty, no need to adjust position
    if (bufferData.length === 0) {
      console.warn("Geometry has no position attribute, skipping position adjustment.");
      return;
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    const material = new THREE.MeshStandardMaterial({
      color: this.#color,
      transparent: true,
      opacity: 0.6,
    });

    this.geometry = geometry;
    this.material = material;

    console.log("Original Position:", this.position.x, this.position.y, this.position.z);

    // this.geometry.computeBoundingBox();
    // const originalCenter = new THREE.Vector3();
    // this.geometry.boundingBox?.getCenter(originalCenter);
    // console.log("Original Center:", originalCenter.x, originalCenter.y, originalCenter.z);

    // this.geometry.center();
    // this.geometry.computeBoundingBox();
    // const newCenter = new THREE.Vector3();
    // this.geometry.boundingBox?.getCenter(newCenter);
    // console.log("New Center after centering:", newCenter.x, newCenter.y, newCenter.z);

    // if (!this.geometry.boundingBox) return;
    // const min = this.geometry.boundingBox.min;
    // // console.log("Position before centering:", this.position.x, this.position.y, this.position.z);
    // console.log("Bounding Box Min:", min.x, min.y, min.z);
    // this.position.set(-min.x, 0, -min.z);

    // if (this._placement) {
    //   this.placement(this._placement.x, this._placement.y, this._placement.z);
    // }

    console.log("New Position after centering:", this.position.x, this.position.y, this.position.z);
  }

  addVertices(vertices: Vector3[]) {
    if (!this.polygon) return;

    this.disposeGeometryMaterial();
    this.polygon.add_vertices(vertices);
    this.generateGeometry();
  }

  saveTransformationToBREP() {
    if (!this.polygon) return;
  }

  /**
   * Rotates the object around the Y-axis.
   * @param angle Rotation angle in Degrees
   * @returns 
   * @summary If rotation methods from threejs is used, it will rotate around the first vertex which might not be desired.
   */
  rotateOnYAxis(angle: number) {
    if (!this.polygon) return;
  }

  // resetVertices() {
  //   if (!this.polygon) return;
  //   this.layerVertices = [];
  //   this.geometry.dispose();
  //   this.polygon?.clear_vertices();
  //   this.isTriangulated = false;
  // }

  // addVertex(threeVertex: Vector3) {
  //   if (this.isTriangulated) {
  //     this.layerVertices = [];
  //     this.geometry.dispose();
  //     this.polygon?.clear_vertices();
  //     this.isTriangulated = false;

  //     for (const vertex of this.layerBackVertices) {
  //       this.layerVertices.push(vertex.clone());
  //     }

  //   };

  //   const backupVertex = new Vector3(
  //     parseFloat(threeVertex.x.toFixed(2)),
  //     0,
  //     parseFloat(threeVertex.z.toFixed(2))
  //   );
  //   this.layerBackVertices.push(backupVertex);

  //   const vertex = new Vector3(
  //     parseFloat(threeVertex.x.toFixed(2)),
  //     // when doing the parse operation getting -0 instead of 0
  //     0,
  //     parseFloat(threeVertex.z.toFixed(2))
  //   );
  //   this.layerVertices.push(vertex);

  //   if (this.layerVertices.length > 3) {
  //     this.polygon?.add_vertices(this.layerVertices);
  //     const bufFlush = this.polygon?.triangulate();
      
  //     if (!bufFlush) {
  //       return;
  //     }
  //     this.addFlushBufferToScene(bufFlush);

  //     this.isTriangulated = true;
  //   }
  // }

  // addHole(holeVertices: Vector3[]) {
  //   if (!this.polygon) return;
  //   this.polygon.add_holes(holeVertices);
  //   const triResult = JSON.parse(this.polygon.triangulate_with_holes());
  //   console.log(triResult);
  //   const newBufferFlush = triResult.new_buffer;
  //   const geometry = new THREE.BufferGeometry();
  //   geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(newBufferFlush), 3));
  //   this.geometry = geometry;

  //   // const bufFlush = this.polygon.get_buffer_flush();
  //   // this.addFlushBufferToScene(bufFlush);
  // }

  // extrude(height: number) {
  //   if (!this.polygon) return;
  //   const extruded_buff = this.polygon.extrude_by_height(height);
  //   console.log(extruded_buff);
  //   this.generateExtrudedGeometry(extruded_buff);
  // }

  generateExtrudedGeometry(extruded_buff: string) {
    // THIS WORKS
    const flushBuffer = JSON.parse(extruded_buff);
    console.log(flushBuffer);

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    geometry.computeVertexNormals();
    this.geometry = geometry;

    const material = new THREE.MeshStandardMaterial({
      color: 0x00ff00, 
      side: THREE.FrontSide, 
      transparent: true, 
      opacity: 0.5, 
      // wireframe: true
    });
    this.material = material;
  }

  getBrepData() {
    if (!this.polygon) return null;
    const brepData = this.polygon.get_brep_serialized();
    return brepData;
  }

  set outlineColor(color: number) {
    if (this.#outlineMesh && this.#outlineMesh.material instanceof THREE.LineBasicMaterial) {
      this.#outlineMesh.material.color.set(color);
    }
  }

  get outlineColor() {
    if (this.#outlineMesh && this.#outlineMesh.material instanceof THREE.LineBasicMaterial) {
      return this.#outlineMesh.material.color.getHex();
    }
    return 0x000000; // Default color if outline mesh is not present
  }

  set outline(enable: boolean) {
    if (enable && !this.#outlineMesh) {
      const outline_buff = this.polygon.get_outline_geometry_serialized();
      const outline_buf = JSON.parse(outline_buff);

      const outlineGeometry = new THREE.BufferGeometry();
      outlineGeometry.setAttribute(
        "position",
        new THREE.Float32BufferAttribute(outline_buf, 3)
      );

      const outlineMaterial = new THREE.LineBasicMaterial({ color: 0x000000 });
      this.#outlineMesh = new THREE.LineSegments(
        outlineGeometry,
        outlineMaterial
      );

      this.#outlineMesh.geometry.center();
      // this.#outlineMesh.applyMatrix4(this.transformationMatrix);

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

  // bTree() {
  //   if (!this.polygon) return;
  //   const bTree = this.polygon.binary_tree();
  //   const parsedData = JSON.parse(bTree);
  //   console.log(parsedData);
  // }

  disposeGeometryMaterial() {
    if (this.geometry) {
      this.geometry.dispose();
    }
    if (this.material instanceof THREE.Material) {
      this.material.dispose();
    }
    if (this.#outlineMesh) {
      this.#outlineMesh.geometry.dispose();
      if (this.#outlineMesh.material instanceof THREE.Material) {
        this.#outlineMesh.material.dispose();
      }
    }
  }

  dispose() {
    // // console.log("Disposing OG - Polygon");
    // this.geometry.dispose();
    // if (this.material instanceof THREE.Material) {
    //   this.material.dispose();
    // }
    // if (this.#outlineMesh) {
    //   this.#outlineMesh.geometry.dispose();
    //   if (this.#outlineMesh.material instanceof THREE.Material) {
    //     this.#outlineMesh.material.dispose();
    //   }
    // }
    // this.polygon = null;
    // this.layerVertices = [];
    // this.layerBackVertices = [];
  }
}