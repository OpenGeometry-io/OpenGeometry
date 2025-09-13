import * as THREE from 'three';
import { OGLine, Vector3 } from "./../../opengeometry/pkg/opengeometry";

export const OPEN_GEOMETRY_THREE_VERSION = '0.0.1';

export interface OpenGeometryOptions {
  container: HTMLElement;
  scene: THREE.Scene;
  camera: THREE.Camera;
  wasmURL: string;
}

export interface IArcOptions {
  radius: number;
  segments: number;
  // position: Vector3; // TODO: Figure out best way to handle position
  startAngle: number;
  endAngle: number;
}

export interface IRectangeOptions {
  ogid?: string;
  width: number;
  breadth: number;
  center: Vector3;
  color: number;
}
