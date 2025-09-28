import init, {
  Vector3
} from "../opengeometry/pkg/opengeometry";
// Vector3 is also available in opengeometry package
// import { Vector3 } from "@opengeometry/openmaths";
import * as THREE from "three";
import { CSS2DRenderer } from "three/examples/jsm/renderers/CSS2DRenderer.js";
import { getUUID } from "./src/utils/randomizer";
import { Pencil } from "./src/pencil";
import { SpotLabel } from "./src/markup/spotMarker";
import { OPEN_GEOMETRY_THREE_VERSION, OpenGeometryOptions } from "./src/base-types";

export type OUTLINE_TYPE = "front" | "side" | "top";

export class OpenGeometry {
  static version = OPEN_GEOMETRY_THREE_VERSION;

  protected scene: THREE.Scene | undefined;
  protected container: HTMLElement | undefined;
  private _pencil: Pencil | undefined;
  private _labelRenderer: CSS2DRenderer | undefined;

  private _enableDebug: boolean = false;

  set enablePencil(value: boolean) {
    if (value && !this._pencil) {
      if (!this.container || !this.scene) {
        throw new Error("Container or Scene is not defined");
      }
      this._pencil = new Pencil(this.container, this.scene, this.camera);
    } else if (!value && this._pencil) {
      // TODO: Disable The Pencil Usage and Dispose it
    }
  }

  get pencil() {
    return this._pencil;
  }

  get labelRenderer() {
    return this._labelRenderer;
  }

  get enableDebug() {
    return this._enableDebug;
  }

  /**
   * Enables or disables debug mode for OpenGeometry.
   * When enabled, it logs debug information to the console.
   * Addtionally,
   * 1. The geometry will be rendered with a semi-transparent material.
   * 2. The faces will be rendered with a random color.
   * 3. The normals will be rendered for better visualization.
   * @param value - A boolean indicating whether to enable or disable debug mode.
   */
  set enableDebug(value: boolean) {
    this._enableDebug = value;
    if (this._enableDebug) {
      console.log("OpenGeometry Debug Mode Enabled");
    }
  }

  constructor(container:HTMLElement, threeScene: THREE.Scene, private camera: THREE.Camera) {
    this.scene = threeScene;
    this.container = container;
  }

  /**
   * Asynchronously creates and initializes an instance of OpenGeometry.
   *
   * This factory method sets up the OpenGeometry engine by linking it with the
   * rendering context and the WebAssembly module. It ensures all required
   * options are provided and prepares the instance for 3D geometry operations.
   *
   * @param options - Configuration object for initializing OpenGeometry.
   * @returns A promise that resolves to a fully initialized OpenGeometry instance.
   * @throws If any of the required options (`container`, `scene`, or `camera`) are missing.
   *
   * @example
   * ```ts
   * const openGeometry = await OpenGeometry.create({
   *   container: document.getElementById('myContainer')!,
   *   scene: threeScene,
   *   camera: threeCamera,
   *   wasmURL: '/assets/opengeometry.wasm'
   * });
   * ```
   */
  static async create(options: OpenGeometryOptions) {
    const { container, scene, camera } = options;
    if (!container || !scene || !camera) {
      throw new Error("Missing required options");
    }
    const openGeometry = new OpenGeometry(container, scene, camera);
    await openGeometry.setup(options.wasmURL);
    return openGeometry;
  }

  private async setup(wasmURL: string) {
    await init(wasmURL);
    this.setuplabelRenderer();
    this.setupEvent();
  }

  private setuplabelRenderer() {
    if (!this.container || !this.scene) {
      throw new Error("Container or Scene is not defined");
    }

    const labelRenderer = new CSS2DRenderer();
    labelRenderer.setSize(this.container.clientWidth, this.container.clientHeight);
    labelRenderer.domElement.style.position = "absolute";
    labelRenderer.domElement.style.top = "0";
    this.container.appendChild(labelRenderer.domElement);
    this._labelRenderer = labelRenderer;
  }

  private setupEvent() {
    // NOTE: The responsibility to resize normal rendererer lies with the user
    // but the label renderer should be resized automatically
    window.addEventListener("resize", () => {
      if (!this.container) return;
      this._labelRenderer?.setSize(this.container?.clientWidth, this.container?.clientHeight);
    });
  }

  // TODO: Can this be handled inside the OpenGeometry class itself?
  /**
   * Updates the label renderer to render the scene with the given camera.
   * This method should be called in the animation loop or render loop of your application.
   * @param scene - The Three.js scene containing the objects to be rendered.
   * @param camera - The Three.js camera used for rendering the scene.
   */
  update(scene: THREE.Scene, camera: THREE.Camera) {
    this._labelRenderer?.render(scene, camera);
  }
}

// export class BasePoly extends THREE.Mesh {
//   ogid: string;
//   layerVertices: Vector3[] = [];
//   layerBackVertices: Vector3[] = [];

//   polygon: BasePolygon | null = null;
//   isTriangulated: boolean = false;

//   constructor(vertices?: Vector3[]) {
//     super();
//     this.ogid = getUUID();
//     this.polygon = new BasePolygon(this.ogid);
    
//     if (vertices) {
//       this.polygon.add_vertices(vertices);

//       // Triangulate the polygon
//       this.polygon?.triangulate();

//       const bufFlush = this.polygon?.get_buffer_flush();
//       this.addFlushBufferToScene(bufFlush);
//     }
//   }

//   addVertices(vertices: Vector3[]) {
//     if (!this.polygon) return;
//     this.polygon.add_vertices(vertices);
//     this.polygon?.triangulate();
//     const bufFlush = this.polygon?.get_buffer_flush();
//     this.addFlushBufferToScene(bufFlush);
//   }

//   resetVertices() {
//     if (!this.polygon) return;
//     this.layerVertices = [];
//     this.geometry.dispose();
//     this.polygon?.reset_polygon();
//     this.isTriangulated = false;
//   }

//   addVertex(threeVertex: Vector3) {
//     if (this.isTriangulated) {
//       this.layerVertices = [];
//       this.geometry.dispose();
//       this.polygon?.reset_polygon();
//       this.isTriangulated = false;

//       for (const vertex of this.layerBackVertices) {
//         this.layerVertices.push(vertex.clone());
//       }

//     };

//     const backupVertex = new Vector3(
//       parseFloat(threeVertex.x.toFixed(2)),
//       0,
//       parseFloat(threeVertex.z.toFixed(2))
//     );
//     this.layerBackVertices.push(backupVertex);

//     const vertex = new Vector3(
//       parseFloat(threeVertex.x.toFixed(2)),
//       // when doing the parse operation getting -0 instead of 0
//       0,
//       parseFloat(threeVertex.z.toFixed(2))
//     );
//     this.layerVertices.push(vertex);

//     if (this.layerVertices.length > 3) {
//       this.polygon?.add_vertices(this.layerVertices);
//       const bufFlush = this.polygon?.triangulate();
      
//       if (!bufFlush) {
//         return;
//       }
//       this.addFlushBufferToScene(bufFlush);

//       this.isTriangulated = true;
//     }
//   }

//   addHole(holeVertices: Vector3[]) {
//     if (!this.polygon) return;
//     this.polygon.add_holes(holeVertices);
//     const triResult = JSON.parse(this.polygon.new_triangulate());
//     const newBufferFlush = triResult.new_buffer;
//     const geometry = new THREE.BufferGeometry();
//     geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(newBufferFlush), 3));
//     this.geometry = geometry;

//     // const bufFlush = this.polygon.get_buffer_flush();
//     // this.addFlushBufferToScene(bufFlush);
//   }

//   addFlushBufferToScene(flush: string) {
//     const flushBuffer = JSON.parse(flush);
//     const geometry = new THREE.BufferGeometry();
//     geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
//     // geometry.computeVertexNormals();

//     const material = new THREE.MeshStandardMaterial({
//       color: 0x00ff00, 
//       // side: THREE.DoubleSide, 
//       transparent: true, 
//       opacity: 0.5, 
//       // wireframe: true
//     });
    
//     this.geometry = geometry;
//     this.material = material;
//   }

//   extrude(height: number) {
//     if (!this.polygon) return;
//     const extruded_buff = this.polygon.extrude_by_height(height);
//     this.generateExtrudedGeometry(extruded_buff);
//   }

//   generateExtrudedGeometry(extruded_buff: string) {
//     // THIS WORKS
//     const flushBuffer = JSON.parse(extruded_buff);
//     console.log(flushBuffer);

//     // const geometry = new THREE.BufferGeometry();
//     // geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
//     // // geometry.computeVertexNormals();

//     // // const material = new THREE.MeshPhongMaterial({
//     // //   color: 0x3a86ff,
//     // // });
//     // // material.side = THREE.DoubleSide;

//     // this.geometry = geometry;
//     // this.material = material;
//   }
// }

// interface IBaseCircleOptions {
//   radius: number;
//   segments: number;
//   position: Vector3;
//   startAngle: number;
//   endAngle: number;
// }

// export class CirclePoly extends THREE.Mesh {
//   ogid: string;
//   polygon: OGPolygon | null = null;
//   baseCircle: Arc;
//   isExtruded: boolean = false;

//   constructor(baseCircle: Arc) {
//     super();
//     this.ogid = getUUID();

//     if (!baseCircle.circleArc) {
//       throw new Error("CircleArc is not defined");
//     }
//     // baseCircle.nodeChild = this;
//     baseCircle.nodeOperation = "polygon";
//     this.baseCircle = baseCircle;

//     this.generateGeometry();
//     this.addFlushBufferToScene();
//   }

//   update() {
//     this.geometry.dispose();

//     this.polygon?.clear_vertices();
//     this.polygon?.add_vertices(this.baseCircle.circleArc.get_raw_points());
    
//     this.generateGeometry();
//     this.addFlushBufferToScene();
//   }

//   generateGeometry() {
//     if (!this.baseCircle.circleArc) return;
//     this.polygon = OGPolygon.new_with_circle(this.baseCircle.circleArc.clone());
//   }

//   addFlushBufferToScene() {
//     if (!this.polygon) return;
//     const bufFlush = this.polygon.get_buffer_flush();
//     const flushBuffer = JSON.parse(bufFlush);
//     const geometry = new THREE.BufferGeometry();
//     geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    
//     // TODO: Do this using a set method, poly.visualizeTriangles = true
//     // different colors for each triangle in the polygon dont interolate
//     // const colors = new Float32Array(flushBuffer.length);
//     // for (let i = 0; i < colors.length; i += 9) {
//     //   const r = Math.random();
//     //   const g = Math.random();
//     //   const b = Math.random();
//     //   colors[i] = r;
//     //   colors[i + 1] = g;
//     //   colors[i + 2] = b;
//     //   colors[i + 3] = r;
//     //   colors[i + 4] = g;
//     //   colors[i + 5] = b;
//     //   colors[i + 6] = r;
//     //   colors[i + 7] = g;
//     //   colors[i + 8] = b;
//     // }

//     // geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));

//     const material = new THREE.MeshStandardMaterial({
//       color: 0x00ff00, 
//       // side: THREE.DoubleSide, 
//       transparent: true, 
//       opacity: 0.5, 
//       // wireframe: true
//     });

//     this.geometry = geometry;
//     this.material = material;
//   }

//   clearGeometry() {
//     this.geometry.dispose();
//   }

//   extrude(height: number) {
//     if (!this.polygon) return;
//     const extruded_buff = this.polygon.extrude_by_height(height);
//     console.log(JSON.parse(extruded_buff));
//     this.isExtruded = true;
    
//     this.generateExtrudedGeometry(extruded_buff);
//   }

//   getBrepData() {
//     if (!this.polygon) return;
//     const brepData = this.polygon.get_brep_data();
//     const parsedData = JSON.parse(brepData);
//     console.log(parsedData);
//   }
 
//   getOutline(type: OUTLINE_TYPE) {
//     if (!this.polygon) return;
//     const outlines = this.polygon.get_outlines();
//     const outlineBuffer = JSON.parse(outlines);

//     // TODO: move this logic to Kernel
//     const faces = [];
//     for (const data of outlineBuffer) {
//       const vertices = [];
//       for (const vertex of data) {
//         const x_float = type === "side" ? 0 : parseFloat(vertex.x.toFixed(5));
//         const y_float = type === "top" ? 0 : parseFloat(vertex.y.toFixed(5));
//         const z_float = type === "front" ? 0 : parseFloat(vertex.z.toFixed(5));
//         vertices.push(new THREE.Vector3(x_float, y_float, z_float));
//       }
//       faces.push(vertices);
//     }

//     const clonedFaces = faces.map((face) => {
//       return face.map((vertex) => {
//         return new THREE.Vector3(vertex.x, vertex.y, vertex.z);
//       });
//     }
//     );

//     // remove duplicates inside the faces
//     const uniqueFaces = clonedFaces.map((face) => {
//       return face.filter((vertex, index, self) =>
//         index === self.findIndex((v) => (
//           v.x === vertex.x && v.y === vertex.y && v.z === vertex.z
//         ))
//       );
//     });

//     // Picking unique vertices from all faces
//     const uniqueVertices = [];
//     const vertexSet = new Set();
//     for (const face of uniqueFaces) {
//       for (const vertex of face) {
//         const key = `${vertex.x},${vertex.y},${vertex.z}`;
//         if (!vertexSet.has(key)) {
//           vertexSet.add(key);
//           uniqueVertices.push(vertex);
//         }
//       }
//     }
    
//     // arrange the vertices in a clockwise manner
//     const center = new THREE.Vector3();
//     for (const vertex of uniqueVertices) {
//       center.add(vertex);
//     }
//     center.divideScalar(uniqueVertices.length);
//     uniqueVertices.sort((a, b) => {
//       if (type === "side") {
//         const angleA = Math.atan2(a.y - center.y, a.z - center.z);
//         const angleB = Math.atan2(b.y - center.y, b.z - center.z);
//         return angleA - angleB;
//       } else if (type === "top") {
//         const angleA = Math.atan2(a.x - center.x, a.z - center.z);
//         const angleB = Math.atan2(b.x - center.x, b.z - center.z);
//         return angleA - angleB;
//       }
//       const angleA = Math.atan2(a.x - center.x, a.y - center.y);
//       const angleB = Math.atan2(b.x - center.x, b.y - center.y);
//       return angleA - angleB;
//     }
//     );

//     // merge collinear vertices
//     const mergedVertices = [];
//     for (let i = 0; i < uniqueVertices.length; i++) {
//       const current = uniqueVertices[i];
//       const next = uniqueVertices[(i + 1) % uniqueVertices.length];
//       const prev = uniqueVertices[(i - 1 + uniqueVertices.length) % uniqueVertices.length];

//       const v1 = new THREE.Vector3().subVectors(current, prev);
//       const v2 = new THREE.Vector3().subVectors(next, current);

//       if (v1.angleTo(v2) > 0.01) {
//         mergedVertices.push(current);
//       }
//     }

//     mergedVertices.push(mergedVertices[0]);
//     // TODO: move logic until here to Kernel

//     // Create a new geometry with the merged vertices
//     const mergedGeometry = new THREE.BufferGeometry().setFromPoints(mergedVertices);
//     const mergedMaterial = new THREE.MeshBasicMaterial({ color: 0x000000, side: THREE.DoubleSide });
//     const mergedMesh = new THREE.Line(mergedGeometry, mergedMaterial);
//     return mergedMesh;
//   }

//   generateExtrudedGeometry(extruded_buff: string) {
//     // THIS WORKS
//     const flushBuffer = JSON.parse(extruded_buff);
//     const geometry = new THREE.BufferGeometry();
//     geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));

//     // To Test If Triangulation is working
//     // const colors = new Float32Array(flushBuffer.length);
//     // for (let i = 0; i < colors.length; i += 9) {
//     //   const r = Math.random();
//     //   const g = Math.random();
//     //   const b = Math.random();
//     //   colors[i] = r;
//     //   colors[i + 1] = g;
//     //   colors[i + 2] = b;
//     //   colors[i + 3] = r;
//     //   colors[i + 4] = g;
//     //   colors[i + 5] = b;
//     //   colors[i + 6] = r;
//     //   colors[i + 7] = g;
//     //   colors[i + 8] = b;
//     // }

//     // geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));

//     // const material = new THREE.MeshPhongMaterial( {
//     //     color: 0xffffff,
//     //     flatShading: true,
//     //     vertexColors: true,
//     //     shininess: 0,
//     //     side: THREE.DoubleSide
//     // });

//     // const material = new THREE.MeshPhongMaterial({
//     //   color: 0x3a86ff,
//     // });
//     // material.side = THREE.DoubleSide;
    
//     this.geometry = geometry;
//     // this.material = material;
//   }

//   dispose() {
//     if (!this.polygon) return;
//     this.geometry.dispose();
//     this.polygon?.clear_vertices();
//     this.polygon = null;
//     this.isExtruded = false;
//   }
// }

// export class RectanglePoly extends THREE.Mesh {
//   ogid: string;
//   polygon: BasePolygon | null = null;
//   baseRectangle: Rectangle;
//   isExtruded: boolean = false;
//   constructor(baseRectangle: Rectangle) {
//     super();
//     this.ogid = getUUID();
   
//     // if (!baseRectangle.polyLineRectangle) {
//     //   throw new Error("BaseRectangle is not defined");
//     // }
//     // // baseRectangle.nodeChild = this;
//     // baseRectangle.nodeOperation = "polygon";
//     this.baseRectangle = baseRectangle;
    
//     this.generateGeometry();
//     this.addFlushBufferToScene();
//   }

//   update() {
//     this.geometry.dispose();
//     this.polygon?.clear_vertices();
//     // this.polygon?.add_vertices(this.baseRectangle.polyLineRectangle.get_raw_points());
//     this.generateGeometry();
//     this.addFlushBufferToScene();
//   }
//   generateGeometry() {
//     // if (!this.baseRectangle.polyLineRectangle) return;
//     // this.polygon = BasePolygon.new_with_rectangle(this.baseRectangle.polyLineRectangle.clone());
//   }

//   addFlushBufferToScene() {
//     if (!this.polygon) return;
//     const bufFlush = this.polygon.get_buffer_flush();
//     const flushBuffer = JSON.parse(bufFlush);
//     const geometry = new THREE.BufferGeometry();
//     geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
//     const material = new THREE.MeshStandardMaterial({ color: 0x3a86ff, transparent: true, opacity: 0.5 });
//     this.geometry = geometry;
//     this.material = material;
//   }

//   clearGeometry() {
//     this.geometry.dispose();
//   }

//   extrude(height: number) {
//     if (!this.polygon) return;
//     const extruded_buff = this.polygon.extrude_by_height(height);
//     this.isExtruded = true;
//     this.generateExtrudedGeometry(extruded_buff);
//   }

//   generateExtrudedGeometry(extruded_buff: string) {
//     // THIS WORKS
//     const flushBuffer = JSON.parse(extruded_buff);
//     const geometry = new THREE.BufferGeometry();
//     geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
//     geometry.computeVertexNormals();

//     // const colors = new Float32Array(flushBuffer.length);
//     // for (let i = 0; i < colors.length; i += 9) {
//     //   const r = Math.random();
//     //   const g = Math.random();
//     //   const b = Math.random();
//     //   colors[i] = r;
//     //   colors[i + 1] = g;
//     //   colors[i + 2] = b;
//     //   colors[i + 3] = r;
//     //   colors[i + 4] = g;
//     //   colors[i + 5] = b;
//     //   colors[i + 6] = r;
//     //   colors[i + 7] = g;
//     //   colors[i + 8] = b;
//     // }
//     // geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));
//     // const material = new THREE.MeshPhongMaterial( {
//     //     color: 0xffffff,
//     //     flatShading: true,
//     //     vertexColors: true,
//     //     shininess: 0,
//     //     side: THREE.DoubleSide
//     // });

//     const material = new THREE.MeshPhongMaterial({
//       color: 0x3a86ff,
//     });
//     material.side = THREE.DoubleSide;

//     this.geometry = geometry;
//     this.material = material;
//   }

//   getOutline(type: OUTLINE_TYPE) {
//     if (!this.polygon) return;
//     const outlines = this.polygon.get_outlines();
//     const outlineBuffer = JSON.parse(outlines);

//     // TODO: move this logic to Kernel
//     const faces = [];
//     for (const data of outlineBuffer) {
//       const vertices = [];
//       for (const vertex of data) {
//         const x_float = type === "side" ? 0 : parseFloat(vertex.x.toFixed(5));
//         const y_float = type === "top" ? 0 : parseFloat(vertex.y.toFixed(5));
//         const z_float = type === "front" ? 0 : parseFloat(vertex.z.toFixed(5));
//         vertices.push(new THREE.Vector3(x_float, y_float, z_float));
//       }
//       faces.push(vertices);
//     }

//     const clonedFaces = faces.map((face) => {
//       return face.map((vertex) => {
//         return new THREE.Vector3(vertex.x, vertex.y, vertex.z);
//       });
//     }
//     );

//     // remove duplicates inside the faces
//     const uniqueFaces = clonedFaces.map((face) => {
//       return face.filter((vertex, index, self) =>
//         index === self.findIndex((v) => (
//           v.x === vertex.x && v.y === vertex.y && v.z === vertex.z
//         ))
//       );
//     });

//     // Picking unique vertices from all faces
//     const uniqueVertices = [];
//     const vertexSet = new Set();
//     for (const face of uniqueFaces) {
//       for (const vertex of face) {
//         const key = `${vertex.x},${vertex.y},${vertex.z}`;
//         if (!vertexSet.has(key)) {
//           vertexSet.add(key);
//           uniqueVertices.push(vertex);
//         }
//       }
//     }
    
//     // arrange the vertices in a clockwise manner
//     const center = new THREE.Vector3();
//     for (const vertex of uniqueVertices) {
//       center.add(vertex);
//     }
//     center.divideScalar(uniqueVertices.length);
//     uniqueVertices.sort((a, b) => {
//       if (type === "side") {
//         const angleA = Math.atan2(a.y - center.y, a.z - center.z);
//         const angleB = Math.atan2(b.y - center.y, b.z - center.z);
//         return angleA - angleB;
//       } else if (type === "top") {
//         const angleA = Math.atan2(a.x - center.x, a.z - center.z);
//         const angleB = Math.atan2(b.x - center.x, b.z - center.z);
//         return angleA - angleB;
//       }
//       const angleA = Math.atan2(a.x - center.x, a.y - center.y);
//       const angleB = Math.atan2(b.x - center.x, b.y - center.y);
//       return angleA - angleB;
//     }
//     );

//     // merge collinear vertices
//     const mergedVertices = [];
//     for (let i = 0; i < uniqueVertices.length; i++) {
//       const current = uniqueVertices[i];
//       const next = uniqueVertices[(i + 1) % uniqueVertices.length];
//       const prev = uniqueVertices[(i - 1 + uniqueVertices.length) % uniqueVertices.length];

//       const v1 = new THREE.Vector3().subVectors(current, prev);
//       const v2 = new THREE.Vector3().subVectors(next, current);

//       if (v1.angleTo(v2) > 0.01) {
//         mergedVertices.push(current);
//       }
//     }

//     mergedVertices.push(mergedVertices[0]);
//     // TODO: move logic until here to Kernel

//     // Create a new geometry with the merged vertices
//     const mergedGeometry = new THREE.BufferGeometry().setFromPoints(mergedVertices);
//     const mergedMaterial = new THREE.MeshBasicMaterial({ color: 0x000000, side: THREE.DoubleSide });
//     const mergedMesh = new THREE.Line(mergedGeometry, mergedMaterial);
//     return mergedMesh;
//   }
// }

export {
  Vector3,
  SpotLabel,
}

export * from './src/primitives/';
export * from './src/shapes/';
