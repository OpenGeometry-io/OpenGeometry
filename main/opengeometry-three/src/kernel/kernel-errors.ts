export function parseKernelError(text: string): { code: string; message: string; details: unknown } | undefined {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return undefined;
  }
  if (typeof value !== 'object' || value === null || !('code' in value) || !('message' in value)) return undefined;
  const { code, message } = value;
  if (typeof code !== 'string' || typeof message !== 'string' || !code || !message) return undefined;
  return { code, message, details: 'details' in value ? value.details : undefined };
}
