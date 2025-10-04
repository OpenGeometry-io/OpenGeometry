import { OGArc, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
// import { IArcOptions } from "../base-types";

export interface IArcOptions {
  center: Vector3;
  radius: number;
  startAngle: number;
  endAngle: number;
  segments: number;
}

export class Arc extends THREE.Line {
  ogid: string;
  options: IArcOptions;
  
  private arc: OGArc;

  // TODO: Create local properties for all Primitive classes
  #color: number = 0x00ff00;

  set color(color: number) {
    this.#color = color;
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: IArcOptions) {
    super();
    this.ogid = getUUID();
  
    this.arc = new OGArc(this.ogid);

    this.options = options || {
      center: new Vector3(0, 0, 0),
      radius: 3.5,
      startAngle: 0,
      endAngle: Math.PI * 2,
      segments: 4,
    };

    this.setConfig(this.options);
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Circle Arc");
    }
  }

  setConfig(options: IArcOptions) {
    this.validateOptions();

    const { center, radius, segments, startAngle, endAngle } = options;
    this.arc.set_config(
      center.clone(),
      radius,
      startAngle,
      endAngle,
      segments
    );

    // If Config changes we need to regenerate geometry
    // TODO: can geometry generation be made optional
    this.generateGeometry();
  }

  private generateGeometry() {
    if (this.geometry) {
      this.geometry.dispose();
    }

    this.arc.generate_geometry();
    const geometryData = this.arc.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);
    // console.log(bufferData);

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    this.geometry = geometry;
    this.material = new THREE.LineBasicMaterial({ color: this.#color });
  }

  discardGeoemtry() {
    this.geometry.dispose();
  }
}
