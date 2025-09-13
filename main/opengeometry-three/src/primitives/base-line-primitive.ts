import * as THREE from 'three';

interface BasePrimitiveOptions {
  [key: string]: any;
}

/**
 * Base class for all line-based primitives in OpenGeometry.
 */
export abstract class BaseLinePrimitive extends THREE.Line {  
  abstract setConfig(options: BasePrimitiveOptions): void;
  abstract getConfig(): any;
  abstract generateGeometry(): void;
  abstract discardGeometry(): void;
}
