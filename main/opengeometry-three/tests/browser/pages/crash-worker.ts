import '../../../../dist/tessellation-worker.js';

self.addEventListener('message', (event: MessageEvent<unknown>) => {
  const data = event.data;
  if (typeof data === 'object' && data !== null && Reflect.get(data, 'kind') === 'throw') {
    throw new Error('crash page worker threw on request');
  }
});
