import * as THREE from 'three';
import type { NodeInfo } from '../dto/node-info.js';
import type { Placement } from '../dto/placement.js';
import { OGError } from '../errors.js';
import { call } from '../kernel/kernel-session.js';
import { ensureGeometry, flush, wanted } from '../rendering/geometry/geometry-scheduler.js';
import { noteDisplayed } from '../rendering/geometry/render-pass.js';
import { deflectionBucket } from '../rendering/lod/deflection.js';
import { worldUnitsPerPixel } from '../rendering/lod/screen-metrics.js';
import { lineMaterial, releaseLine, releaseSurface, surfaceMaterial } from '../rendering/materials/material-pool.js';
import type { GeometryRecord } from '../rendering/records/geometry-record.js';
import { runtime } from '../runtime/runtime-state.js';
import { encode, operationParams, scope } from '../world-graph/codec.js';
import { node, worldGraph } from '../world-graph/world-graph-client.js';
import type { Appearance, BodyOptions } from './body-options.js';
import { register, unregister } from './body-registry.js';
import { DisplayClone } from './display-clone.js';
import type { SystemAssembly } from './system-assembly.js';

type DisplaySize = { shapeId: string | null; revision: number | null; diagonal: number; floor: number };

function singleMaterial(material: THREE.Material | THREE.Material[], label: string): THREE.Material {
  if (Array.isArray(material)) throw new OGError('InvalidOperand', label, 'display material must be a single material');
  return material;
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
  private previousParent: THREE.Object3D | null = null;
  private surfaceKey: string;
  private lineKey: string;
  private lastCameraEvaluation = -Infinity;
  private sizeCache?: DisplaySize | undefined;

  protected constructor(ogId: string, bodyType: 'Wire' | 'Solid', options: BodyOptions) {
    super();
    this.ogId = ogId;
    this.lastInfo = node(ogId);
    this.handle = this.lastInfo.handle;
    this.generation = this.lastInfo.generation;
    this.bodyType = bodyType;
    this.appearance = {
      color: bodyType === 'Wire' ? 0x2563eb : 0x6699dd, opacity: 1, outline: bodyType === 'Wire', ...options.appearance,
    };
    this.matrixAutoUpdate = false;
    this.matrixWorldAutoUpdate = false;
    const surfaceStyle = surfaceMaterial(this.appearance);
    this.surfaceKey = surfaceStyle.key;
    this.surface = new THREE.Mesh(new THREE.BufferGeometry(), surfaceStyle.material);
    this.surface.visible = false;
    this.surface.frustumCulled = false;
    const lineStyle = lineMaterial(bodyType === 'Wire' ? 0x2563eb : 0x243040);
    this.lineKey = lineStyle.key;
    this.outline = new THREE.LineSegments(new THREE.BufferGeometry(), lineStyle.material);
    this.outline.visible = false;
    this.outline.frustumCulled = false;
    this.add(this.surface, this.outline);
    this.surface.onBeforeRender = (renderer, _scene, camera) => { this.observeCamera(renderer, camera); };
    this.outline.onBeforeRender = (renderer, _scene, camera) => { this.observeCamera(renderer, camera); };
    register(this);
    this.applyWorldMatrix(call('Body.constructor', () => worldGraph().worldMatrix(ogId)));
  }

  protected check(): void { call('Body', () => worldGraph().nodeByHandle(this.handle, this.generation)); }

  private observeCamera(renderer: THREE.WebGLRenderer, camera: THREE.Camera): void {
    const state = runtime();
    if (this.appearance.deflection !== undefined || state.displayDeflection !== undefined) return;
    const now = performance.now();
    if (now - this.lastCameraEvaluation < 100) return;
    this.lastCameraEvaluation = now;
    const bounds = this.getBounds();
    if (!bounds) return;
    const height = renderer.getDrawingBufferSize(new THREE.Vector2()).y;
    const target = worldUnitsPerPixel(camera, bounds, height) * (runtime().moving ? 2 : 0.5);
    const scale = Math.cbrt(Math.abs(this.matrixWorld.determinant()));
    if (Number.isFinite(target) && target > 0 && scale > 0) {
      const bucket = deflectionBucket(target / scale);
      if (this.lastInfo.shapeId && state.buckets.get(this.lastInfo.shapeId) !== bucket) {
        state.cameraBuckets.set(this.lastInfo.shapeId, bucket);
        state.buckets.delete(this.lastInfo.shapeId);
        queueMicrotask(() => { ensureGeometry(this); });
      }
    }
  }

  applyWorldMatrix(values: ArrayLike<number>): void {
    if ([0, 1, 2, 4, 5, 6, 8, 9, 10].some((index) => this.matrixWorld.elements[index] !== values[index])) {
      this.sizeCache = undefined;
    }
    this.matrixWorld.fromArray(values);
    if (this.parent) {
      this.matrix.copy(this.parent.matrixWorld).invert().multiply(this.matrixWorld);
    } else this.matrix.copy(this.matrixWorld);
    this.matrix.decompose(this.position, this.quaternion, this.scale);
    super.updateMatrixWorld(true);
  }

  override updateMatrixWorld(force?: boolean): void {
    if (this.visible) noteDisplayed(this);
    super.updateMatrixWorld(force);
  }

  swapReadyRecord(): void {
    if (!this.visible || !this.lastInfo.shapeId || this.lastInfo.shapeRevision === null) return;
    const bucket = wanted(this);
    const record = runtime().records.get(this.lastInfo.shapeId, this.lastInfo.shapeRevision, bucket);
    if (record) this.install(record);
  }

  install(record: GeometryRecord): void {
    if (this.record === record) return;
    const pool = runtime().records;
    if (this.record) pool.release(this.record);
    this.record = record;
    pool.retain(record);
    if (record.surface) this.surface.geometry = record.surface;
    this.outline.geometry = record.outline;
    this.surface.position.copy(record.origin);
    this.outline.position.copy(record.origin);
    this.surface.visible = this.bodyType === 'Solid';
    this.outline.visible = this.appearance.outline;
    this.surface.frustumCulled = true;
    this.outline.frustumCulled = true;
  }

  setAppearance(partial: Partial<Appearance>): void {
    const previous = this.appearance;
    this.appearance = { ...previous, ...partial };
    if (previous.color !== this.appearance.color || previous.opacity !== this.appearance.opacity) {
      const surfaceStyle = surfaceMaterial(this.appearance);
      releaseSurface(this.surfaceKey);
      this.surfaceKey = surfaceStyle.key;
      this.surface.material = surfaceStyle.material;
    }
    this.outline.visible = Boolean(this.record) && this.appearance.outline;
    if (this.appearance.deflection !== undefined) runtime().buckets.delete(this.lastInfo.shapeId ?? '');
  }

  transform(kind: string, params: Record<string, unknown>): void {
    this.check();
    call(`${this.bodyType}.transform`, () => worldGraph().transform(this.ogId, encode({ kind, ...params })));
  }

  getPlacement(): Placement {
    this.check();
    return JSON.parse(call(`${this.bodyType}.getPlacement`, () => worldGraph().placement(this.ogId)));
  }

  getWorldPlacement(): Placement {
    this.check();
    return JSON.parse(call(`${this.bodyType}.getWorldPlacement`, () => worldGraph().worldPlacement(this.ogId)));
  }

  addChild(children: (Body | SystemAssembly)[], options: { keepWorld?: boolean } = {}): void {
    this.check();
    call(`${this.bodyType}.addChild`, () => worldGraph().addChild(
      this.ogId, encode(children.map((child) => child.ogId)), Boolean(options.keepWorld),
    ));
  }

  removeChild(child: Body | SystemAssembly, options: { keepWorld?: boolean } = {}): void {
    this.check();
    call(`${this.bodyType}.removeChild`, () => worldGraph().removeChild(
      this.ogId, child.ogId, Boolean(options.keepWorld),
    ));
  }

  getChildren(): string[] {
    this.check();
    return JSON.parse(call(`${this.bodyType}.getChildren`, () => worldGraph().children(this.ogId)));
  }

  getParent(): string | null {
    this.check();
    return JSON.parse(call(`${this.bodyType}.getParent`, () => worldGraph().parent(this.ogId)));
  }

  getBounds(): [number, number, number, number, number, number] | null {
    this.check();
    return JSON.parse(call(`${this.bodyType}.getBounds`, () => worldGraph().bounds(this.ogId)));
  }

  displaySize(): { shapeId: string | null; revision: number | null; diagonal: number; floor: number } {
    const { shapeId, shapeRevision } = this.lastInfo;
    if (this.sizeCache && this.sizeCache.shapeId === shapeId && this.sizeCache.revision === shapeRevision) {
      return this.sizeCache;
    }
    const bounds = this.getBounds();
    const diagonal = bounds ? Math.hypot(bounds[3] - bounds[0], bounds[4] - bounds[1], bounds[5] - bounds[2]) : 1;
    const floor = deflectionBucket(Math.max(1e-7, diagonal * 2 ** -22));
    this.sizeCache = { shapeId, revision: shapeRevision, diagonal, floor };
    return this.sizeCache;
  }

  getBrep() {
    this.check();
    return JSON.parse(call(`${this.bodyType}.getBrep`, () => worldGraph().brep(this.ogId)));
  }

  getInstanceCount(): number {
    this.check();
    return call(`${this.bodyType}.getInstanceCount`, () => worldGraph().instanceCount(this.ogId));
  }

  makeUnique(): void {
    this.check();
    call(`${this.bodyType}.makeUnique`, () => worldGraph().makeUnique(this.ogId));
    flush();
  }

  rebuild(kind: string, params: Record<string, unknown>, options: { instances?: 'all' } = {}): void {
    this.check();
    const graph = worldGraph();
    if (kind === 'Polyline') {
      const points = params['points'] as [number, number, number][];
      call(`${this.bodyType}.rebuild`, () => graph.rebuildPolyline(
        this.ogId, new Float64Array(points.flat()), Boolean(params['closed']), scope(options),
      ));
    } else if (kind === 'Extrude' || kind === 'Sweep') {
      call(`${this.bodyType}.rebuild`, () => graph.rebuildOperation(
        this.ogId, encode(operationParams(kind, params)), scope(options),
      ));
    } else {
      call(`${this.bodyType}.rebuild`, () => graph.rebuildPrimitive(
        this.ogId, encode({ kind, ...params }), scope(options),
      ));
    }
    flush();
  }

  hideForDispose(): void {
    if (this.inLimbo) return;
    this.visible = false;
    runtime().displayed.delete(this);
    this.previousParent = this.parent;
    this.parent?.remove(this);
    this.inLimbo = true;
    if (runtime().marks.size === 0) this.finalize();
  }

  reviveIfRestored(): void {
    if (!this.inLimbo) return;
    try {
      call('revive', () => worldGraph().nodeByHandle(this.handle, this.generation));
      this.inLimbo = false;
      this.visible = true;
      this.previousParent?.add(this);
      this.previousParent = null;
      this.lastInfo = node(this.ogId);
    } catch { this.visible = false; }
  }

  finalize(): void {
    unregister(this);
    this.inLimbo = false;
    this.surface.geometry = new THREE.BufferGeometry();
    this.outline.geometry = new THREE.BufferGeometry();
    releaseSurface(this.surfaceKey);
    releaseLine(this.lineKey);
  }

  dispose(): void {
    this.check();
    call(`${this.bodyType}.dispose`, () => worldGraph().dispose(this.ogId));
    this.hideForDispose();
    flush();
  }

  override clone(): this {
    const label = `${this.bodyType}.clone`;
    const mesh = new THREE.Mesh(this.surface.geometry, singleMaterial(this.surface.material, label).clone());
    const outline = new THREE.LineSegments(this.outline.geometry, singleMaterial(this.outline.material, label).clone());
    mesh.position.copy(this.surface.position);
    outline.position.copy(this.outline.position);
    const display = new DisplayClone(this.record, mesh, outline);
    return display as unknown as this;
  }

  override copy(): this {
    throw new OGError('InvalidOperand', `${this.bodyType}.copy`, 'Use duplicate() for an independent body');
  }
}
