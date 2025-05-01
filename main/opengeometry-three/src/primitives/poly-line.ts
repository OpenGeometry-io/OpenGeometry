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

  private polyline: OGPolyLine;

  constructor(points: Vector3D[] = []) {
    super();
    this.ogid = getUUID();
    this.points = points;
    this.polyline = new OGPolyLine(this.ogid);
  
    this.setConfig(points);
    this.generateGeometry();
  }

  setConfig(points: Vector3D[]) {
    if (this.points.length < 2) return;
    this.polyline.set_config(points);
  }

  addPoint(point: Vector3D) {
    this.points.push(point);
    this.polyline.add_point(point);

    if (this.points.length < 2) return;
    this.generateGeometry();
  }

  private clearGeometry() {
    this.geometry.dispose();
  }

  private generateGeometry() {
    this.clearGeometry();
    const buf = this.polyline.get_points();
    const bufFlush = JSON.parse(buf);
    console.log(bufFlush);
    const line = new THREE.BufferGeometry().setFromPoints(bufFlush);
    const material = new THREE.LineBasicMaterial({ color: 0xff0000 });
    this.geometry = line;
    this.material = material;

    this.isClosed = this.polyline.is_closed();
  }
}