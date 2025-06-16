import * as THREE from 'three';
import { OGPolyLine, Vector3D } from '../../../opengeometry/pkg/opengeometry';
import { getUUID } from "../utils/randomizer";

/**
 * PolyLine defined by multiple points
 */
export class PolyLine extends THREE.Line {
  ogid: string;
  points: Vector3D[] = [];
  isClosed: boolean = false;

  private polyline: OGPolyLine | null = null;

  set color(color: number) {
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(points?: Vector3D[]) {
    super();
    this.ogid = getUUID();
    this.polyline = new OGPolyLine(this.ogid);
  
    if (points) {
      this.addMultiplePoints(points);
    }
  }

  addMultiplePoints(points: Vector3D[]) {
    this.points = points;
    if (!this.polyline) return;
    this.polyline.add_multiple_points(points);
    this.generateGeometry();
  }

  // TODO: This needs to be improved 
  translate(translation: Vector3D) {
    if (!this.polyline) return;
    this.polyline.translate(translation);
    this.generateGeometry();
  }

  set_position(position: Vector3D) {
    if (!this.polyline) return;
    this.polyline.set_position(position);
  }

  addPoint(point: Vector3D) {
    this.points.push(point);
    if (!this.polyline) return;
    this.polyline.add_point(point);
    if (this.points.length < 2) return;
    this.generateGeometry();
  }

  private clearGeometry() {
    this.geometry.dispose();
  }

  private generateGeometry() {
    this.clearGeometry();
    if (!this.polyline) return;
    const buf = this.polyline.get_points();
    const bufFlush = JSON.parse(buf);
    const line = new THREE.BufferGeometry().setFromPoints(bufFlush);
    const material = new THREE.LineBasicMaterial({ color: 0xff0000 });
    this.geometry = line;
    this.material = material;

    this.isClosed = this.polyline.is_closed();
  }

  getBrepData() {
    if (!this.polyline) return null;
    return this.polyline.get_brep_data();
  }

  // TODO: Add proper return type
  createOffset(offset: number) {
    if (!this.polyline) return null;
    const offsetData = this.polyline.get_offset(offset);
    if (!offsetData) return null;

    const data = JSON.parse(offsetData);
    if (!data.treated || data.treated.length === 0) {
      return null;
    }
    return data;
  }

  dispose() {
    console.log("Disposing OG - Polyline");
    this.clearGeometry();
    this.polyline = null;
  }
}