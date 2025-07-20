import * as THREE from 'three';
import { OGSimpleLine, Vector3 } from "./../../opengeometry/pkg/opengeometry";

export const OPEN_GEOMETRY_THREE_VERSION = '0.0.1';

export interface OpenGeometryOptions {
  container: HTMLElement;
  scene: THREE.Scene;
  camera: THREE.Camera;
  wasmURL: string;
}

export interface IBaseCircleOptions {
  radius: number;
  segments: number;
  position: Vector3;
  startAngle: number;
  endAngle: number;
}

export interface RectangeOptions {
  ogid?: string;

  width: number;
  breadth: number;
  center: Vector3;
  color: number;
}
