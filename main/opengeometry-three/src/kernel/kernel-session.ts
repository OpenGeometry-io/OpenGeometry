import { OGError } from '../errors';
import { emit } from '../runtime/event-bus';
import { runtime as runtimeState } from '../runtime/runtime-state';
import { parseKernelError } from './kernel-errors';
import { takePanicMessage } from './kernel-loader';

export function kernelCall<T>(call: string, fn: () => T): T {
  try {
    return fn();
  } catch (cause) {
    if (cause instanceof OGError) throw cause;
    if (cause instanceof WebAssembly.RuntimeError) {
      throw new OGError('KernelPanic', call, takePanicMessage() ?? cause.message);
    }
    const value = typeof cause === 'string' ? parseKernelError(cause) : undefined;
    if (value) throw new OGError(value.code, call, value.message, value.details);
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
