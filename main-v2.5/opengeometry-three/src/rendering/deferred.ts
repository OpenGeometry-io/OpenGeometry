import type { OGError } from '../errors.js';

export class Deferred<T> {
  resolve!: (value: T) => void;
  reject!: (reason: OGError) => void;
  readonly promise: Promise<T>;

  constructor() {
    this.promise = new Promise<T>((resolve, reject) => {
      this.resolve = resolve;
      this.reject = reject;
    });
    this.promise.catch(() => undefined);
  }
}
