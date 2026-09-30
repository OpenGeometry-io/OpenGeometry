export type ChangeSet = {
  revision: number;
  added: { og_id: string }[];
  changed: { og_id: string }[];
  removed: { og_id: string }[];
};
