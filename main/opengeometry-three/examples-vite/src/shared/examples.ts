import type { ExampleCategory, ExampleDefinition } from "./example-contract";

export const categoryLabels: Record<ExampleCategory, string> = {
  primitives: "Primitives",
  shapes: "Shapes",
  operations: "Operations",
};

const modules = import.meta.glob("../examples/**/*.ts", { eager: true });

const discoveredExamples = Object.values(modules)
  .map((entry) => (entry as { default?: ExampleDefinition }).default)
  .filter((entry): entry is ExampleDefinition => Boolean(entry));

export const examples = discoveredExamples.sort((left, right) => {
  if (left.category === right.category) {
    return left.title.localeCompare(right.title);
  }

  return left.category.localeCompare(right.category);
});

export function getExampleBySlug(slug: string): ExampleDefinition {
  const match = examples.find((example) => example.slug === slug);
  if (!match) {
    throw new Error(`Unknown example slug: ${slug}`);
  }

  return match;
}

export function getExamplesByCategory(category: ExampleCategory): ExampleDefinition[] {
  return examples.filter((example) => example.category === category);
}
