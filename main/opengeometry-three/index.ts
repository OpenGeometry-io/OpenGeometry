import init, { 
  Vector3D, 
  BasePolygon,
  BaseFlatMesh,
  CircleArc,
  OGSimpleLine,
  OGPolyLine,
  OGRectangle,
} from "../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { CSS2DRenderer } from "three/examples/jsm/renderers/CSS2DRenderer.js";
import { getUUID } from "./src/utils/randomizer";
import { Pencil } from "./src/pencil";
import { SpotLabel } from "./src/markup/spotMarker";
import { OPEN_GEOMETRY_THREE_VERSION, OpenGeometryOptions } from "./src/base-types";
import { BaseCircle } from "./src/primitives/circle";
import { Rectangle } from "./src/primitives/rectangle";

export type OUTLINE_TYPE = "front" | "side" | "top";

export class OpenGeometry {
  static version = OPEN_GEOMETRY_THREE_VERSION;

  protected scene: THREE.Scene | undefined;
  protected container: HTMLElement | undefined;
  private _pencil: Pencil | undefined;
  private _labelRenderer: CSS2DRenderer | undefined;

  constructor(container:HTMLElement, threeScene: THREE.Scene, private camera: THREE.Camera) {
    this.scene = threeScene;
    this.container = container;
  }

  // Why Generic Types are used sometimes
  // verifyOptions(options: OpenGeometryOptions) {
  //   for (const key in options) {
  //     if (options[key as keyof OpenGeometryOptions] === undefined) {
  //       throw new Error(`Missing required option: ${key}`);
  //     }
  //   }
  // }

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
    if (!this.container || !this.scene) return;
    this._pencil = new Pencil(this.container, this.scene, this.camera);
    this.setupEvent();
  }

  get pencil() {
    return this._pencil;
  }

  get labelRenderer() {
    return this._labelRenderer;
  }

  setuplabelRenderer() {
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

  setupEvent() {
    window.addEventListener("resize", () => {
      if (!this.container) return;
      this._labelRenderer?.setSize(this.container?.clientWidth, this.container?.clientHeight);
    });
  }

  update(scene: THREE.Scene, camera: THREE.Camera) {
    this._labelRenderer?.render(scene, camera);
  }
}

export class BasePoly extends THREE.Mesh {
  ogid: string;
  layerVertices: Vector3D[] = [];
  layerBackVertices: Vector3D[] = [];

  polygon: BasePolygon | null = null;
  isTriangulated: boolean = false;

  constructor(vertices?: Vector3D[]) {
    super();
    this.ogid = getUUID();
    this.polygon = new BasePolygon(this.ogid);
    
    if (vertices) {
      this.polygon.add_vertices(vertices);

      // Triangulate the polygon
      this.polygon?.triangulate();

      const bufFlush = this.polygon?.get_buffer_flush();
      this.addFlushBufferToScene(bufFlush);
    }
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
    const triResult = JSON.parse(this.polygon.new_triangulate());
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

    const material = new THREE.MeshStandardMaterial({ color: 0x3a86ff, side: THREE.DoubleSide });
    this.geometry = geometry;
    this.material = material;
  }

  extrude(height: number) {
    if (!this.polygon) return;
    const extruded_buff = this.polygon.extrude_by_height(height);
    this.generateExtrudedGeometry(extruded_buff);
  }

  generateExtrudedGeometry(extruded_buff: string) {
    // THIS WORKS
    const flushBuffer = JSON.parse(extruded_buff);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    geometry.computeVertexNormals();

    const material = new THREE.MeshPhongMaterial({
      color: 0x3a86ff,
    });
    material.side = THREE.DoubleSide;

    this.geometry = geometry;
    this.material = material;
  }
}

interface IBaseCircleOptions {
  radius: number;
  segments: number;
  position: Vector3D;
  startAngle: number;
  endAngle: number;
}

export class CirclePoly extends THREE.Mesh {
  ogid: string;
  polygon: BasePolygon | null = null;
  baseCircle: BaseCircle;
  isExtruded: boolean = false;

  constructor(baseCircle: BaseCircle) {
    super();
    this.ogid = getUUID();

    if (!baseCircle.circleArc) {
      throw new Error("CircleArc is not defined");
    }
    // baseCircle.nodeChild = this;
    baseCircle.nodeOperation = "polygon";
    this.baseCircle = baseCircle;

    this.generateGeometry();
    this.addFlushBufferToScene();
  }

  update() {
    this.geometry.dispose();

    this.polygon?.clear_vertices();
    this.polygon?.add_vertices(this.baseCircle.circleArc.get_raw_points());
    
    this.generateGeometry();
    this.addFlushBufferToScene();
  }

  generateGeometry() {
    if (!this.baseCircle.circleArc) return;
    this.polygon = BasePolygon.new_with_circle(this.baseCircle.circleArc.clone());
  }

  addFlushBufferToScene() {
    if (!this.polygon) return;
    const bufFlush = this.polygon.get_buffer_flush();
    const flushBuffer = JSON.parse(bufFlush);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    
    // TODO: Do this using a set method, poly.visualizeTriangles = true
    // different colors for each triangle in the polygon dont interolate
    // const colors = new Float32Array(flushBuffer.length);
    // for (let i = 0; i < colors.length; i += 9) {
    //   const r = Math.random();
    //   const g = Math.random();
    //   const b = Math.random();
    //   colors[i] = r;
    //   colors[i + 1] = g;
    //   colors[i + 2] = b;
    //   colors[i + 3] = r;
    //   colors[i + 4] = g;
    //   colors[i + 5] = b;
    //   colors[i + 6] = r;
    //   colors[i + 7] = g;
    //   colors[i + 8] = b;
    // }

    // geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));

    const material = new THREE.MeshStandardMaterial( {
        color: 0x4460FF,
        side: THREE.DoubleSide,
        transparent: true,
        opacity: 0.8
    });

    this.geometry = geometry;
    this.material = material;
  }

  clearGeometry() {
    this.geometry.dispose();
  }

  extrude(height: number) {
    if (!this.polygon) return;
    const extruded_buff = this.polygon.extrude_by_height(height);
    this.isExtruded = true;
    
    this.generateExtrudedGeometry(extruded_buff);
  }
 
  getOutline(type: OUTLINE_TYPE) {
    if (!this.polygon) return;
    const outlines = this.polygon.get_outlines();
    const outlineBuffer = JSON.parse(outlines);

    // TODO: move this logic to Kernel
    const faces = [];
    for (const data of outlineBuffer) {
      const vertices = [];
      for (const vertex of data) {
        const x_float = type === "side" ? 0 : parseFloat(vertex.x.toFixed(5));
        const y_float = type === "top" ? 0 : parseFloat(vertex.y.toFixed(5));
        const z_float = type === "front" ? 0 : parseFloat(vertex.z.toFixed(5));
        vertices.push(new THREE.Vector3(x_float, y_float, z_float));
      }
      faces.push(vertices);
    }

    const clonedFaces = faces.map((face) => {
      return face.map((vertex) => {
        return new THREE.Vector3(vertex.x, vertex.y, vertex.z);
      });
    }
    );

    // remove duplicates inside the faces
    const uniqueFaces = clonedFaces.map((face) => {
      return face.filter((vertex, index, self) =>
        index === self.findIndex((v) => (
          v.x === vertex.x && v.y === vertex.y && v.z === vertex.z
        ))
      );
    });

    // Picking unique vertices from all faces
    const uniqueVertices = [];
    const vertexSet = new Set();
    for (const face of uniqueFaces) {
      for (const vertex of face) {
        const key = `${vertex.x},${vertex.y},${vertex.z}`;
        if (!vertexSet.has(key)) {
          vertexSet.add(key);
          uniqueVertices.push(vertex);
        }
      }
    }
    
    // arrange the vertices in a clockwise manner
    const center = new THREE.Vector3();
    for (const vertex of uniqueVertices) {
      center.add(vertex);
    }
    center.divideScalar(uniqueVertices.length);
    uniqueVertices.sort((a, b) => {
      if (type === "side") {
        const angleA = Math.atan2(a.y - center.y, a.z - center.z);
        const angleB = Math.atan2(b.y - center.y, b.z - center.z);
        return angleA - angleB;
      } else if (type === "top") {
        const angleA = Math.atan2(a.x - center.x, a.z - center.z);
        const angleB = Math.atan2(b.x - center.x, b.z - center.z);
        return angleA - angleB;
      }
      const angleA = Math.atan2(a.x - center.x, a.y - center.y);
      const angleB = Math.atan2(b.x - center.x, b.y - center.y);
      return angleA - angleB;
    }
    );

    // merge collinear vertices
    const mergedVertices = [];
    for (let i = 0; i < uniqueVertices.length; i++) {
      const current = uniqueVertices[i];
      const next = uniqueVertices[(i + 1) % uniqueVertices.length];
      const prev = uniqueVertices[(i - 1 + uniqueVertices.length) % uniqueVertices.length];

      const v1 = new THREE.Vector3().subVectors(current, prev);
      const v2 = new THREE.Vector3().subVectors(next, current);

      if (v1.angleTo(v2) > 0.01) {
        mergedVertices.push(current);
      }
    }

    mergedVertices.push(mergedVertices[0]);
    // TODO: move logic until here to Kernel

    // Create a new geometry with the merged vertices
    const mergedGeometry = new THREE.BufferGeometry().setFromPoints(mergedVertices);
    const mergedMaterial = new THREE.MeshBasicMaterial({ color: 0x000000, side: THREE.DoubleSide });
    const mergedMesh = new THREE.Line(mergedGeometry, mergedMaterial);
    return mergedMesh;
  }

  generateExtrudedGeometry(extruded_buff: string) {
    // THIS WORKS
    const flushBuffer = JSON.parse(extruded_buff);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    geometry.computeVertexNormals();

    // To Test If Triangulation is working
    // const colors = new Float32Array(flushBuffer.length);
    // for (let i = 0; i < colors.length; i += 9) {
    //   const r = Math.random();
    //   const g = Math.random();
    //   const b = Math.random();
    //   colors[i] = r;
    //   colors[i + 1] = g;
    //   colors[i + 2] = b;
    //   colors[i + 3] = r;
    //   colors[i + 4] = g;
    //   colors[i + 5] = b;
    //   colors[i + 6] = r;
    //   colors[i + 7] = g;
    //   colors[i + 8] = b;
    // }

    // geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));

    // const material = new THREE.MeshPhongMaterial( {
    //     color: 0xffffff,
    //     flatShading: true,
    //     vertexColors: true,
    //     shininess: 0,
    //     side: THREE.DoubleSide
    // });

    const material = new THREE.MeshPhongMaterial({
      color: 0x3a86ff,
    });
    material.side = THREE.DoubleSide;
    
    this.geometry = geometry;
    this.material = material;
  }
}

export class RectanglePoly extends THREE.Mesh {
  ogid: string;
  polygon: BasePolygon | null = null;
  baseRectangle: Rectangle;
  isExtruded: boolean = false;
  constructor(baseRectangle: Rectangle) {
    super();
    this.ogid = getUUID();
   
    if (!baseRectangle.polyLineRectangle) {
      throw new Error("BaseRectangle is not defined");
    }
    // baseRectangle.nodeChild = this;
    baseRectangle.nodeOperation = "polygon";
    this.baseRectangle = baseRectangle;
    
    this.generateGeometry();
    this.addFlushBufferToScene();
  }

  update() {
    this.geometry.dispose();
    this.polygon?.clear_vertices();
    this.polygon?.add_vertices(this.baseRectangle.polyLineRectangle.get_raw_points());
    this.generateGeometry();
    this.addFlushBufferToScene();
  }
  generateGeometry() {
    if (!this.baseRectangle.polyLineRectangle) return;
    this.polygon = BasePolygon.new_with_rectangle(this.baseRectangle.polyLineRectangle.clone());
  }

  addFlushBufferToScene() {
    if (!this.polygon) return;
    const bufFlush = this.polygon.get_buffer_flush();
    const flushBuffer = JSON.parse(bufFlush);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    const material = new THREE.MeshStandardMaterial({ color: 0x3a86ff, transparent: true, opacity: 0.5 });
    this.geometry = geometry;
    this.material = material;
  }

  clearGeometry() {
    this.geometry.dispose();
  }

  extrude(height: number) {
    if (!this.polygon) return;
    const extruded_buff = this.polygon.extrude_by_height(height);
    this.isExtruded = true;
    this.generateExtrudedGeometry(extruded_buff);
  }

  generateExtrudedGeometry(extruded_buff: string) {
    // THIS WORKS
    const flushBuffer = JSON.parse(extruded_buff);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    geometry.computeVertexNormals();

    // const colors = new Float32Array(flushBuffer.length);
    // for (let i = 0; i < colors.length; i += 9) {
    //   const r = Math.random();
    //   const g = Math.random();
    //   const b = Math.random();
    //   colors[i] = r;
    //   colors[i + 1] = g;
    //   colors[i + 2] = b;
    //   colors[i + 3] = r;
    //   colors[i + 4] = g;
    //   colors[i + 5] = b;
    //   colors[i + 6] = r;
    //   colors[i + 7] = g;
    //   colors[i + 8] = b;
    // }
    // geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    // const material = new THREE.MeshPhongMaterial( {
    //     color: 0xffffff,
    //     flatShading: true,
    //     vertexColors: true,
    //     shininess: 0,
    //     side: THREE.DoubleSide
    // });

    const material = new THREE.MeshPhongMaterial({
      color: 0x3a86ff,
    });
    material.side = THREE.DoubleSide;

    this.geometry = geometry;
    this.material = material;
  }

  getOutline(type: OUTLINE_TYPE) {
    if (!this.polygon) return;
    const outlines = this.polygon.get_outlines();
    const outlineBuffer = JSON.parse(outlines);

    // TODO: move this logic to Kernel
    const faces = [];
    for (const data of outlineBuffer) {
      const vertices = [];
      for (const vertex of data) {
        const x_float = type === "side" ? 0 : parseFloat(vertex.x.toFixed(5));
        const y_float = type === "top" ? 0 : parseFloat(vertex.y.toFixed(5));
        const z_float = type === "front" ? 0 : parseFloat(vertex.z.toFixed(5));
        vertices.push(new THREE.Vector3(x_float, y_float, z_float));
      }
      faces.push(vertices);
    }

    const clonedFaces = faces.map((face) => {
      return face.map((vertex) => {
        return new THREE.Vector3(vertex.x, vertex.y, vertex.z);
      });
    }
    );

    // remove duplicates inside the faces
    const uniqueFaces = clonedFaces.map((face) => {
      return face.filter((vertex, index, self) =>
        index === self.findIndex((v) => (
          v.x === vertex.x && v.y === vertex.y && v.z === vertex.z
        ))
      );
    });

    // Picking unique vertices from all faces
    const uniqueVertices = [];
    const vertexSet = new Set();
    for (const face of uniqueFaces) {
      for (const vertex of face) {
        const key = `${vertex.x},${vertex.y},${vertex.z}`;
        if (!vertexSet.has(key)) {
          vertexSet.add(key);
          uniqueVertices.push(vertex);
        }
      }
    }
    
    // arrange the vertices in a clockwise manner
    const center = new THREE.Vector3();
    for (const vertex of uniqueVertices) {
      center.add(vertex);
    }
    center.divideScalar(uniqueVertices.length);
    uniqueVertices.sort((a, b) => {
      if (type === "side") {
        const angleA = Math.atan2(a.y - center.y, a.z - center.z);
        const angleB = Math.atan2(b.y - center.y, b.z - center.z);
        return angleA - angleB;
      } else if (type === "top") {
        const angleA = Math.atan2(a.x - center.x, a.z - center.z);
        const angleB = Math.atan2(b.x - center.x, b.z - center.z);
        return angleA - angleB;
      }
      const angleA = Math.atan2(a.x - center.x, a.y - center.y);
      const angleB = Math.atan2(b.x - center.x, b.y - center.y);
      return angleA - angleB;
    }
    );

    // merge collinear vertices
    const mergedVertices = [];
    for (let i = 0; i < uniqueVertices.length; i++) {
      const current = uniqueVertices[i];
      const next = uniqueVertices[(i + 1) % uniqueVertices.length];
      const prev = uniqueVertices[(i - 1 + uniqueVertices.length) % uniqueVertices.length];

      const v1 = new THREE.Vector3().subVectors(current, prev);
      const v2 = new THREE.Vector3().subVectors(next, current);

      if (v1.angleTo(v2) > 0.01) {
        mergedVertices.push(current);
      }
    }

    mergedVertices.push(mergedVertices[0]);
    // TODO: move logic until here to Kernel

    // Create a new geometry with the merged vertices
    const mergedGeometry = new THREE.BufferGeometry().setFromPoints(mergedVertices);
    const mergedMaterial = new THREE.MeshBasicMaterial({ color: 0x000000, side: THREE.DoubleSide });
    const mergedMesh = new THREE.Line(mergedGeometry, mergedMaterial);
    return mergedMesh;
  }
}

/**
 * Base Flat Mesh
 */
export class FlatMesh extends THREE.Mesh {
  constructor(vertices: Vector3D[]) {
    super();
    const baseMesh = new BaseFlatMesh(getUUID());
    baseMesh.add_vertices(vertices);
    const bufFlush = baseMesh.triangulate();
    const flushBuffer = JSON.parse(bufFlush);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    const material = new THREE.MeshStandardMaterial({ color: 0xff0000, transparent: true, opacity: 0.5, side: THREE.DoubleSide });
    this.geometry = geometry;
    this.material = material;
  }
}


export {
  Vector3D,
  SpotLabel,
}

export * from './src/primitives/';