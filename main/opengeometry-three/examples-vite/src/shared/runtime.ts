import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { OpenGeometry } from "@og-three";
import "../styles/theme.css";

export interface ExampleContext {
  scene: THREE.Scene;
  camera: THREE.PerspectiveCamera;
  renderer: THREE.WebGLRenderer;
  controls: OrbitControls;
}

export type ExampleControlDefinition =
  | {
      type: "number";
      key: string;
      label: string;
      min: number;
      max: number;
      step?: number;
      value: number;
    }
  | {
      type: "boolean";
      key: string;
      label: string;
      value: boolean;
    };

export type ExampleControlState = Record<string, number | boolean>;

interface BootstrapConfig {
  title: string;
  description: string;
  build: (ctx: ExampleContext) => void | Promise<void>;
}

function getWasmUrl(): string {
  // Resolve from the built runtime chunk location (`examples-dist/assets/...`)
  // so nested example pages (e.g. `shapes/sphere.html`) do not request
  // `shapes/assets/...` and 404.
  if (import.meta.env.PROD) {
    return new URL("./wasm/opengeometry_bg.wasm", import.meta.url).toString();
  }

  // Dev server fallback.
  return new URL(
    "../../../../opengeometry/pkg/opengeometry_bg.wasm",
    import.meta.url
  ).toString();
}

export async function bootstrapExample(config: BootstrapConfig) {
  const app = document.getElementById("app");
  if (!app) {
    throw new Error("Missing #app container");
  }
  document.body.classList.add("og-example-page");

  const badge = document.createElement("div");
  badge.className = "og-badge";
  badge.innerHTML = `
    <div class="og-badge-kicker">OPEN GEOMETRY • SPEC VIEW</div>
    <strong class="og-badge-title">${config.title}</strong>
    <span class="og-badge-desc">${config.description}</span>
    <a class="og-badge-link" href="../index.html">All Example Specs</a>
  `;
  document.body.appendChild(badge);

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0xf3f4f6);

  const camera = new THREE.PerspectiveCamera(
    55,
    window.innerWidth / window.innerHeight,
    0.1,
    4000
  );
  camera.position.set(5.5, 4.2, 6.5);

  const renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setPixelRatio(window.devicePixelRatio);
  renderer.setSize(window.innerWidth, window.innerHeight);
  app.appendChild(renderer.domElement);

  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.target.set(0, 0.8, 0);
  controls.update();

  scene.add(new THREE.GridHelper(32, 32, 0x9ca3af, 0xd1d5db));

  const ambient = new THREE.AmbientLight(0xffffff, 0.65);
  scene.add(ambient);

  const key = new THREE.DirectionalLight(0xffffff, 0.85);
  key.position.set(6, 8, 4);
  scene.add(key);

  const fill = new THREE.DirectionalLight(0xffffff, 0.35);
  fill.position.set(-5, 3, -6);
  scene.add(fill);

  await OpenGeometry.create({ wasmURL: getWasmUrl() });
  await config.build({ scene, camera, renderer, controls });

  function onResize() {
    camera.aspect = window.innerWidth / window.innerHeight;
    camera.updateProjectionMatrix();
    renderer.setSize(window.innerWidth, window.innerHeight);
  }

  window.addEventListener("resize", onResize);

  function animate() {
    requestAnimationFrame(animate);
    controls.update();
    renderer.render(scene, camera);
  }

  animate();
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function disposeMaterial(material: unknown) {
  if (!material) {
    return;
  }

  if (Array.isArray(material)) {
    material.forEach((entry) => {
      const candidate = entry as { dispose?: () => void };
      candidate.dispose?.();
    });
    return;
  }

  const candidate = material as { dispose?: () => void };
  candidate.dispose?.();
}

function disposeObject3D(object: THREE.Object3D) {
  object.traverse((node) => {
    const withGeometry = node as { geometry?: { dispose?: () => void } };
    const withMaterial = node as { material?: unknown };
    withGeometry.geometry?.dispose?.();
    disposeMaterial(withMaterial.material);
  });
}

export function replaceSceneObject<T extends THREE.Object3D>(
  scene: THREE.Scene,
  previous: T | null,
  next: T
): T {
  if (previous) {
    previous.parent?.remove(previous);
    disposeObject3D(previous);
  }

  scene.add(next);
  return next;
}

export function mountControls(
  title: string,
  definitions: ExampleControlDefinition[],
  onChange: (state: ExampleControlState) => void
) {
  const panel = document.createElement("aside");
  panel.className = "og-controls";

  const heading = document.createElement("h3");
  heading.textContent = title;
  panel.appendChild(heading);

  const state: ExampleControlState = {};
  for (const definition of definitions) {
    state[definition.key] = definition.value;
  }

  const emitChange = () => {
    onChange({ ...state });
  };

  for (const definition of definitions) {
    const row = document.createElement("label");
    row.className = "og-control-row";

    const header = document.createElement("div");
    header.className = "og-control-header";

    const label = document.createElement("span");
    label.className = "og-control-label";
    label.textContent = definition.label;
    header.appendChild(label);

    if (definition.type === "number") {
      const valueLabel = document.createElement("code");
      valueLabel.className = "og-control-value";
      valueLabel.textContent = definition.value.toFixed(3).replace(/\.?0+$/, "");
      header.appendChild(valueLabel);
      row.appendChild(header);

      const inputs = document.createElement("div");
      inputs.className = "og-control-range";

      const rangeInput = document.createElement("input");
      rangeInput.type = "range";
      rangeInput.min = String(definition.min);
      rangeInput.max = String(definition.max);
      rangeInput.step = String(definition.step ?? 0.01);
      rangeInput.value = String(definition.value);

      const numberInput = document.createElement("input");
      numberInput.type = "number";
      numberInput.min = String(definition.min);
      numberInput.max = String(definition.max);
      numberInput.step = String(definition.step ?? 0.01);
      numberInput.value = String(definition.value);

      const updateValue = (raw: number) => {
        const next = clamp(raw, definition.min, definition.max);
        state[definition.key] = next;
        rangeInput.value = String(next);
        numberInput.value = String(next);
        valueLabel.textContent = next.toFixed(3).replace(/\.?0+$/, "");
        emitChange();
      };

      rangeInput.addEventListener("input", () => {
        updateValue(Number(rangeInput.value));
      });

      numberInput.addEventListener("change", () => {
        updateValue(Number(numberInput.value));
      });

      inputs.appendChild(rangeInput);
      inputs.appendChild(numberInput);
      row.appendChild(inputs);
    } else {
      const boolWrap = document.createElement("div");
      boolWrap.className = "og-control-bool";

      const toggle = document.createElement("input");
      toggle.type = "checkbox";
      toggle.className = "og-toggle";
      toggle.checked = definition.value;
      toggle.setAttribute("aria-label", definition.label);

      const boolLabel = document.createElement("span");
      boolLabel.className = "og-control-bool-state";
      boolLabel.textContent = definition.value ? "Enabled" : "Disabled";

      const updateToggle = () => {
        state[definition.key] = toggle.checked;
        boolLabel.textContent = toggle.checked ? "Enabled" : "Disabled";
        emitChange();
      };
      toggle.addEventListener("change", updateToggle);

      boolWrap.appendChild(toggle);
      boolWrap.appendChild(boolLabel);
      row.appendChild(header);
      row.appendChild(boolWrap);
    }

    panel.appendChild(row);
  }

  document.body.appendChild(panel);
  emitChange();

  return () => {
    panel.remove();
  };
}
