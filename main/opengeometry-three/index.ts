import init, { 
  Vector3D, 
  BasePolygon,
  BaseFlatMesh
} from "../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { CSS2DRenderer } from "three/examples/jsm/renderers/CSS2DRenderer.js";
import { getUUID } from "./src/utils/randomizer";
import { Pencil } from "./src/pencil";
import { SpotLabel } from "./src/markup/spotMarker";

export class OpenGeometry {
  protected scene: THREE.Scene | undefined;
  protected container: HTMLElement | undefined;
  private _pencil: Pencil | undefined;
  private _labelRenderer: CSS2DRenderer | undefined;

  constructor(container:HTMLElement, threeScene: THREE.Scene, private camera: THREE.Camera) {
    // this.setup();
    this.scene = threeScene;

    this.container = container;
  }

  async setup() {
    await init();
    this.setuplabelRenderer();

    if (!this.container || !this.scene) return;
    this._pencil = new Pencil(this.container, this.scene, this.camera);

    this.setupEvent();
    console.log("OpenGeometry Kernel 0.0.1");
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
  ogid: number;
  layerVertices: Vector3D[] = [];
  layerBackVertices: Vector3D[] = [];

  polygon: BasePolygon | null = null;
  isTriangulated: boolean = false;

  constructor(vertices?: Vector3D[]) {
    super();
    this.ogid = getUUID();
    console.log("OGID: ", this.ogid);
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

      console.log("Resetting the polygon");
      console.log(this.layerBackVertices);

      for (const vertex of this.layerBackVertices) {
        this.layerVertices.push(vertex.clone());
      }
      console.log(this.layerVertices);
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

      console.log(this.layerBackVertices);
      console.log(bufFlush);
      
      if (!bufFlush) {
        return;
      }
      this.addFlushBufferToScene(bufFlush);

      this.isTriangulated = true;
    }
  }

  addFlushBufferToScene(flush: string) {
    const flushBuffer = JSON.parse(flush);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    const material = new THREE.MeshStandardMaterial({ color: 0x3a86ff, transparent: true, opacity: 0.5, side: THREE.DoubleSide });
    this.geometry = geometry;
    this.material = material;
    // this.geometry.attributes.position.needsUpdate = true;
    // this.geometry.computeVertexNormals();
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