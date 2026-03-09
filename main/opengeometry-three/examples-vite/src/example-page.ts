import { bootstrapExample } from "./shared/runtime";
import { getExampleBySlug } from "./shared/examples";

const slug = document.body.dataset.exampleSlug;

if (!slug) {
  throw new Error("Missing example slug on page body");
}

const example = getExampleBySlug(slug);

void bootstrapExample({
  example,
  build: example.build,
});
