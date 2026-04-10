import { executeBooleanSubtractionMany } from "../operations/boolean";
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
 * Array-only operand list accepted by shape instance `.subtract(...)` helpers.
 */
export type ShapeSubtractOperands = ShapeSubtractOperand[];

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
  operands: ShapeSubtractOperands,
  options?: ShapeSubtractOptions
): ShapeSubtractResult {
  if (!Array.isArray(operands)) {
    throw new Error(
      "shape.subtract(...) now requires an operand array. Pass shape.subtract([operand], options) even for a single cutter."
    );
  }

  return executeBooleanSubtractionMany(host, operands, options);
}
