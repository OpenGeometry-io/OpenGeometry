import { Polygon, Vector3 } from "@og-three";
import Stats from "three/examples/jsm/libs/stats.module.js";
import GUI from "three/examples/jsm/libs/lil-gui.module.min.js";
import { defineExample } from "../../shared/example-contract";
import { replaceSceneObject } from "../../shared/runtime";

type LoopPoint = [number, number, number];

type PolygonDatasetEntry = {
  vertices: LoopPoint[];
  holes?: LoopPoint[][];
  description: string;
  category: string;
};

const polygonDataset: Record<string, PolygonDatasetEntry> = {
  Triangle: {
    vertices: [
      [0, 0, 0],
      [3, 0, 0],
      [1.5, 0, 3],
    ],
    description: "Basic triangle test for the smallest valid polygon footprint.",
    category: "Basic Shapes",
  },
  Square: {
    vertices: [
      [-2, 0, -2],
      [2, 0, -2],
      [2, 0, 2],
      [-2, 0, 2],
    ],
    description: "Simple rectangular slab used as the baseline fill and outline case.",
    category: "Basic Shapes",
  },
  L_Shape: {
    vertices: [
      [0, 0, 0],
      [6, 0, 0],
      [6, 0, 4],
      [10, 0, 4],
      [10, 0, 10],
      [0, 0, 10],
    ],
    description: "Concave architectural footprint for notch handling and triangulation checks.",
    category: "Concave Cases",
  },
  Concave_Polygon: {
    vertices: [
      [0, 0, 0],
      [3, 0, 0],
      [4, 0, 2],
      [6, 0, 0],
      [10, 0, 0],
      [10, 0, 3],
      [8, 0, 4],
      [10, 0, 6],
      [10, 0, 10],
      [6, 0, 10],
      [4, 0, 8],
      [3, 0, 10],
      [0, 0, 10],
      [0, 0, 6],
      [2, 0, 4],
      [0, 0, 3],
    ],
    description: "Complex concave polygon with multiple indentations for triangulation stress.",
    category: "Concave Cases",
  },
  Complex_Multi_Hole: {
    vertices: [
      [-12, 0, -8],
      [12, 0, -8],
      [12, 0, 8],
      [-12, 0, 8],
    ],
    holes: [
      [
        [-8, 0, -5],
        [-8, 0, 1],
        [-3, 0, 1],
        [-3, 0, -5],
      ],
      [
        [2, 0, -5],
        [5, 0, -4],
        [6, 0, -1],
        [5, 0, 2],
        [2, 0, 1],
        [1, 0, -2],
      ],
      [
        [0, 0, 4],
        [0, 0, 6.5],
        [5, 0, 6.5],
        [5, 0, 5],
        [2, 0, 5],
        [2, 0, 4],
      ],
    ],
    description: "Rectangular slab with three interior voids to validate multi-hole redraw stability.",
    category: "Hole Test Suite",
  },
  Highly_Complex: {
    vertices: [
      [0, 0, 0],
      [1, 0, 0],
      [2, 0, 1],
      [3, 0, 0],
      [4, 0, 1],
      [5, 0, 0],
      [6, 0, 1],
      [7, 0, 0],
      [8, 0, 1],
      [9, 0, 0],
      [10, 0, 1],
      [11, 0, 2],
      [12, 0, 1],
      [13, 0, 2],
      [14, 0, 3],
      [15, 0, 4],
      [16, 0, 5],
      [17, 0, 6],
      [18, 0, 7],
      [19, 0, 8],
      [20, 0, 9],
      [19, 0, 10],
      [18, 0, 9],
      [17, 0, 8],
      [16, 0, 7],
      [15, 0, 6],
      [14, 0, 5],
      [13, 0, 4],
      [12, 0, 3],
      [11, 0, 2],
      [10, 0, 3],
      [9, 0, 2],
      [8, 0, 3],
      [7, 0, 2],
      [6, 0, 3],
      [5, 0, 2],
      [4, 0, 3],
      [3, 0, 2],
      [2, 0, 3],
      [1, 0, 2],
      [0, 0, 1],
    ],
    description: "Large irregular polygon used to inspect performance and triangulation consistency.",
    category: "Performance Testing",
  },
  Building: {
    vertices: [
      [66.1, 0, 11.2],
      [66.1, 0, 9.6],
      [66.6, 0, 9.6],
      [66.6, 0, 8.7],
      [74.3, 0, 8.7],
      [77.1, 0, 8.7],
      [77.1, 0, 11.4],
      [75.0, 0, 11.4],
      [75.0, 0, 11.3],
      [74.2, 0, 11.3],
      [74.2, 0, 10.6],
      [71.0, 0, 10.6],
      [71.0, 0, 11.3],
      [66.6, 0, 11.3],
      [66.6, 0, 11.2],
    ],
    description: "Architectural outline sampled from a real-world earcut-oriented building footprint.",
    category: "Dataset Cases",
  },
};

function toVector3Loop(points: LoopPoint[]): Vector3[] {
  return points.map((point) => new Vector3(point[0], point[1], point[2]));
}

function applyPolygonMaterial(
  polygon: Polygon,
  color: string,
  opacity: number,
  wireframe: boolean,
  outline: boolean
) {
  const candidate = polygon as unknown as {
    material?: {
      color?: { set: (next: string) => void };
      opacity?: number;
      transparent?: boolean;
      wireframe?: boolean;
    };
  };

  polygon.outline = outline;
  candidate.material?.color?.set(color);
  if (candidate.material) {
    candidate.material.opacity = opacity;
    candidate.material.transparent = opacity < 1;
    candidate.material.wireframe = wireframe;
  }
}

export default defineExample({
  slug: "shapes/polygon-suite",
  category: "shapes",
  title: "Polygon Suite",
  description: "Dataset-backed polygon validation with concave, performance, and multi-hole cases.",
  statusLabel: "ready",
  chips: ["Holes", "Concavity"],
  footerText: "Holes, Concavity",
  build: ({ scene, camera, renderer, controls }) => {
    camera.position.set(0, 20, 20);
    controls.target.set(0, 0, 0);
    controls.update();

    const stats = new Stats();
    stats.dom.id = "stats";
    document.body.appendChild(stats.dom);

    const description = document.createElement("div");
    description.id = "polygon-description";
    document.body.appendChild(description);

    const renderBase = renderer.render.bind(renderer);
    renderer.render = ((renderScene, renderCamera) => {
      stats.begin();
      renderBase(renderScene, renderCamera);
      stats.end();
    }) as typeof renderer.render;

    const info = {
      vertices: 0,
      holes: 0,
      category: "",
    };

    const params = {
      polygonType: "Complex_Multi_Hole",
      showOutline: true,
      polygonColor: "#4CAF50",
      opacity: 0.7,
      wireframe: false,
      showStats: true,
      statsMode: 0,
    };

    let current: Polygon | null = null;

    const updatePolygonDescription = (polygonName: string) => {
      const polygonData = polygonDataset[polygonName];
      if (!polygonData) {
        return;
      }

      const holeSummary = polygonData.holes?.length
        ? `<br><em>Holes: ${polygonData.holes.length}</em>`
        : "";

      description.innerHTML = `
        <strong>${polygonName}</strong><br>
        <em>Category: ${polygonData.category}</em>${holeSummary}<br>
        ${polygonData.description}
      `;
    };

    const createPolygon = (polygonName: string) => {
      const polygonData = polygonDataset[polygonName];
      if (!polygonData) {
        return;
      }

      const polygon = new Polygon({
        vertices: toVector3Loop(polygonData.vertices),
        color: 0x4caf50,
      });

      for (const hole of polygonData.holes ?? []) {
        polygon.addHole(toVector3Loop(hole));
      }

      polygon.position.y = 0.01;
      applyPolygonMaterial(
        polygon,
        params.polygonColor,
        params.opacity,
        params.wireframe,
        params.showOutline
      );

      current = replaceSceneObject(scene, current, polygon);
      info.vertices = polygonData.vertices.length;
      info.holes = polygonData.holes?.length ?? 0;
      info.category = polygonData.category;
      updatePolygonDescription(polygonName);
    };

    const gui = new GUI();
    const polygonFolder = gui.addFolder("Polygon Test Suite");
    polygonFolder.open();
    polygonFolder
      .add(params, "polygonType", Object.keys(polygonDataset))
      .name("Polygon Shape")
      .onChange((value: string) => {
        createPolygon(value);
      });
    polygonFolder
      .add(params, "showOutline")
      .name("Show Outline")
      .onChange((value: boolean) => {
        if (current) {
          applyPolygonMaterial(current, params.polygonColor, params.opacity, params.wireframe, value);
        }
      });
    polygonFolder
      .addColor(params, "polygonColor")
      .name("Polygon Color")
      .onChange((value: string) => {
        if (current) {
          applyPolygonMaterial(current, value, params.opacity, params.wireframe, params.showOutline);
        }
      });
    polygonFolder
      .add(params, "opacity", 0, 1, 0.01)
      .name("Opacity")
      .onChange((value: number) => {
        if (current) {
          applyPolygonMaterial(current, params.polygonColor, value, params.wireframe, params.showOutline);
        }
      });
    polygonFolder
      .add(params, "wireframe")
      .name("Wireframe")
      .onChange((value: boolean) => {
        if (current) {
          applyPolygonMaterial(current, params.polygonColor, params.opacity, value, params.showOutline);
        }
      });

    const performanceFolder = gui.addFolder("Performance");
    performanceFolder.open();
    performanceFolder
      .add(params, "showStats")
      .name("Show FPS Stats")
      .onChange((value: boolean) => {
        stats.dom.style.display = value ? "block" : "none";
      });
    performanceFolder
      .add(params, "statsMode", { FPS: 0, "Frame Time (ms)": 1, "Memory (MB)": 2 })
      .name("Stats Mode")
      .onChange((value: number) => {
        stats.showPanel(Number(value));
      });

    const infoFolder = gui.addFolder("Polygon Info");
    infoFolder.open();
    infoFolder.add(info, "vertices").name("Vertex Count").listen();
    infoFolder.add(info, "holes").name("Hole Count").listen();
    infoFolder.add(info, "category").name("Category").listen();

    createPolygon(params.polygonType);
  },
});
