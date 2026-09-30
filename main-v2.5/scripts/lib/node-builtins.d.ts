declare module 'node:fs' {
  export interface Dirent {
    name: string;
    parentPath: string;
    isFile(): boolean;
    isDirectory(): boolean;
  }
  export function readFileSync(path: string | URL, encoding: 'utf8'): string;
  export function readFileSync(path: string | URL): Uint8Array;
  export function readdirSync(path: string, options: { withFileTypes: true; recursive?: boolean }): Dirent[];
  export function existsSync(path: string): boolean;
  export function mkdirSync(path: string, options?: { recursive?: boolean }): string | undefined;
  export function openSync(path: string, flags: string): number;
  export function writeSync(fd: number, text: string): number;
  export function closeSync(fd: number): void;
}

declare module 'node:child_process' {
  export interface ChildProcess {
    on(event: 'close', listener: (code: number | null) => void): ChildProcess;
    on(event: 'error', listener: (error: Error) => void): ChildProcess;
  }
  export function spawn(
    command: string,
    args: string[],
    options: { cwd: string; env: Record<string, string | undefined>; stdio: ['ignore', number, number] },
  ): ChildProcess;
}

declare module 'node:test' {
  export function test(name: string, body: () => void | Promise<void>): Promise<void>;
}

declare module 'node:fs/promises' {
  export interface Stats {
    isDirectory(): boolean;
  }
  export function cp(source: string, destination: string): Promise<void>;
  export function mkdir(path: string, options?: { recursive?: boolean }): Promise<string | undefined>;
  export function readFile(path: string, encoding: 'utf8'): Promise<string>;
  export function readdir(path: string): Promise<string[]>;
  export function stat(path: string): Promise<Stats>;
  export function writeFile(path: string, data: string | Uint8Array): Promise<void>;
}

declare module 'node:path' {
  interface Path {
    sep: string;
    resolve(...segments: string[]): string;
    dirname(path: string): string;
    join(...segments: string[]): string;
    relative(from: string, to: string): string;
    basename(path: string, suffix?: string): string;
    extname(path: string): string;
  }
  const path: Path;
  export default path;
}

declare module 'node:url' {
  export function fileURLToPath(url: string | URL): string;
  export function pathToFileURL(path: string): URL;
}

declare module 'node:crypto' {
  export interface Hash {
    update(data: string | Uint8Array): Hash;
    digest(encoding: 'hex'): string;
  }
  export function createHash(algorithm: string): Hash;
}

declare module 'node:assert/strict' {
  interface Assert {
    (value: unknown, message?: string): asserts value;
    equal(actual: unknown, expected: unknown, message?: string): void;
    notEqual(actual: unknown, expected: unknown, message?: string): void;
    deepEqual(actual: unknown, expected: unknown, message?: string): void;
    throws(block: () => unknown, error?: (error: unknown) => boolean, message?: string): void;
  }
  const assert: Assert;
  export default assert;
}

declare module 'node:perf_hooks' {
  export const performance: { now(): number };
}

declare const process: {
  argv: string[];
  arch: string;
  platform: string;
  env: { OG_THREE_VERSION?: string; OG_PW_PORT_BASE?: string };
  exitCode: number | undefined;
  stdout: { write(text: string): boolean };
  stderr: { write(text: string): boolean };
};

interface ImportMeta {
  readonly dirname: string;
}
