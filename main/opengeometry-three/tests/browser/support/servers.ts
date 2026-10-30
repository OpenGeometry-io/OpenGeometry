export const PORT_BASE = Number(process.env.OG_PW_PORT_BASE ?? '4175');
export const EXAMPLES_URL = `http://127.0.0.1:${String(PORT_BASE)}`;
export const PAGES_URL = `http://127.0.0.1:${String(PORT_BASE + 1)}`;
