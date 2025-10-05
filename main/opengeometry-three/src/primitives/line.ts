import { OGLine, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

export interface ILineOptions {
  ogid?: string;
  start: Vector3;
  end: Vector3;
  color: number;
}

/**
 * Simple Line defined by Two Points
 */
export class Line extends THREE.Line {
  ogid: string;
  options: ILineOptions = {
    start: new Vector3(0, 0, 0.5),
    end: new Vector3(1, 0, 0.5),
    color: 0x000000
  };
  
  private line: OGLine;

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: ILineOptions) {
    super();

    this.ogid = options?.ogid ?? getUUID();
    this.line = new OGLine(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;
    
    this.setConfig(this.options);
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Line");
    }
  }

  setConfig(options: ILineOptions) {
    this.validateOptions();

    const { start, end } = options;
    this.line.set_config(start.clone(), end.clone());

    this.generateGeometry();
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

    this.line.generate_geometry();
    const geometryData = this.line.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);
    
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    this.geometry = geometry;
    this.material = new THREE.LineBasicMaterial({ color: this.options.color });
  }
}
