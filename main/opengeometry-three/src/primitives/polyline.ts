import * as THREE from 'three';
import { OGPolyline, Vector3 } from '../../../opengeometry/pkg/opengeometry';
import { getUUID } from "../utils/randomizer";

interface IPolyLineOptions {
  points: Vector3[];
}

export class Polyline extends THREE.Line {
  ogid: string;
  options: IPolyLineOptions = { points: [] };
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

  constructor(options?: IPolyLineOptions) {
    super();
    this.ogid = getUUID();
    this.polyline = new OGPolyline(this.ogid);
  
    if (options) {
      this.options = options;
      this.setConfig();
      this.generateGeometry();
    }
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Polyline");
    }
  }

  setConfig() {
    this.validateOptions();
    const { points } = this.options;
    this.polyline.set_config(points);
  }

  // addMultiplePoints(points: Vector3[]) {
  //   this.points = points;
  //   if (!this.polyline) return;
  //   this.polyline.add_multiple_points(points);
  //   this.generateGeometry();
  // }

  // // TODO: This needs to be improved 
  // translate(translation: Vector3) {
  //   if (!this.polyline) return;
  //   this.polyline.translate(translation);
  //   this.generateGeometry();
  // }

  // set_position(position: Vector3) {
  //   if (!this.polyline) return;
  //   this.polyline.set_position(position);
  // }

  addPoint(point: Vector3) {
    if (!this.polyline) return;

    this.polyline.add_point(point);
    // if (this.points.length < 2) return;
    this.clearGeometry();
    this.generateGeometry();
  }

  /**
   * Every time there are property changes, geometry needs to be discarded and regenerated.
   * This is to ensure that the geometry is always up-to-date with the current state.
   */
  private clearGeometry() {
    this.geometry.dispose();
  }

  saveTransformationToBREP() {
    if (!this.polyline) return;
    // this.polyline.set_transformation_matrix(this.transformationMatrix.toArray());
  }

  private generateGeometry() {
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