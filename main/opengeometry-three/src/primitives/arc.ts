import { OGArc, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
// import { IArcOptions } from "../base-types";

interface IArcOptions {
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
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options: IArcOptions) {
    super();
    this.ogid = getUUID();
    this.options = options;
    
    this.arc = new OGArc(this.ogid);

    this.setConfig();
    this.generateGeometry();
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Circle Arc");
    }
  }

  setConfig() {
    this.validateOptions();

    const { center, radius, segments, startAngle, endAngle } = this.options;
    this.arc.set_config(
      center.clone(),
      radius,
      startAngle,
      endAngle,
      segments
    );
  }

  private generateGeometry() {
    this.arc.generate_geometry();
    const geometryData = this.arc.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);

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
