export type ChangeSet = {
  revision: number;
  added: { ogId: string }[];
  changed: { ogId: string }[];
  removed: { ogId: string }[];
};
