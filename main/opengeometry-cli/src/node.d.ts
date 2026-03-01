declare module "node:crypto" {
  export function randomUUID(): string;
}

declare module "node:fs" {
  export function readFileSync(path: string): Uint8Array;
}

declare module "node:fs/promises" {
  export function mkdir(path: string, options?: { recursive?: boolean }): Promise<void>;
  export function readFile(path: string, encoding: string): Promise<string>;
  export function rename(oldPath: string, newPath: string): Promise<void>;
  export function writeFile(path: string, data: string, encoding?: string): Promise<void>;
}

declare module "node:path" {
  export function dirname(path: string): string;
  export function resolve(...paths: string[]): string;
}

declare module "node:url" {
  export function fileURLToPath(url: unknown): string;
}

declare const process: {
  argv: string[];
  cwd(): string;
  stdout: { write(chunk: string): void };
  stderr: { write(chunk: string): void };
  exitCode?: number;
};
