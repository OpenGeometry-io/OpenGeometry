export type Events = 'geometry' | 'warning' | 'error' | 'fatal';
export type Listener = (event: unknown) => void;
