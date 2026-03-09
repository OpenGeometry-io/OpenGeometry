import type { ExampleContext } from "./runtime";

export type ExampleCategory = "primitives" | "shapes" | "operations";

export interface ExampleDefinition {
  slug: `${ExampleCategory}/${string}`;
  category: ExampleCategory;
  title: string;
  description: string;
  statusLabel: string;
  chips: string[];
  footerText: string;
  build: (ctx: ExampleContext) => void | Promise<void>;
}

export function defineExample(example: ExampleDefinition): ExampleDefinition {
  return example;
}
