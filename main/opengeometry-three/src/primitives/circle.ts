import { CircleArc } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import { IBaseCircleOptions } from "../base-types";

export class BaseCircle extends THREE.Line {
  ogid: string;
  circleArc: CircleArc;
  options: IBaseCircleOptions;
  // nodeChild: CirclePoly | null = null;
  nodeOperation: String = "none";

  set color(color: number) {
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

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
    const material = new THREE.LineBasicMaterial({ color: 0x000000 });
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
    // if (this.nodeChild) {
    //   this.nodeChild.update();
    // }
  }
}