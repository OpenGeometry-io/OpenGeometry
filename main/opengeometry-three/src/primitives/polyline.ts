import * as THREE from 'three';
import { OGPolyline, Vector3 } from '../../../opengeometry/pkg/opengeometry';
import { getUUID } from "../utils/randomizer";

export interface IPolylineOptions {
  ogid?: string;
  color: number;
  points: Vector3[];
}

export class Polyline extends THREE.Line {
  ogid: string;
  options: IPolylineOptions = { 
    points: [],
    color: 0x00ff00
  };

  isClosed: boolean = false;

  private polyline: OGPolyline;

  transformationMatrix: THREE.Matrix4 = new THREE.Matrix4();

  // Properties that can be set externally but are not part of the constructor
  // TODO: Consider making these properties part of the constructor options
  #color: number = 0x00ff00;

  set color(color: number) {
    this.#color = color;
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: IPolylineOptions) {
    super();

    this.ogid = options?.ogid ?? getUUID();
    this.polyline = new OGPolyline(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;
    
    this.setConfig(this.options);
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Polyline");
    }
  }

  setConfig(options: IPolylineOptions) {
    this.validateOptions();

    const { points } = options;
    this.polyline.set_config(points);

    this.generateGeometry();
  }

  addPoint(point: Vector3) {
    if (!this.polyline) return;

    this.polyline.add_point(point);
    if (this.options.points.length < 2) return;
    
    const { points } = this.options;
    points.push(point);

    this.setConfig({ ...this.options, points });
  }

  /**
   * Every time there are property changes, geometry needs to be discarded and regenerated.
   * This is to ensure that the geometry is always up-to-date with the current state.
   */
  discardGeometry() {
    this.geometry.dispose();
  }

  private generateGeometry() {
    this.discardGeometry();

    this.polyline.generate_geometry();
    const geometryData = this.polyline.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    this.geometry = geometry;
    this.material = new THREE.LineBasicMaterial({ color: this.#color });
    
    this.isClosed = this.polyline.is_closed();
  }

  // // TODO: Add proper return type
  // createOffset(offset: number) {
  //   if (!this.polyline) return null;
  //   const offsetData = this.polyline.get_offset(offset);
  //   if (!offsetData) return null;

  //   const data = JSON.parse(offsetData);
  //   if (!data.treated || data.treated.length === 0) {
  //     return null;
  //   }
  //   return data;
  // }
}