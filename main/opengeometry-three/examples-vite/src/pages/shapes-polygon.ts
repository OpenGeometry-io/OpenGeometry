import { Polygon, Vector3 } from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

function buildPolygonVertices(sides: number, radius: number): Vector3[] {
  const clampedSides = Math.max(3, Math.floor(sides));
  const points: Vector3[] = [];

  for (let i = 0; i < clampedSides; i += 1) {
    const t = (i / clampedSides) * Math.PI * 2;
    const r = i % 2 === 0 ? radius : radius * 0.72;
    points.push(new Vector3(Math.cos(t) * r, 0, Math.sin(t) * r));
  }

  return points;
}

bootstrapExample({
  title: "Shape: Polygon",
  description: "Interactive polygon triangulation with side/radius controls.",
  build: ({ scene }) => {
    let current: Polygon | null = null;

    mountControls(
      "Polygon Parameters",
      [
        { type: "number", key: "sides", label: "Sides", min: 3, max: 12, step: 1, value: 5 },
        { type: "number", key: "radius", label: "Radius", min: 0.4, max: 3, step: 0.05, value: 1.8 },
        { type: "boolean", key: "outline", label: "Outline", value: true },
      ],
      (state) => {
        const polygon = new Polygon({
          vertices: buildPolygonVertices(state.sides as number, state.radius as number),
          color: 0x2563eb,
        });
        polygon.outline = state.outline as boolean;

        current = replaceSceneObject(scene, current, polygon);
      }
    );
  },
});
