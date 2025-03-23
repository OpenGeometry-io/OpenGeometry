import init, { 
  Vector3D, 
  BasePolygon,
  BaseFlatMesh,
  CircleArc,
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

interface IBaseCircleOptions {
  radius: number;
  segments: number;
  position: Vector3D;
  startAngle: number;
  endAngle: number;
}
export class BaseCircle extends THREE.Line {
  ogid: string;
  circleArc: CircleArc;
  options: IBaseCircleOptions;
  nodeChild: CirclePoly | null = null;
  nodeOperation: String = "none";

  constructor(options: IBaseCircleOptions) {
    super();
    this.ogid = getUUID();
    this.options = options;
    this.circleArc = new CircleArc(this.ogid);

    this.setConfig();
    this.generateGeometry();
  }

  setConfig() {
    const { radius, segments, position, startAngle, endAngle } = this.options;
    this.circleArc.set_config(
      position,
      radius,
      startAngle,
      endAngle,
      segments
    );
  }

  generateGeometry() {
    this.circleArc.generate_points();
    const bufRaw = this.circleArc.get_points();
    const bufFlush = JSON.parse(bufRaw);
    console.log(bufFlush);
    const line = new THREE.BufferGeometry().setFromPoints(bufFlush);
    const material = new THREE.LineBasicMaterial({ color: 0x00ff00 });
    this.geometry = line;
    this.material = material;
  }

  discardGeoemtry() {
    this.geometry.dispose();
  }
  
  set radius(radius: number) {
    this.options.radius = radius;
    this.circleArc.update_radius(radius);

    this.generateGeometry();
    if (this.nodeChild) {
      this.nodeChild.update();
    }
  }
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
    baseCircle.nodeChild = this;
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
    console.log(flushBuffer);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));
    
    // different colors for each triangle in the polygon dont interolate
    const colors = new Float32Array(flushBuffer.length);
    for (let i = 0; i < colors.length; i += 9) {
      const r = Math.random();
      const g = Math.random();
      const b = Math.random();
      colors[i] = r;
      colors[i + 1] = g;
      colors[i + 2] = b;
      colors[i + 3] = r;
      colors[i + 4] = g;
      colors[i + 5] = b;
      colors[i + 6] = r;
      colors[i + 7] = g;
      colors[i + 8] = b;
    }

    geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));

    const material = new THREE.MeshPhongMaterial( {
        color: 0xffffff,
        flatShading: true,
        vertexColors: true,
        shininess: 0,
        side: THREE.DoubleSide
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
    console.log(JSON.parse(extruded_buff));
    this.isExtruded = true;
    
    this.generateExtrudedGeometry(extruded_buff);
  }

  generateExtrudedGeometry(extruded_buff: string) {
    const flushBuffer = JSON.parse(extruded_buff);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(flushBuffer), 3));

    const colors = new Float32Array(flushBuffer.length);
    for (let i = 0; i < colors.length; i += 9) {
      const r = Math.random();
      const g = Math.random();
      const b = Math.random();
      colors[i] = r;
      colors[i + 1] = g;
      colors[i + 2] = b;
      colors[i + 3] = r;
      colors[i + 4] = g;
      colors[i + 5] = b;
      colors[i + 6] = r;
      colors[i + 7] = g;
      colors[i + 8] = b;
    }

    geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));

    const material = new THREE.MeshPhongMaterial( {
        color: 0xffffff,
        flatShading: true,
        vertexColors: true,
        shininess: 0,
        side: THREE.DoubleSide
    });
    
    this.geometry = geometry;
    this.material = material;
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
