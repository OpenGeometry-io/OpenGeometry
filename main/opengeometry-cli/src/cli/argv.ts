import type { Point3 } from "../types.js";

export interface ParsedOptions {
  positionals: string[];
  options: Record<string, string | boolean>;
}

export class CliUsageError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CliUsageError";
  }
}

export function parseOptionTokens(tokens: string[]): ParsedOptions {
  const positionals: string[] = [];
  const options: Record<string, string | boolean> = {};

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];

    if (!token.startsWith("--")) {
      positionals.push(token);
      continue;
    }

    const withoutPrefix = token.slice(2);
    if (withoutPrefix.length === 0) {
      throw new CliUsageError("Invalid empty option name");
    }

    const eqIndex = withoutPrefix.indexOf("=");
    if (eqIndex >= 0) {
      const key = withoutPrefix.slice(0, eqIndex);
      const rawValue = withoutPrefix.slice(eqIndex + 1);
      options[key] = rawValue;
      continue;
    }

    const key = withoutPrefix;
    const nextToken = tokens[index + 1];
    if (nextToken !== undefined && !nextToken.startsWith("--")) {
      options[key] = nextToken;
      index += 1;
      continue;
    }

    options[key] = true;
  }

  return { positionals, options };
}

export function hasFlag(parsed: ParsedOptions, key: string): boolean {
  return key in parsed.options;
}

export function getOptionalStringOption(
  parsed: ParsedOptions,
  key: string
): string | undefined {
  const value = parsed.options[key];
  if (value === undefined) {
    return undefined;
  }

  if (typeof value !== "string") {
    throw new CliUsageError(`Option --${key} requires a value`);
  }

  return value;
}

export function getRequiredStringOption(parsed: ParsedOptions, key: string): string {
  const value = getOptionalStringOption(parsed, key);
  if (value === undefined) {
    throw new CliUsageError(`Missing required option --${key}`);
  }

  return value;
}

export function parsePoint3(value: string, optionName: string): Point3 {
  const parts = value.split(",").map((part) => Number(part.trim()));
  if (parts.length !== 3 || parts.some((item) => !Number.isFinite(item))) {
    throw new CliUsageError(
      `Invalid ${optionName} value '${value}'. Expected comma-separated numbers: x,y,z`
    );
  }

  return {
    x: parts[0],
    y: parts[1],
    z: parts[2]
  };
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return JSON.stringify(error);
}
