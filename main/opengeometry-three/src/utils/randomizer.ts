import { v4 as uuidv4 } from 'uuid';

export function getUUID() {
  const time = performance.now() * 1000;
  const random = Math.random() * 1000;

  const uuid = Math.floor(time + random);
  return uuid;
}
