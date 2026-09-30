import { OGError } from '../errors.js';
import { emit } from '../runtime/event-bus.js';
import { runtime as runtimeState } from '../runtime/runtime-state.js';

export function kernelCall<T>(call: string, fn: () => T): T {
  try {
    return fn();
  } catch (cause) {
    if (cause instanceof OGError) throw cause;
    if (cause instanceof WebAssembly.RuntimeError) {
      throw new OGError('KernelPanic', call, cause.message);
    }
    if (typeof cause === 'string') {
      try {
        const value = JSON.parse(cause) as { code: string; message: string; details: unknown };
        if (value.code && value.message) throw new OGError(value.code, call, value.message, value.details);
      } catch (parsed) {
        if (parsed instanceof OGError) throw parsed;
      }
    }
    throw new OGError('InvalidGeometry', call, String(cause));
  }
}

export function call<T>(name: string, fn: () => T): T {
  const runtime = runtimeState();
  if (runtime.poisoned) throw new OGError('KernelPanic', name, 'kernel runtime is poisoned');
  try {
    return kernelCall(name, fn);
  } catch (error) {
    if (error instanceof OGError && error.code === 'KernelPanic') {
      runtime.poisoned = true;
      emit('fatal', error);
    }
    throw error;
  }
}
