import * as THREE from 'three';

export function worldUnitsPerPixel(
  camera: THREE.Camera,
  bounds: [number, number, number, number, number, number],
  height: number,
): number {
  if (camera instanceof THREE.OrthographicCamera) return (camera.top - camera.bottom) / (camera.zoom * height);
  if (!(camera instanceof THREE.PerspectiveCamera)) return 1 / height;
  const centre = new THREE.Vector3(
    (bounds[0] + bounds[3]) / 2, (bounds[1] + bounds[4]) / 2, (bounds[2] + bounds[5]) / 2,
  );
  centre.applyMatrix4(camera.matrixWorldInverse);
  const extent = Math.hypot(bounds[3] - bounds[0], bounds[4] - bounds[1], bounds[5] - bounds[2]) / 2;
  const depth = Math.max(camera.near, -centre.z - extent);
  return 2 * depth * Math.tan(THREE.MathUtils.degToRad(camera.getEffectiveFOV()) / 2) / height;
}
