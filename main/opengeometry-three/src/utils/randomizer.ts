import { v4 as uuidv4 } from 'uuid';

export function getUUID() {
  const uuid = uuidv4();
  return uuid;
}
