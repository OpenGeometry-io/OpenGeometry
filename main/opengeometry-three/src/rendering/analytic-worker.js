let runtime;
let initialization;
let totalBytes = 0;
const models = new Map();
const jobs = new Map();
let scheduled = false;

function respond(request, result, transfer = []) {
  self.postMessage({ request, result }, transfer);
}
function reject(request, error) {
  self.postMessage({ request, error: error instanceof Error ? error.message : error });
}
function drain() {
  scheduled = false;
  const entry = jobs.entries().next();
  if (entry.done) return;
  const [handle, job] = entry.value;
  jobs.delete(handle);
  let mesh;
  try {
    const model = models.get(handle);
    if (!model) throw new Error("Unknown analytic worker model handle");
    mesh = model.brep.tessellate(job.deflection);
    const result = {
      positions: mesh.positions(), normals: mesh.normals(), indices: mesh.indices(),
      triangleFaceIds: mesh.triangle_face_ids(), outlinePositions: mesh.outline_positions(),
      outlineEdgeIds: mesh.outline_edge_ids(), revision: mesh.revision(), achievedDeflection: mesh.achieved_deflection(),
    };
    const transfer = [result.positions, result.normals, result.indices, result.triangleFaceIds,
      result.outlinePositions, result.outlineEdgeIds].map((array) => array.buffer);
    respond(job.request, result, transfer);
  } catch (error) { reject(job.request, error); }
  finally { mesh?.free(); }
  schedule();
}
function schedule() {
  if (scheduled || !jobs.size) return;
  scheduled = true;
  setTimeout(drain, 0);
}
function invalidate(handle) {
  const job = jobs.get(handle);
  if (job) {
    reject(job.request, "Superseded tessellation request");
    jobs.delete(handle);
  }
}
async function receive(message) {
  const { request, kind, handle } = message;
  try {
    if (kind === "init") {
      if (initialization) throw new Error("Analytic worker already initialized");
      initialization = (async () => {
      runtime = await import(/* @vite-ignore */ message.kernelURL);
        await runtime.default({ module_or_path: message.wasmURL });
      })();
      await initialization;
      respond(request, true);
      return;
    }
    if (!initialization) throw new Error("Analytic worker has not been initialized");
    await initialization;
    if (typeof handle !== "string" || !handle || handle.length > 1024) throw new Error("Invalid analytic worker handle");
    if (kind === "model") {
      const bytes = typeof message.serialized === "string" ? new TextEncoder().encode(message.serialized).length : Infinity;
      const previous = models.get(handle);
      if (bytes > 64 * 1024 * 1024 || totalBytes - (previous?.bytes ?? 0) + bytes > 128 * 1024 * 1024
        || (!previous && models.size >= 64)) throw new Error("Analytic worker model resource limit exceeded");
      const brep = new runtime.OGAnalyticBrep(message.serialized);
      invalidate(handle);
      previous?.brep.free();
      models.set(handle, { brep, bytes });
      totalBytes += bytes - (previous?.bytes ?? 0);
      respond(request, { revision: brep.get_revision() });
    } else if (kind === "remove") {
      invalidate(handle);
      const previous = models.get(handle);
      if (previous) { previous.brep.free(); totalBytes -= previous.bytes; models.delete(handle); }
      respond(request, true);
    } else if (kind === "tessellate") {
      if (!models.has(handle)) throw new Error("Unknown analytic worker model handle");
      if (!Number.isFinite(message.deflection) || message.deflection <= 0) throw new Error("Invalid tessellation deflection");
      invalidate(handle);
      jobs.set(handle, message);
      schedule();
    } else throw new Error("Unknown analytic worker operation");
  } catch (error) { reject(request, error); }
}
self.onmessage = (event) => { void receive(event.data); };
