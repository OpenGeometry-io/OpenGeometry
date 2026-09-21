export type AnalyticGeometryErrorCode =
  | "math"
  | "invalid_geometry"
  | "invalid_topology"
  | "unsupported_geometry"
  | "ambiguous_profile_alignment"
  | "coverage_gap"
  | "singular_parameterization"
  | "missing_reference"
  | "unsupported_schema"
  | "unresolved_intersection"
  | "unresolved_tessellation"
  | "limit_exceeded"
  | "unknown";

export class AnalyticGeometryError extends Error {
  readonly code: AnalyticGeometryErrorCode;
  readonly detail: unknown;
  readonly families?: readonly [string, string];

  constructor(code: AnalyticGeometryErrorCode, message: string, detail?: unknown) {
    super(message);
    this.name = "AnalyticGeometryError";
    this.code = code;
    this.detail = detail;
    if (code === "coverage_gap" && isRecord(detail) && isStringPair(detail.families)) {
      this.families = detail.families;
    }
  }
}

export function parseAnalyticGeometryError(raw: unknown): AnalyticGeometryError {
  const serialized = extractMessage(raw);
  try {
    const payload = JSON.parse(serialized) as unknown;
    if (typeof payload === "string") {
      const code = errorCode(payload);
      return new AnalyticGeometryError(code, messageFor(code));
    }
    if (isRecord(payload)) {
      const variant = Object.keys(payload)[0];
      if (variant) {
        const detail = payload[variant];
        const code = errorCode(variant);
        return new AnalyticGeometryError(code, messageFor(code, detail), detail);
      }
    }
  } catch {
    // The fallback below preserves non-kernel JavaScript failures.
  }
  return new AnalyticGeometryError("unknown", serialized || "Unknown analytic geometry error", raw);
}

function messageFor(code: AnalyticGeometryErrorCode, detail?: unknown): string {
  if (code === "coverage_gap" && isRecord(detail) && isStringPair(detail.families)) {
    return `Analytic boolean coverage is unavailable for ${detail.families[0]} and ${detail.families[1]}.`;
  }
  if (code === "missing_reference" && isRecord(detail)) {
    return `Missing ${String(detail.kind ?? "geometry")} reference ${String(detail.index ?? "")}.`.trim();
  }
  if (code === "unsupported_schema" && isRecord(detail)) {
    return `Unsupported BRep schema version ${String(detail.found ?? "unknown")}; schema version 2 is required.`;
  }
  if (typeof detail === "string" && detail.length > 0) return detail;
  return code.replace(/_/g, " ");
}

function errorCode(variant: string): AnalyticGeometryErrorCode {
  const code = variant.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
  switch (code) {
    case "math":
    case "invalid_geometry":
    case "invalid_topology":
    case "unsupported_geometry":
    case "ambiguous_profile_alignment":
    case "coverage_gap":
    case "singular_parameterization":
    case "missing_reference":
    case "unsupported_schema":
    case "unresolved_intersection":
    case "unresolved_tessellation":
    case "limit_exceeded":
      return code;
    default:
      return "unknown";
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStringPair(value: unknown): value is [string, string] {
  return Array.isArray(value) && value.length === 2 && value.every((item) => typeof item === "string");
}

function extractMessage(raw: unknown): string {
  if (typeof raw === "string") return raw;
  if (raw instanceof Error) return raw.message;
  if (isRecord(raw) && typeof raw.message === "string") return raw.message;
  try {
    return String(raw ?? "");
  } catch {
    return "Unknown analytic geometry error";
  }
}
