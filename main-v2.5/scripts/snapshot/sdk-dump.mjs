import { OpenGeometry } from '../../dist/index.js';
import { digest, outcome, outcomeAsync } from './stable-output.mjs';

export function getters(item) {
  const result = {
    ogId: item.ogId,
    handle: item.handle,
    generation: item.generation,
    placement: outcome(() => item.getPlacement()),
    worldPlacement: outcome(() => item.getWorldPlacement()),
    children: outcome(() => item.getChildren()),
    parent: outcome(() => item.getParent()),
    bounds: outcome(() => item.getBounds()),
  };
  if (typeof item.getInstanceCount === 'function') {
    result.bodyType = item.bodyType;
    result.appearance = item.appearance;
    result.instanceCount = outcome(() => item.getInstanceCount());
  }
  if (typeof item.getReport === 'function') result.report = outcome(() => item.getReport());
  return result;
}

export function allGetters(items) {
  return Object.fromEntries(items.map((item) => [item.ogId, getters(item)]));
}

function attributeDigest(attribute) {
  return {
    itemSize: attribute.itemSize,
    count: attribute.count,
    type: attribute.array.constructor.name,
    sha256: digest(attribute.array),
  };
}

function geometryDigest(geometry) {
  const names = Object.keys(geometry.attributes).sort();
  return {
    attributes: Object.fromEntries(names.map((name) => [name, attributeDigest(geometry.attributes[name])])),
    index: geometry.index ? attributeDigest(geometry.index) : null,
  };
}

function objectDigest(object) {
  return {
    visible: object.visible,
    position: object.position.toArray(),
    geometry: geometryDigest(object.geometry),
  };
}

export function geometry(bodies) {
  return Object.fromEntries(bodies.map((body) => [body.ogId, {
    visible: body.visible,
    matrixWorld: [...body.matrixWorld.elements],
    surface: objectDigest(body.surface),
    outline: objectDigest(body.outline),
  }]));
}

export async function exportStep(options) {
  return outcomeAsync(() => OpenGeometry.exportStep(options));
}
