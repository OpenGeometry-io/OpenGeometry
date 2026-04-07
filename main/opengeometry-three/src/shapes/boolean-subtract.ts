import { booleanSubtraction } from "../operations/boolean";
import type {
  BooleanExecutionOptions,
  BooleanOperand,
  BooleanResult,
} from "../operations/boolean";

/**
 * Operand types accepted by shape instance `.subtract(...)` helpers.
 */
export type ShapeSubtractOperand = BooleanOperand;

/**
 * Options accepted by shape instance `.subtract(...)` helpers.
 */
export type ShapeSubtractOptions = BooleanExecutionOptions;

/**
 * Return type for shape instance `.subtract(...)` helpers.
 */
export type ShapeSubtractResult = BooleanResult;

/**
 * Executes a kernel-backed subtraction for a shape wrapper against another boolean operand.
 */
export function subtractShapeOperand(
  host: ShapeSubtractOperand,
  operand: ShapeSubtractOperand,
  options?: ShapeSubtractOptions
): ShapeSubtractResult {
  return booleanSubtraction(host, operand, options);
}
