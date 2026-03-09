import "./styles/theme.css";
import { categoryLabels, getExamplesByCategory } from "./shared/examples";
import { getExampleIconMarkup } from "./shared/icon-registry";
import type { ExampleCategory } from "./shared/example-contract";

const app = document.getElementById("app");

if (!app) {
  throw new Error("Missing #app container");
}

const shell = document.createElement("main");
shell.className = "og-specs-shell";

const head = document.createElement("header");
head.className = "og-specs-head";
head.innerHTML = `
  <div>
    <p class="og-specs-note">OpenGeometry Technical Sandbox</p>
    <h1 class="og-specs-title">Examples</h1>
  </div>
  <p class="og-specs-note">Build command: <code>npm --prefix main/opengeometry-three run build-example-three</code></p>
`;
shell.appendChild(head);

for (const category of ["primitives", "shapes", "operations"] as ExampleCategory[]) {
  const items = getExamplesByCategory(category);
  const section = document.createElement("section");
  section.className = "og-specs-section";

  const heading = document.createElement("div");
  heading.className = "og-specs-section-head";
  heading.innerHTML = `
    <h2 class="og-specs-section-title">${categoryLabels[category]}</h2>
    <p class="og-specs-count">${items.length} items</p>
  `;
  section.appendChild(heading);

  const grid = document.createElement("div");
  grid.className = "og-specs-grid";

  for (const example of items) {
    const card = document.createElement("article");
    card.className = "og-spec-card";
    const chips = example.chips
      .map((chip) => `<div class="og-chip"><span>${chip}</span></div>`)
      .join("");

    card.innerHTML = `
      <div class="og-spec-card-head">
        <div class="og-spec-card-brand">${getExampleIconMarkup(example.slug)}</div>
        <p class="og-spec-badge">${example.statusLabel}</p>
      </div>
      <div>
        <h3 class="og-spec-card-title">${example.title}</h3>
        <p class="og-spec-desc">${example.description}</p>
      </div>
      <div class="og-spec-chip-row">${chips}</div>
      <div class="og-spec-card-footer">
        <a class="og-spec-open" href="./${example.slug}.html">Open example</a>
      </div>
    `;

    grid.appendChild(card);
  }

  section.appendChild(grid);
  shell.appendChild(section);
}

app.replaceChildren(shell);
