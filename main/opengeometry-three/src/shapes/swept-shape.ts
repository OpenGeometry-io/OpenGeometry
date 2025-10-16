import * as THREE from "three";
import { OGSweptShape, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { getUUID } from "../utils/randomizer";

interface ISweptShapeOptions {
  ogid?: string;
  color?: number;
  profileVertices?: Vector3[];
  pathPoints?: Vector3[];
}

/**
 * SweptShape class that extends THREE.Mesh
 * Creates a 3D shape by sweeping a 2D profile along a 3D path
 */
export class SweptShape extends THREE.Mesh {
  ogid: string;
  options: ISweptShapeOptions = {};
  sweptShape: OGSweptShape;

  #color: number = 0x3a86ff;
  #outlineMesh: THREE.Line | null = null;

  transformationMatrix: THREE.Matrix4 = new THREE.Matrix4();

  get color(): number {
    return this.#color;
  }

  set color(color: number) {
    this.#color = color;
    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: ISweptShapeOptions) {
    super();

    this.ogid = options?.ogid ?? getUUID();
    this.sweptShape = new OGSweptShape(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    if (this.options.color) {
      this.color = this.options.color;
    }

    this.setConfig(this.options);
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for SweptShape");
    }
  }

  setConfig(options?: ISweptShapeOptions) {
    this.validateOptions();

    if (options) {
      this.options = { ...this.options, ...options };
    }

    const { profileVertices, pathPoints } = this.options;

    if (profileVertices && profileVertices.length >= 3 && pathPoints && pathPoints.length >= 2) {
      this.sweptShape.sweep_along_points(profileVertices, pathPoints);
      this.generateGeometry();
    }
  }

  /**
   * Generate geometry from the swept shape
   */
  generateGeometry() {
    this.disposeGeometry();

    this.sweptShape.generate_geometry();
    const geometryData = this.sweptShape.get_geometry_serialized();
    
    if (!geometryData) return;

    const vertexBuffer = JSON.parse(geometryData);
    const geometry = new THREE.BufferGeometry();
    
    geometry.setAttribute(
      "position", 
      new THREE.BufferAttribute(new Float32Array(vertexBuffer), 3)
    );
    
    geometry.computeVertexNormals();

    const material = new THREE.MeshStandardMaterial({
      color: this.#color,
      side: THREE.DoubleSide,
    });

    this.geometry = geometry;
    this.material = material;
  }

  /**
   * Set the profile vertices for the sweep operation
   */
  setProfileVertices(vertices: Vector3[]) {
    this.options.profileVertices = vertices;
    this.setConfig();
  }

  /**
   * Set the path points for the sweep operation
   */
  setPathPoints(points: Vector3[]) {
    this.options.pathPoints = points;
    this.setConfig();
  }

  /**
   * Create a swept shape from a profile and path
   */
  sweepAlongPath(profileVertices: Vector3[], pathPoints: Vector3[]) {
    this.options.profileVertices = profileVertices;
    this.options.pathPoints = pathPoints;
    this.setConfig();
  }

  /**
   * Get the B-Rep data from the swept shape
   */
  getBrepData() {
    if (!this.sweptShape) return null;
    const brepData = this.sweptShape.get_brep_serialized();
    return brepData;
  }

  /**
   * Clear the swept shape data
   */
  clearSweptShape() {
    if (this.sweptShape) {
      this.sweptShape.clear();
    }
    this.disposeGeometry();
  }

  /**
   * Dispose of geometry and material resources
   */
  disposeGeometry() {
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

  /**
   * Enable/disable outline rendering
   */
  set outline(enable: boolean) {
    if (enable && !this.#outlineMesh) {
      // Create outline mesh from geometry edges
      const edges = new THREE.EdgesGeometry(this.geometry);
      const lineMaterial = new THREE.LineBasicMaterial({ 
        color: 0x000000,
        linewidth: 2 
      });
      this.#outlineMesh = new THREE.LineSegments(edges, lineMaterial);
      this.add(this.#outlineMesh);
    } else if (!enable && this.#outlineMesh) {
      this.remove(this.#outlineMesh);
      this.#outlineMesh.geometry.dispose();
      if (this.#outlineMesh.material instanceof THREE.Material) {
        this.#outlineMesh.material.dispose();
      }
      this.#outlineMesh = null;
    }
  }

  get outline(): boolean {
    return this.#outlineMesh !== null;
  }

  get outlineMesh(): THREE.Line | null {
    return this.#outlineMesh;
  }

  /**
   * Dispose of all resources when the object is destroyed
   */
  dispose() {
    this.disposeGeometry();
    this.clearSweptShape();
  }
}