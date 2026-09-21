import * as THREE from "three";

import { AnalyticSolid, type AnalyticBooleanOp } from "../shapes/analytic-solid";

/** Rendering and tessellation options applied to an analytic boolean result. */
export interface BooleanExecutionOptions {
  deflection?: number;
  color?: THREE.ColorRepresentation;
  opacity?: number;
  transparent?: boolean;
  side?: THREE.Side;
  outline?: boolean;
}

export type BooleanOperand = AnalyticSolid;

export function booleanUnion(
  lhs: AnalyticSolid,
  rhs: AnalyticSolid,
  options?: BooleanExecutionOptions,
): AnalyticSolid {
  return executeBoolean(lhs, rhs, "union", options);
}

export function booleanIntersection(
  lhs: AnalyticSolid,
  rhs: AnalyticSolid,
  options?: BooleanExecutionOptions,
): AnalyticSolid {
  return executeBoolean(lhs, rhs, "intersection", options);
}

export function booleanSubtraction(
  lhs: AnalyticSolid,
  rhs: AnalyticSolid,
  options?: BooleanExecutionOptions,
): AnalyticSolid {
  return executeBoolean(lhs, rhs, "subtraction", options);
}

export function executeBooleanSubtractionMany(
  lhs: BooleanOperand,
  cutters: BooleanOperand[],
  options?: BooleanExecutionOptions,
): AnalyticSolid {
  const host = analyticOperand(lhs);
  if (!Array.isArray(cutters) || cutters.length === 0) {
    throw new Error("Analytic subtraction requires a nonempty operand array.");
  }
  const analyticCutters = cutters.map(analyticOperand);
  let result = host.boolean(analyticCutters[0], "subtraction", {
    deflection: options?.deflection ?? host.deflection,
    color: options?.color,
  });
  try {
    for (const cutter of analyticCutters.slice(1)) {
      const next = result.boolean(cutter, "subtraction", {
        deflection: options?.deflection ?? result.deflection,
        color: options?.color,
      });
      result.dispose();
      result = next;
    }
  } catch (error) {
    result.dispose();
    throw error;
  }
  applyRenderOptions(result, options);
  return result;
}

function executeBoolean(
  lhs: AnalyticSolid,
  rhs: AnalyticSolid,
  operation: AnalyticBooleanOp,
  options?: BooleanExecutionOptions,
): AnalyticSolid {
  const host = analyticOperand(lhs);
  const tool = analyticOperand(rhs);
  const result = host.boolean(tool, operation, {
    deflection: options?.deflection ?? host.deflection,
    color: options?.color,
  });
  applyRenderOptions(result, options);
  return result;
}

function analyticOperand(operand: BooleanOperand): AnalyticSolid {
  if (!(operand instanceof AnalyticSolid)) {
    throw new Error(
      "Boolean operations require strict analytic BRep v2 operands; faceted BRep operands are unsupported.",
    );
  }
  return operand;
}

function applyRenderOptions(result: AnalyticSolid, options?: BooleanExecutionOptions): void {
  const material = result.surface.material;
  if (options?.color !== undefined) material.color.set(options.color);
  if (options?.opacity !== undefined) {
    material.opacity = options.opacity;
    material.transparent = options.opacity < 1 || material.transparent;
  }
  if (options?.transparent !== undefined) material.transparent = options.transparent;
  if (options?.side !== undefined) material.side = options.side;
  material.needsUpdate = true;
  result.outline = options?.outline ?? true;
}
