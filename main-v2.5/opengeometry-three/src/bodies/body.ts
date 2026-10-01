import * as THREE from 'three';
import type { Brep } from '../dto/brep.js';
import type { NodeInfo } from '../dto/node-info.js';
import type { Placement } from '../dto/placement.js';
import { OGError } from '../errors.js';
import { call } from '../kernel/kernel-session.js';
import { enterLimbo, revive } from '../marks/limbo.js';
import { flush, wanted } from '../rendering/geometry/geometry-scheduler.js';
import { noteDisplayed } from '../rendering/geometry/render-pass.js';
import { observeCamera } from '../rendering/lod/camera-observer.js';
import { overrideChanged } from '../rendering/lod/lod-controller.js';
import {
  lineMaterial, releaseLine, releaseSurface, surfaceMaterial, type MaterialAppearance,
} from '../rendering/materials/material-pool.js';
import type { GeometryRecord } from '../rendering/records/geometry-record.js';
import { bodyKey, runtime } from '../runtime/runtime-state.js';
import { encode, operationParams, polylinePoints, scope } from '../world-graph/codec.js';
import { node, worldGraph } from '../world-graph/world-graph-client.js';
import type { Appearance, BodyOptions } from './body-options.js';
import { derivePose, keepPose, poseMemory } from './body-placement.js';
import { register, unregister } from './body-registry.js';
import { DisplayClone } from './display-clone.js';
import {
  addChild, checkNode, getBounds, getBrep, getChildren, getInstanceCount, getParent, getPlacement, getWorldPlacement,
  localBounds, removeChild,
} from './node-methods.js';
import type { SystemAssembly } from './system-assembly.js';

function singleMaterial(material: THREE.Material | THREE.Material[], label: string): THREE.Material {
  if (Array.isArray(material)) throw new OGError('InvalidOperand', label, 'display material must be a single material');
  return material;
}

function lineStyle(bodyType: 'Wire' | 'Solid', appearance: Appearance): MaterialAppearance {
  return { color: bodyType === 'Wire' ? appearance.color : 0x243040, opacity: appearance.opacity };
}

export abstract class Body extends THREE.Group {
  readonly ogId: string;
  readonly handle: number;
  readonly generation: number;
  readonly surface: THREE.Mesh;
  readonly outline: THREE.LineSegments;
  readonly bodyType: 'Wire' | 'Solid';
  appearance: Appearance;
  lastInfo: NodeInfo;
  record?: GeometryRecord;
  inLimbo = false;
  private surfaceKey: string;
  private lineKey: string;
  private readonly epoch = runtime().epoch;
  private readonly pose = poseMemory();

  protected constructor(ogId: string, bodyType: 'Wire' | 'Solid', options: BodyOptions) {
    super();
    this.ogId = ogId;
    this.lastInfo = node(ogId, `${bodyType}.constructor`);
    this.handle = this.lastInfo.handle;
    this.generation = this.lastInfo.generation;
    this.bodyType = bodyType;
    this.appearance = {
      color: bodyType === 'Wire' ? 0x2563eb : 0x6699dd, opacity: 1, outline: bodyType === 'Wire', ...options.appearance,
    };
    this.matrixAutoUpdate = false;
    this.matrixWorldAutoUpdate = false;
    const { shapeId, shapeRevision } = this.lastInfo;
    const placeholder = shapeId && shapeRevision !== null
      ? runtime().records.placeholder(shapeId, shapeRevision, () => localBounds(`${bodyType}.constructor`, ogId))
      : undefined;
    const surfaceStyle = surfaceMaterial(this.appearance);
    this.surfaceKey = surfaceStyle.key;
    this.surface = new THREE.Mesh(placeholder ?? new THREE.BufferGeometry(), surfaceStyle.material);
    this.surface.visible = Boolean(placeholder) && bodyType === 'Solid';
    const outlineStyle = lineMaterial(lineStyle(bodyType, this.appearance));
    this.lineKey = outlineStyle.key;
    this.outline = new THREE.LineSegments(placeholder ?? new THREE.BufferGeometry(), outlineStyle.material);
    this.outline.visible = Boolean(placeholder) && this.appearance.outline;
    this.add(this.surface, this.outline);
    this.surface.onBeforeRender = (renderer, _scene, camera) => { observeCamera(renderer, camera); };
    this.outline.onBeforeRender = (renderer, _scene, camera) => { observeCamera(renderer, camera); };
    register(this);
    this.applyWorldMatrix(call(`${bodyType}.constructor`, () => worldGraph().worldMatrix(ogId)));
  }

  protected check(call: string): void { checkNode(call, 'body', this.epoch, this.handle, this.generation); }

  applyWorldMatrix(values: ArrayLike<number>): void {
    this.matrixWorld.fromArray(values);
    derivePose(this.pose, this);
    super.updateMatrixWorld(true);
  }

  override updateMatrixWorld(force?: boolean): void {
    keepPose(this.pose, this, this.ogId);
    if (this.visible) noteDisplayed(this);
    super.updateMatrixWorld(force);
  }

  swapReadyRecord(): void {
    if (!this.visible || !this.lastInfo.shapeId || this.lastInfo.shapeRevision === null) return;
    const bucket = wanted(this);
    if (bucket === undefined) return;
    const record = runtime().records.get(this.lastInfo.shapeId, this.lastInfo.shapeRevision, bucket);
    if (record) this.install(record);
  }

  install(record: GeometryRecord): void {
    if (this.record === record) return;
    const pool = runtime().records;
    if (this.record) pool.release(this.record);
    this.record = record;
    pool.retain(record);
    const held = pool.releasePlaceholder(this.outline.geometry);
    if (record.surface) this.surface.geometry = record.surface;
    else if (held) this.surface.geometry = new THREE.BufferGeometry();
    this.outline.geometry = record.outline;
    this.surface.position.copy(record.origin);
    this.outline.position.copy(record.origin);
    this.surface.visible = this.bodyType === 'Solid';
    this.outline.visible = this.appearance.outline;
  }

  setAppearance(partial: Partial<Appearance>): void {
    this.check(`${this.bodyType}.setAppearance`);
    const previous = this.appearance;
    this.appearance = { ...previous, ...partial };
    if (previous.color !== this.appearance.color || previous.opacity !== this.appearance.opacity) {
      const surfaceStyle = surfaceMaterial(this.appearance);
      const outlineStyle = lineMaterial(lineStyle(this.bodyType, this.appearance));
      releaseSurface(this.surfaceKey);
      releaseLine(this.lineKey);
      this.surfaceKey = surfaceStyle.key;
      this.lineKey = outlineStyle.key;
      this.surface.material = surfaceStyle.material;
      this.outline.material = outlineStyle.material;
    }
    this.outline.visible = this.appearance.outline;
    const shapeId = this.lastInfo.shapeId;
    if (shapeId && previous.deflection !== this.appearance.deflection) overrideChanged(runtime(), shapeId);
  }

  transform(kind: string, params: Record<string, unknown>): void {
    this.check(`${this.bodyType}.transform`);
    call(`${this.bodyType}.transform`, () => worldGraph().transform(this.ogId, encode({ kind, ...params })));
  }

  getPlacement(): Placement {
    this.check(`${this.bodyType}.getPlacement`);
    return getPlacement(this.bodyType, this.ogId);
  }

  getWorldPlacement(): Placement {
    this.check(`${this.bodyType}.getWorldPlacement`);
    return getWorldPlacement(this.bodyType, this.ogId);
  }

  addChild(children: (Body | SystemAssembly)[], options: { keepWorld?: boolean } = {}): void {
    this.check(`${this.bodyType}.addChild`);
    addChild(this.bodyType, this.ogId, children, Boolean(options.keepWorld));
  }

  removeChild(child: Body | SystemAssembly, options: { keepWorld?: boolean } = {}): void {
    this.check(`${this.bodyType}.removeChild`);
    removeChild(this.bodyType, this.ogId, child.ogId, Boolean(options.keepWorld));
  }

  getChildren(): string[] { this.check(`${this.bodyType}.getChildren`); return getChildren(this.bodyType, this.ogId); }

  getParent(): string | null { this.check(`${this.bodyType}.getParent`); return getParent(this.bodyType, this.ogId); }

  getBounds(): [number, number, number, number, number, number] | null {
    this.check(`${this.bodyType}.getBounds`);
    return getBounds(this.bodyType, this.ogId);
  }

  getBrep(): Brep { this.check(`${this.bodyType}.getBrep`); return getBrep(this.bodyType, this.ogId); }

  getInstanceCount(): number {
    this.check(`${this.bodyType}.getInstanceCount`);
    return getInstanceCount(this.bodyType, this.ogId);
  }

  makeUnique(): void {
    this.check(`${this.bodyType}.makeUnique`);
    call(`${this.bodyType}.makeUnique`, () => worldGraph().makeUnique(this.ogId));
    flush();
  }

  rebuild(kind: string, params: Record<string, unknown>, options: { instances?: 'all' } = {}): void {
    this.check(`${this.bodyType}.rebuild`);
    const graph = worldGraph();
    const label = `${this.bodyType}.rebuild`;
    if (kind === 'Polyline') {
      const points = polylinePoints(params, label);
      call(label, () => graph.rebuildPolyline(this.ogId, points, Boolean(params['closed']), scope(options)));
    } else if (kind === 'Extrude' || kind === 'Sweep') {
      const operation = encode(operationParams(kind, params, label));
      call(label, () => graph.rebuildOperation(this.ogId, operation, scope(options)));
    } else {
      call(label, () => graph.rebuildPrimitive(this.ogId, encode({ kind, ...params }), scope(options)));
    }
    flush();
  }

  hideForDispose(): void { enterLimbo(this); }

  reviveIfRestored(): void { revive(this); }

  finalize(): void {
    const state = runtime();
    if (this.epoch !== state.epoch || state.bodies.get(bodyKey(this.handle, this.generation)) !== this) return;
    unregister(this);
    this.inLimbo = false;
    delete this.record;
    state.records.releasePlaceholder(this.outline.geometry);
    this.surface.geometry = new THREE.BufferGeometry();
    this.outline.geometry = new THREE.BufferGeometry();
    releaseSurface(this.surfaceKey);
    releaseLine(this.lineKey);
  }

  dispose(): void {
    this.check(`${this.bodyType}.dispose`);
    call(`${this.bodyType}.dispose`, () => worldGraph().dispose(this.ogId));
    this.hideForDispose();
    flush();
  }

  override clone(): this {
    const label = `${this.bodyType}.clone`;
    const surface = this.record ? this.surface.geometry : new THREE.BufferGeometry();
    const lines = this.record ? this.outline.geometry : new THREE.BufferGeometry();
    const mesh = new THREE.Mesh(surface, singleMaterial(this.surface.material, label).clone());
    const outline = new THREE.LineSegments(lines, singleMaterial(this.outline.material, label).clone());
    mesh.position.copy(this.surface.position);
    outline.position.copy(this.outline.position);
    const display = new DisplayClone(this.record, mesh, outline);
    display.matrix.copy(this.matrixWorld);
    display.matrix.decompose(display.position, display.quaternion, display.scale);
    display.matrixWorld.copy(this.matrixWorld);
    return display as unknown as this;
  }

  override copy(): this {
    throw new OGError('InvalidOperand', `${this.bodyType}.copy`, 'Use duplicate() for an independent body');
  }
}
