export class OGError extends Error {
  readonly code: string;
  readonly call: string;
  readonly details: unknown;

  constructor(code: string, call: string, message: string, details: unknown = {}) {
    super(message);
    this.name = 'OGError';
    this.code = code;
    this.call = call;
    this.details = details;
  }

  toJSON(): { code: string; call: string; message: string; details: unknown } {
    return { code: this.code, call: this.call, message: this.message, details: this.details };
  }

  static fromJSON(value: { code: string; call: string; message: string; details?: unknown }): OGError {
    return new OGError(value.code, value.call, value.message, value.details);
  }
}
