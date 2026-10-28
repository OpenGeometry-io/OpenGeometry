import type { ShapeReport } from '../dto/boolean-report';
import type { Bounds } from '../dto/bounds';
import type { Brep } from '../dto/brep';
import type { ChangeSet } from '../dto/change-set';
import type { Creation } from '../dto/creation';
import type { AnalyticExchangeBodyV2 } from '../dto/ifc-body';
import type { MarkStats } from '../dto/mark-stats';
import type { NodeInfo } from '../dto/node-info';
import type { Placement } from '../dto/placement';
import type { CurvedRegion2D, OffsetPath2D, OffsetRegion, RegionOperation } from '../dto/regions';
import type { StepExport, StepExportReport } from '../dto/step-export';
import { OGError } from '../errors';

type Fields = Record<string, unknown>;
type Check<T> = (value: unknown) => value is T;
type ChangePacket = { changesJson: string; matrices: Float64Array };
type StepGlue = { text: string; reportJson: string };

const BREP_TOPOLOGY = ['vertices', 'edges', 'halfedges', 'loops', 'faces', 'wires', 'shells'] as const;
const STEP_COUNTS = ['products', 'solids', 'faces', 'edges', 'pcurvelessEdges', 'entities', 'bytes'] as const;
const REGION_OPERATIONS = ['union', 'intersection', 'subtraction'] as const;

export function encode(value: unknown): string {
  return JSON.stringify(value);
}

export function scope(options?: { instances?: 'all' }): string {
  return encode(options?.instances === 'all' ? 'AllInstances' : 'Node');
}

export function operationParams(kind: string, params: Record<string, unknown>, call: string): Record<string, unknown> {
  if (kind === 'Extrude') {
    const holes = params['holes'] ?? [];
    if (!Array.isArray(holes)) throw new OGError('InvalidParameter', call, 'holes must be a list of bodies');
    const holeIds = holes.map((hole: unknown) => ogIdOf(hole, call, 'hole'));
    return { kind, profile: ogIdOf(params['profile'], call, 'profile'), holes: holeIds, distance: params['distance'] };
  }
  if (kind === 'Sweep') {
    const profile = ogIdOf(params['profile'], call, 'profile');
    return { kind, profile, path: ogIdOf(params['path'], call, 'path'), caps: params['caps'] };
  }
  if (kind === 'Revolve') return { ...params, kind, profile: ogIdOf(params['profile'], call, 'profile') };
  return { kind, ...params };
}

export function polylinePoints(params: Record<string, unknown>, call: string): Float64Array {
  const points = params['points'];
  if (!Array.isArray(points) || !points.every(isPoint)) {
    throw new OGError('InvalidParameter', call, 'points must be a list of [x, y, z] points');
  }
  return new Float64Array(points.flat());
}

export function regionsJson(regions: unknown, call: string): string {
  if (!Array.isArray(regions) || !regions.every(isOffsetRegion)) {
    throw new OGError('InvalidParameter', call, 'regions must be a list of { outer, holes } of { x, y, z } rings');
  }
  const ring = (points: OffsetRegion['outer']): number[][] => points.map((point) => [point.x, point.y, point.z]);
  return encode(regions.map((region) => ({ outer: ring(region.outer), holes: region.holes.map(ring) })));
}

export function regionOperation(value: unknown, call: string): RegionOperation {
  if (!isRegionOperation(value)) {
    throw new OGError('InvalidParameter', call, "operation must be 'union', 'intersection' or 'subtraction'");
  }
  return value;
}

export function curvedRegionsJson(regions: unknown, call: string): string {
  if (!Array.isArray(regions)) throw new OGError('InvalidParameter', call, 'curved regions must be a list');
  return encode(regions);
}

export function numberList(value: unknown, call: string, label: string): Float64Array {
  if (!isNumberList(value)) throw new OGError('InvalidParameter', call, `${label} must be a list of numbers`);
  return value instanceof Float64Array ? value : new Float64Array(value);
}

export function offsetHolesJson(holes: unknown, call: string): string {
  if (!Array.isArray(holes) || !holes.every(isOffsetHole)) {
    throw new OGError('InvalidParameter', call, 'holes must be a list of { ring, distances }');
  }
  return encode(holes.map((hole) => ({ ring: Array.from(hole.ring), distances: Array.from(hole.distances) })));
}

export function decodeOffsetPath(text: string, call: string): OffsetPath2D {
  return decode(text, call, 'offset path', (value): value is OffsetPath2D => isFields(value)
    && isList(value['edges'], isCurvedEdge) && isList(value['bevelled'], isNumber)
    && typeof value['closed'] === 'boolean');
}

export function decodeRegions(text: string, call: string): OffsetRegion[] {
  return decode(text, call, 'regions', (value): value is OffsetRegion[] => isList(value, isOffsetRegion));
}

export function decodeCurvedRegions(text: string, call: string): CurvedRegion2D[] {
  return decode(text, call, 'curved regions', (value): value is CurvedRegion2D[] => isList(value, isCurvedRegion));
}

function decode<T>(text: string, call: string, payload: string, check: Check<T>): T {
  return checked(parsed(text), call, payload, check);
}

export function decodeNodeInfo(text: string, call: string): NodeInfo {
  return decode(text, call, 'node info', isNodeInfo);
}

export function decodeCreation(text: string, call: string): Creation {
  return decode(text, call, 'creation', (value): value is Creation => isFields(value) && isString(value['ogId']));
}

export function decodePlacement(text: string, call: string): Placement {
  return decode(text, call, 'placement', isPlacement);
}

export function decodeChildren(text: string, call: string): string[] {
  return decode(text, call, 'children', isStrings);
}

export function decodeParent(text: string, call: string): string | null {
  return decode(text, call, 'parent', (value): value is string | null => value === null || isString(value));
}

export function decodeBounds(text: string, call: string): Bounds | null {
  return decode(text, call, 'bounds', (value): value is Bounds | null => value === null || isNumbers(value, 6));
}

export function decodeIfcBody(text: string, call: string): AnalyticExchangeBodyV2 {
  return decode(text, call, 'IFC body', isIfcBody);
}

export function decodeBrep(text: string, call: string): Brep {
  return decode(text, call, 'brep', isBrep);
}

export function decodeShapeReport(text: string, call: string): ShapeReport | null {
  return decode(text, call, 'report', (value): value is ShapeReport | null => value === null || isShapeReport(value));
}

export function decodeMarkStats(text: string, call: string): MarkStats {
  return decode(text, call, 'mark stats', (value): value is MarkStats => isFields(value)
    && isNumber(value['liveMarks']) && isNumber(value['retainedRevisions']));
}

export function decodeDisplayBuckets(text: string, call: string): { floor: number; static: number } {
  return decode(text, call, 'display buckets', (value): value is { floor: number; static: number } => isFields(value)
    && Number.isFinite(value['floor']) && Number.isFinite(value['static']));
}

export function decodeChangeSet(packet: unknown, call: string): { changes: ChangeSet; matrices: Float64Array } {
  const glue = checked(packet, call, 'change packet', isChangePacket);
  return { changes: decode(glue.changesJson, call, 'change set', isChangeSet), matrices: glue.matrices };
}

export function decodeChanges(text: string, call: string): ChangeSet {
  return decode(text, call, 'change set', isChangeSet);
}

export function decodeStepExport(value: unknown, call: string): StepExport {
  const glue = checked(value, call, 'STEP export', isStepGlue);
  return { text: glue.text, report: decode(glue.reportJson, call, 'STEP report', isStepReport) };
}

function checked<T>(value: unknown, call: string, payload: string, check: Check<T>): T {
  if (!check(value)) throw new OGError('InvalidGeometry', call, `malformed ${payload}`);
  return value;
}

function parsed(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

function ogIdOf(value: unknown, call: string, role: string): string {
  if (isFields(value) && isString(value['ogId'])) return value['ogId'];
  throw new OGError('InvalidParameter', call, `${role} must be a body`);
}

function isFields(value: unknown): value is Fields {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isString(value: unknown): value is string {
  return typeof value === 'string';
}

function isNumber(value: unknown): value is number {
  return typeof value === 'number';
}

function isStrings(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(isString);
}

function isNumbers(value: unknown, length: number): boolean {
  return Array.isArray(value) && value.length === length && value.every(isNumber);
}

function isPoint(value: unknown): value is [number, number, number] {
  return isNumbers(value, 3);
}

function isNodeInfo(value: unknown): value is NodeInfo {
  return isFields(value) && isString(value['ogId']) && isNumber(value['handle']) && isNumber(value['generation'])
    && isNullable(value['parent'], isString) && isStrings(value['children']) && isString(value['kind'])
    && isNullable(value['shapeId'], isString) && isNullable(value['shapeRevision'], isNumber)
    && isNullable(value['bodyType'], isString);
}

function isNullable(value: unknown, check: (item: unknown) => boolean): boolean {
  return value === null || check(value);
}

function isPlacement(value: unknown): value is Placement {
  return isFields(value) && isPoint(value['origin']) && isPoint(value['xDirection']) && isPoint(value['normal'])
    && isNumber(value['scale']);
}

function isBrep(value: unknown): value is Brep {
  if (!isFields(value) || !isFields(value['topology'])) return false;
  const topology = value['topology'];
  return isString(value['id']) && isNumber(value['revision']) && isNumber(value['schema_version'])
    && isFields(value['geometry']) && isFields(value['accuracy']) && isFields(value['quality'])
    && Array.isArray(value['solids']) && BREP_TOPOLOGY.every((key) => Array.isArray(topology[key]));
}

function isShapeReport(value: unknown): value is ShapeReport {
  return isFields(value) && isFields(value['report']) && isStrings(value['handlers']) && isNumber(value['revision']);
}

function isChangePacket(value: unknown): value is ChangePacket {
  return isFields(value) && isString(value['changesJson']) && value['matrices'] instanceof Float64Array;
}

function isChangeSet(value: unknown): value is ChangeSet {
  const isChanges = (list: unknown): boolean => Array.isArray(list)
    && list.every((item: unknown) => isFields(item) && isString(item['ogId']) && isNumber(item['handle'])
      && isNumber(item['generation']) && isNullable(item['shapeId'], isString)
      && isNullable(item['shapeRevision'], isNumber));
  return isFields(value) && isNumber(value['revision'])
    && isChanges(value['added']) && isChanges(value['changed']) && isChanges(value['removed']);
}

function isStepGlue(value: unknown): value is StepGlue {
  return isFields(value) && isString(value['text']) && isString(value['reportJson']);
}

function isStepReport(value: unknown): value is StepExportReport {
  return isFields(value) && STEP_COUNTS.every((key) => isNumber(value[key]))
    && Array.isArray(value['bodies']) && Array.isArray(value['skipped']);
}

function isList(value: unknown, check: (item: unknown) => boolean): boolean {
  return Array.isArray(value) && value.every(check);
}

function isRegionOperation(value: unknown): value is RegionOperation {
  return REGION_OPERATIONS.some((operation) => operation === value);
}

function hasOnly(value: Fields, keys: string[]): boolean {
  return Object.keys(value).every((key) => keys.includes(key));
}

function isOffsetRegion(value: unknown): value is OffsetRegion {
  const isRing = (ring: unknown): boolean => isList(ring, (point) => isFields(point) && hasOnly(point, ['x', 'y', 'z'])
    && isNumber(point['x']) && isNumber(point['y']) && isNumber(point['z']));
  return isFields(value) && hasOnly(value, ['outer', 'holes']) && isRing(value['outer'])
    && isList(value['holes'], isRing);
}

function isNumberList(value: unknown): value is ArrayLike<number> {
  return value instanceof Float64Array || isList(value, isNumber);
}

function isOffsetHole(value: unknown): value is { ring: ArrayLike<number>; distances: ArrayLike<number> } {
  return isFields(value) && hasOnly(value, ['ring', 'distances'])
    && isNumberList(value['ring']) && isNumberList(value['distances']);
}

function isCurvedEdge(edge: unknown): boolean {
  return isFields(edge) && (edge['kind'] === 'line' || edge['kind'] === 'arc');
}

function isCurvedRegion(value: unknown): value is CurvedRegion2D {
  const isEdges = (ring: unknown): boolean => isList(ring, isCurvedEdge);
  return isFields(value) && isEdges(value['outer']) && isList(value['holes'], isEdges);
}

function isIfcBody(value: unknown): value is AnalyticExchangeBodyV2 {
  return isFields(value) && value['schema_version'] === 2 && value['length_unit'] === 'metre'
    && value['up_axis'] === 'Z' && value['coordinate_space'] === 'world'
    && (value['representation_type'] === 'AdvancedBrep' || value['representation_type'] === 'CSG')
    && Array.isArray(value['entities']) && Array.isArray(value['solids']) && Array.isArray(value['faces']);
}
