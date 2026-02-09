import { OGLine, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import { LineGeometry } from 'three/examples/jsm/lines/LineGeometry.js';

export interface ILineOptions {
  ogid?: string;
  start: Vector3;
  end: Vector3;
  color: number;
  fatLines?: boolean;
  width?: number;
}

/**
 * Simple Line defined by Two Points
 */
export class Line extends THREE.Line {
  ogid: string;
  options: ILineOptions = {
    start: new Vector3(0, 0, 0.5),
    end: new Vector3(1, 0, 0.5),
    color: 0x000000,
    fatLines: false,
    width: 20
  };

  private line: OGLine;
  private fatLine: Line2 | null = null;

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
    if (this.fatLine && this.fatLine.material instanceof LineMaterial) {
      this.fatLine.material.color.set(color);
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

    this.options = { ...this.options, ...options };

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

    // Handle fat lines
    if (this.options.fatLines) {
      if (!this.fatLine) {
        this.fatLine = new Line2(new LineGeometry(), new LineMaterial({ color: this.options.color, linewidth: this.options.width, resolution: new THREE.Vector2(window.innerWidth, window.innerHeight) }));
        this.add(this.fatLine);
      }

      const positions = [];
      for (let i = 0; i < bufferData.length; i += 3) {
        positions.push(bufferData[i], bufferData[i + 1], bufferData[i + 2]);
      }

      this.fatLine.geometry.setPositions(positions);
      (this.fatLine.material as LineMaterial).color.set(this.options.color);
      (this.fatLine.material as LineMaterial).linewidth = this.options.width ?? 5;
      (this.fatLine.material as LineMaterial).resolution.set(window.innerWidth, window.innerHeight);

      this.fatLine.visible = true;
    } else {
      if (this.fatLine) {
        this.fatLine.visible = false;
      }
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    this.geometry = geometry;
    this.geometry = geometry;
    this.material = new THREE.LineBasicMaterial({ color: this.options.color });

    if (this.options.fatLines) {
      this.material.visible = false;
    } else {
      this.material.visible = true;
    }
  }

  getDXF() {
    const dxfData = this.line.get_dxf_serialized();
    return dxfData;
  }
}
