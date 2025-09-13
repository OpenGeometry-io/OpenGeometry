import { OGLine, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

interface ILineOptions {
  start: Vector3;
  end: Vector3;
}

/**
 * Simple Line defined by Two Points
 */
export class Line extends THREE.Line {
  ogid: string;
  options: ILineOptions;
  
  private line: OGLine;

  set color(color: number) {
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }
  constructor(options: ILineOptions) {
    super();
    this.ogid = getUUID();
    this.options = options;

    this.line = new OGLine(this.ogid);

    this.setConfig();
    this.generateGeometry();
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Line");
    }
  }

  setConfig() {
    this.validateOptions();

    const { start, end } = this.options;
    this.line.set_config(start.clone(), end.clone());
  }

  private generateGeometry() {
    this.line.generate_geometry();
    const geometryData = this.line.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);
    
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    this.geometry = geometry;
    this.material = new THREE.LineBasicMaterial({ color: 0x00ff00 });
  }
}
