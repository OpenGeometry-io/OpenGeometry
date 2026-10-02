export function requiredElement<T extends Element>(
  selector: string,
  type: abstract new () => T,
  parent: ParentNode = document,
): T {
  const element = parent.querySelector(selector);
  if (!(element instanceof type)) throw new Error(`Example page has no ${type.name} at ${selector}`);
  return element;
}
