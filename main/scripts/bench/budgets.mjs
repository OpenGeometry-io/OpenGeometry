const MARGIN = 1.25;

export function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

export function platformKey(env, platform, arch) {
  return env.CI === 'true' ? `${platform}-${arch}-ci` : `${platform}-${arch}`;
}

function sizeFindings(sizes, baseline) {
  return Object.entries(sizes).flatMap(([name, value]) => {
    const budget = baseline[name];
    if (budget === undefined) return [`missing size baseline for ${name}`];
    if (value > budget * MARGIN) return [`${name} regressed: ${String(value)} exceeds ${String(budget * MARGIN)}`];
    return [];
  });
}

function timingReport(timings, block, key) {
  if (block === undefined) return { findings: [], missing: [`missing timing baseline for ${key}`] };
  const entries = Object.entries(timings);
  const missing = entries.filter(([name]) => block[name] === undefined)
    .map(([name]) => `missing timing baseline for ${name} on ${key}`);
  const findings = entries.filter(([name, value]) => block[name] !== undefined && value > block[name] * MARGIN)
    .map(([name, value]) => `${name} regressed: ${String(value)} ms exceeds ${String(block[name] * MARGIN)} ms`);
  return { findings, missing };
}

export function budgetFindings(result, baseline, key, ci) {
  const sizes = sizeFindings(result.sizes, baseline.sizes);
  const timing = timingReport(result.timings, baseline.timings[key], key);
  if (ci) return { findings: [...sizes, ...timing.findings, ...timing.missing], notes: [] };
  return { findings: [...sizes, ...timing.findings], notes: timing.missing };
}
