import * as THREE from 'three';
import { OGSimpleLine, Vector3D } from "./../../opengeometry/pkg/opengeometry";

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
  position: Vector3D;
  startAngle: number;
  endAngle: number;
}

export type RectangeOptions = {
  width: number;
  breadth: number;
  center: Vector3D
}
