import { OGArc } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import { IArcOptions } from "../base-types";

// TODO: What if user wants infintely smooth circle
export class Arc extends THREE.Line {
  ogid: string;
  arc: OGArc;
  
  options: IArcOptions
  // nodeChild: CirclePoly | null = null;
  nodeOperation: String = "none";

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
    const { radius, segments, startAngle, endAngle } = this.options;
    this.arc.set_config(
      // position,
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
