import { requiredElement } from './dom.js';

export type NumberControl = { range: HTMLInputElement; number: HTMLInputElement; value: HTMLElement };

export function numberControl(name: string): NumberControl {
  const row = requiredElement(`[data-control="${name}"]`, HTMLElement);
  return {
    range: requiredElement('[data-range]', HTMLInputElement, row),
    number: requiredElement('[data-number]', HTMLInputElement, row),
    value: requiredElement('[data-value]', HTMLElement, row),
  };
}

export function onNumberInput(control: NumberControl, accept: (value: number) => void): void {
  const commit = (raw: string): void => {
    const parsed = Number(raw);
    if (raw.trim() === '' || !Number.isFinite(parsed)) return;
    if (parsed < Number(control.range.min) || parsed > Number(control.range.max)) return;
    control.range.value = String(parsed);
    control.number.value = String(parsed);
    control.value.textContent = parsed.toFixed(2).replace(/\.?0+$/, '');
    accept(parsed);
  };
  control.range.addEventListener('input', () => { commit(control.range.value); });
  control.number.addEventListener('input', () => { commit(control.number.value); });
}
