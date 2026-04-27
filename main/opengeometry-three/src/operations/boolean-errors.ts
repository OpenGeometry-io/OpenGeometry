/**
 * Typed error hierarchy for boolean operations.
 *
 * The Rust kernel emits a structured `BooleanError` as JSON across the WASM
 * boundary (see `main/opengeometry/src/booleans/error.rs`). This module parses
 * that JSON and re-throws it as a typed `BooleanError` (or a more specific
 * subclass) so app code can `instanceof`-discriminate without parsing message
 * strings.
 *
 * Cutter indices are **0-based** to match `Vec` / array indexing on both
 * sides of the boundary.
 */

import * as THREE from "three";

export type BooleanErrorPhase =
  | "input_validation"
  | "union_pre_pass"
  | "subtract_step"
  | "output_validation";

export type BooleanErrorReason =
  | "invalid_operand"
  | "malformed_input"
  | "mixed_operand_kinds"
  | "unsupported_operand_kind"
  | "non_coplanar_planar_operands"
  | "topology_error"
  | "kernel_failure"
  | "non_manifold_edges"
  | "open_shell"
  | "coincident_faces"
  | "degenerate_triangle"
  | "empty_result"
  | "overlapping_cutters"
  | "cutter_exceeds_host";

interface KindPayload {
  code: BooleanErrorReason;
  other_index?: number;
  axis?: string;
  overshoot?: number;
}

interface BooleanErrorJson {
  kind: KindPayload;
  phase: BooleanErrorPhase;
  cutter_index: number | null;
  message: string;
  details: string | null;
  edge_samples: Array<[{ x: number; y: number; z: number }, { x: number; y: number; z: number }]> | null;
}

/**
 * Base class for every typed boolean error. Carries the structured payload the
 * kernel emitted across WASM. Specialized subclasses (`OverlappingCuttersError`,
 * `CutterExceedsHostError`, `NonManifoldOutputError`, `CoincidentFacesError`)
 * narrow the payload for the variants that carry extra fields.
 */
export class BooleanError extends Error {
  readonly reason: BooleanErrorReason;
  readonly phase: BooleanErrorPhase;
  readonly cutterIndex: number | null;
  readonly details: string | null;
  readonly edgeSamples: ReadonlyArray<[THREE.Vector3, THREE.Vector3]> | null;

  constructor(payload: BooleanErrorJson) {
    super(payload.message);
    this.name = "BooleanError";
    this.reason = payload.kind.code;
    this.phase = payload.phase;
    this.cutterIndex = payload.cutter_index;
    this.details = payload.details;
    this.edgeSamples = payload.edge_samples
      ? payload.edge_samples.map(([a, b]) => [
          new THREE.Vector3(a.x, a.y, a.z),
          new THREE.Vector3(b.x, b.y, b.z),
        ] as [THREE.Vector3, THREE.Vector3])
      : null;
  }
}

/**
 * Two cutters overlap each other. `otherIndex` is the 0-based index of the
 * cutter that conflicts with the one at `cutterIndex`.
 */
export class OverlappingCuttersError extends BooleanError {
  readonly otherIndex: number;

  constructor(payload: BooleanErrorJson) {
    super(payload);
    this.name = "OverlappingCuttersError";
    this.otherIndex = payload.kind.other_index ?? -1;
  }
}

/**
 * A cutter extends past the host's hull along the named axis.
 */
export class CutterExceedsHostError extends BooleanError {
  readonly axis: "x" | "y" | "z";
  readonly overshoot: number;

  constructor(payload: BooleanErrorJson) {
    super(payload);
    this.name = "CutterExceedsHostError";
    const axis = (payload.kind.axis ?? "x").toLowerCase();
    this.axis = (axis === "y" || axis === "z" ? axis : "x") as "x" | "y" | "z";
    this.overshoot = payload.kind.overshoot ?? 0;
  }
}

/**
 * Output mesh has open or over-shared edges after a boolean. `edgeSamples`
 * carries up to ~8 sample non-manifold edges to aid app-side debugging.
 */
export class NonManifoldOutputError extends BooleanError {
  constructor(payload: BooleanErrorJson) {
    super(payload);
    this.name = "NonManifoldOutputError";
  }
}

/**
 * Cutter and host share a coplanar face. Distinct from
 * `DegenerateTriangleError` (zero-area input) — see `06a-snap-tolerance.md`.
 */
export class CoincidentFacesError extends BooleanError {
  constructor(payload: BooleanErrorJson) {
    super(payload);
    this.name = "CoincidentFacesError";
  }
}

/**
 * Parses a WASM-thrown error and returns the appropriate typed `BooleanError`
 * subclass. If the error's message is not valid `BooleanError` JSON, returns a
 * fallback `BooleanError` with `reason: "kernel_failure"` carrying the original
 * string, so callers always get a typed error and never have to `try/catch`
 * twice.
 */
export function parseBooleanError(raw: unknown): BooleanError {
  const message = extractMessage(raw);
  let payload: BooleanErrorJson | null = null;

  try {
    const parsed = JSON.parse(message) as Partial<BooleanErrorJson>;
    if (parsed && typeof parsed === "object" && parsed.kind && typeof parsed.kind === "object") {
      payload = {
        kind: parsed.kind as KindPayload,
        phase: (parsed.phase ?? "input_validation") as BooleanErrorPhase,
        cutter_index: parsed.cutter_index ?? null,
        message: parsed.message ?? message,
        details: parsed.details ?? null,
        edge_samples: parsed.edge_samples ?? null,
      };
    }
  } catch {
    payload = null;
  }

  if (!payload) {
    payload = {
      kind: { code: "kernel_failure" },
      phase: "input_validation",
      cutter_index: null,
      message,
      details: null,
      edge_samples: null,
    };
  }

  switch (payload.kind.code) {
    case "overlapping_cutters":
      return new OverlappingCuttersError(payload);
    case "cutter_exceeds_host":
      return new CutterExceedsHostError(payload);
    case "non_manifold_edges":
    case "open_shell":
      return new NonManifoldOutputError(payload);
    case "coincident_faces":
      return new CoincidentFacesError(payload);
    default:
      return new BooleanError(payload);
  }
}

function extractMessage(raw: unknown): string {
  if (raw === null || raw === undefined) {
    return "";
  }
  if (typeof raw === "string") {
    return raw;
  }
  if (raw instanceof Error) {
    return raw.message;
  }
  if (typeof raw === "object" && "message" in (raw as Record<string, unknown>)) {
    const message = (raw as Record<string, unknown>).message;
    if (typeof message === "string") {
      return message;
    }
  }
  try {
    return String(raw);
  } catch {
    return "Unknown boolean error";
  }
}
