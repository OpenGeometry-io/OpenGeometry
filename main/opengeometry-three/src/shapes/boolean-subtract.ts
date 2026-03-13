import { booleanSubtraction } from "../operations/boolean";
import type {
  BooleanExecutionOptions,
  BooleanOperand,
  BooleanResult,
} from "../operations/boolean";

export type ShapeSubtractOperand = BooleanOperand;
export type ShapeSubtractOptions = BooleanExecutionOptions;
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
