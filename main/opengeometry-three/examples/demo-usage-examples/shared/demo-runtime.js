import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { OpenGeometry } from "../../../../../dist/index.js";

function getWasmUrl() {
  return new URL("../../../../../dist/opengeometry_bg.wasm", import.meta.url).toString();
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function disposeMaterial(material) {
  if (!material) {
    return;
  }

  if (Array.isArray(material)) {
    material.forEach((entry) => entry.dispose?.());
    return;
  }

  material.dispose?.();
}

function disposeObject3D(object) {
  object.traverse((node) => {
    node.geometry?.dispose?.();
    disposeMaterial(node.material);
  });
}

function replaceSceneObject(scene, previous, next) {
  if (previous) {
    previous.parent?.remove(previous);
    disposeObject3D(previous);
  }

  scene.add(next);
  return next;
}

function colorToHex(value) {
  return `#${(value >>> 0).toString(16).padStart(6, "0")}`.slice(0, 7);
}

function hexToColor(hex) {
  return Number.parseInt(hex.replace("#", ""), 16);
}

function toThreePoint(point) {
  return new THREE.Vector3(point.x, point.y, point.z);
}

function toThreePoints(points) {
  return points.map((point) => toThreePoint(point));
}

export function createPathGuide(points, color = 0x475569) {
  const geometry = new THREE.BufferGeometry().setFromPoints(toThreePoints(points));
  const material = new THREE.LineBasicMaterial({ color });
  return new THREE.Line(geometry, material);
}

export function createProfileGuide(points, origin = new THREE.Vector3(-3, 0, -3), color = 0x0f172a) {
  const translated = points.map((point) => {
    const p = toThreePoint(point);
    p.add(origin);
    return p;
  });

  if (translated.length > 2) {
    translated.push(translated[0].clone());
  }

  const geometry = new THREE.BufferGeometry().setFromPoints(translated);
  const material = new THREE.LineBasicMaterial({ color });
  return new THREE.Line(geometry, material);
}

function mountControls(title, definitions, onChange) {
  const panel = document.createElement("aside");
  panel.className = "og-demo-controls";

  const heading = document.createElement("h3");
  heading.textContent = title;
  panel.appendChild(heading);

  const state = {};
  definitions.forEach((definition) => {
    state[definition.key] = definition.value;
  });

  const emit = () => onChange({ ...state });

  for (const definition of definitions) {
    const row = document.createElement("label");
    row.className = "og-demo-control-row";

    const header = document.createElement("div");
    header.className = "og-demo-control-header";

    const label = document.createElement("span");
    label.className = "og-demo-control-label";
    label.textContent = definition.label;
    header.appendChild(label);

    if (definition.type === "number") {
      const value = document.createElement("code");
      value.className = "og-demo-control-value";
      value.textContent = Number(definition.value).toFixed(3).replace(/\.?0+$/, "");
      header.appendChild(value);
      row.appendChild(header);

      const rangeWrap = document.createElement("div");
      rangeWrap.className = "og-demo-control-range";

      const range = document.createElement("input");
      range.type = "range";
      range.min = String(definition.min);
      range.max = String(definition.max);
      range.step = String(definition.step ?? 0.01);
      range.value = String(definition.value);

      const number = document.createElement("input");
      number.type = "number";
      number.min = String(definition.min);
      number.max = String(definition.max);
      number.step = String(definition.step ?? 0.01);
      number.value = String(definition.value);

      const update = (raw) => {
        const next = clamp(Number(raw), definition.min, definition.max);
        state[definition.key] = next;
        range.value = String(next);
        number.value = String(next);
        value.textContent = next.toFixed(3).replace(/\.?0+$/, "");
        emit();
      };

      range.addEventListener("input", () => update(range.value));
      number.addEventListener("change", () => update(number.value));

      rangeWrap.appendChild(range);
      rangeWrap.appendChild(number);
      row.appendChild(rangeWrap);
    } else if (definition.type === "boolean") {
      row.appendChild(header);

      const boolWrap = document.createElement("div");
      boolWrap.className = "og-demo-control-bool";

      const toggle = document.createElement("input");
      toggle.type = "checkbox";
      toggle.checked = Boolean(definition.value);
      toggle.className = "og-demo-toggle";
      toggle.setAttribute("aria-label", definition.label);

      const status = document.createElement("span");
      status.textContent = toggle.checked ? "Enabled" : "Disabled";

      const update = () => {
        state[definition.key] = toggle.checked;
        status.textContent = toggle.checked ? "Enabled" : "Disabled";
        emit();
      };

      toggle.addEventListener("change", update);

      boolWrap.appendChild(toggle);
      boolWrap.appendChild(status);
      row.appendChild(boolWrap);
    } else if (definition.type === "color") {
      const hex = document.createElement("code");
      hex.className = "og-demo-control-value";
      hex.textContent = colorToHex(definition.value);
      header.appendChild(hex);
      row.appendChild(header);

      const colorWrap = document.createElement("div");
      colorWrap.className = "og-demo-control-color";

      const picker = document.createElement("input");
      picker.type = "color";
      picker.value = colorToHex(definition.value);

      const update = () => {
        const next = hexToColor(picker.value);
        state[definition.key] = next;
        hex.textContent = picker.value;
        emit();
      };

      picker.addEventListener("input", update);

      colorWrap.appendChild(picker);
      row.appendChild(colorWrap);
    }

    panel.appendChild(row);
  }

  document.body.appendChild(panel);
  emit();
}

function addFrame(scene) {
  const grid = new THREE.GridHelper(36, 36, 0x9ca3af, 0xd1d5db);
  scene.add(grid);

  const ambient = new THREE.AmbientLight(0xffffff, 0.68);
  scene.add(ambient);

  const key = new THREE.DirectionalLight(0xffffff, 0.9);
  key.position.set(7, 9, 5);
  scene.add(key);

  const fill = new THREE.DirectionalLight(0xffffff, 0.4);
  fill.position.set(-6, 4, -5);
  scene.add(fill);
}

export async function bootstrapSweepDemo(config) {
  const app = document.getElementById("app");
  if (!app) {
    throw new Error("Missing #app container");
  }

  document.title = config.title;
  document.body.classList.add("og-demo-page");

  const badge = document.createElement("div");
  badge.className = "og-demo-badge";
  badge.innerHTML = `
    <div class="og-demo-kicker">OPEN GEOMETRY • SWEEP USAGE</div>
    <strong class="og-demo-title">${config.title}</strong>
    <span class="og-demo-desc">${config.description}</span>
    <div class="og-demo-links">
      <a href="./index.html">Usage Index</a>
      <a href="../sweep.html">Legacy Sweep Example</a>
    </div>
  `;
  document.body.appendChild(badge);

  const status = document.createElement("div");
  status.className = "og-demo-status";
  status.textContent = "Initializing kernel...";
  document.body.appendChild(status);

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0xf3f4f6);

  const camera = new THREE.PerspectiveCamera(55, window.innerWidth / window.innerHeight, 0.1, 5000);
  const cameraPos = config.cameraPosition ?? [8, 6, 9];
  camera.position.set(cameraPos[0], cameraPos[1], cameraPos[2]);

  const renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setPixelRatio(window.devicePixelRatio);
  renderer.setSize(window.innerWidth, window.innerHeight);
  app.appendChild(renderer.domElement);

  const orbit = new OrbitControls(camera, renderer.domElement);
  orbit.enableDamping = true;
  const target = config.controlsTarget ?? [0, 1, 0];
  orbit.target.set(target[0], target[1], target[2]);
  orbit.update();

  addFrame(scene);

  await OpenGeometry.create({ wasmURL: getWasmUrl() });
  status.textContent = "Kernel ready";

  let current = null;

  mountControls(config.panelTitle, config.controls, (state) => {
    try {
      const next = config.createSceneObject({ state, THREE, scene });
      current = replaceSceneObject(scene, current, next);
      status.textContent = "Geometry rebuilt";
      status.classList.remove("og-demo-status-error");
    } catch (error) {
      status.textContent = error instanceof Error ? error.message : "Failed to rebuild geometry";
      status.classList.add("og-demo-status-error");
      console.error(error);
    }
  });

  function onResize() {
    camera.aspect = window.innerWidth / window.innerHeight;
    camera.updateProjectionMatrix();
    renderer.setSize(window.innerWidth, window.innerHeight);
  }

  window.addEventListener("resize", onResize);

  function animate() {
    requestAnimationFrame(animate);
    orbit.update();
    renderer.render(scene, camera);
  }

  animate();
}
