import { OGSimpleLine, Vector3 } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

/**
 * Simple Line defined by Two Points
 */
export class SimpleLine extends THREE.Line {
  ogid: string;
  points: Vector3[] = [];

  set color(color: number) {
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(
    start: Vector3 = new Vector3(1, 0, 0),
    end: Vector3 = new Vector3(-1, 0, 0)
  ) {
    super();
    this.ogid = getUUID();
    this.points.push(start);
    this.points.push(end);

    this.generateGeometry();
  }

  addPoint(point: Vector3) {
    this.points.push(point);
    if (this.points.length > 2) {
      throw new Error("Simple Line can only have two points, clear points or use PolyLine");
    }

    if (this.points.length < 2) return;
    this.generateGeometry();
  }

  private generateGeometry() {
    const ogLine = new OGSimpleLine(this.ogid);
    ogLine.set_config(this.points[0], this.points[1]);
    const buf = ogLine.get_points();
    const bufFlush = JSON.parse(buf);
    const line = new THREE.BufferGeometry().setFromPoints(bufFlush);
    const material = new THREE.LineBasicMaterial({ color: 0xff0000 });
    this.geometry = line;
    this.material = material;
  }
}
