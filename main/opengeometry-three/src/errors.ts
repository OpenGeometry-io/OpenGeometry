import type { OperateErrorDetails } from './dto/boolean-report';

export type ErrorDetails = Partial<OperateErrorDetails> & Record<string, unknown>;

function isDetails(value: unknown): value is ErrorDetails {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export class OGError extends Error {
  readonly code: string;
  readonly call: string;
  readonly details: ErrorDetails;

  constructor(code: string, call: string, message: string, details: unknown = {}) {
    super(message);
    this.name = 'OGError';
    this.code = code;
    this.call = call;
    this.details = isDetails(details) ? details : {};
  }

  toJSON(): { code: string; call: string; message: string; details: unknown } {
    return { code: this.code, call: this.call, message: this.message, details: this.details };
  }

  static fromJSON(value: { code: string; call: string; message: string; details?: unknown }): OGError {
    return new OGError(value.code, value.call, value.message, value.details);
  }
}
