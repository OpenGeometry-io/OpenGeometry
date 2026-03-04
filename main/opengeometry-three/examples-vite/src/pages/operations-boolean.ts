import * as THREE from "three";
import { Vector3 } from "../../../../opengeometry/pkg/opengeometry";
import {
  BooleanOperation,
  BooleanShape,
  Cuboid,
  Cylinder,
  Sphere,
  Wedge,
} from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

function buildOperand(kind: string, center: Vector3, color: number): THREE.Mesh {
  switch (kind) {
    case "sphere": {
      const sphere = new Sphere({
        center,
        radius: 0.9,
        widthSegments: 30,
        heightSegments: 20,
        color,
      });
      sphere.outline = true;
      return sphere;
    }
    case "cylinder": {
      const cylinder = new Cylinder({
        center,
        radius: 0.7,
        height: 1.9,
        segments: 32,
        angle: Math.PI * 2,
        color,
      });
      cylinder.outline = true;
      return cylinder;
    }
    case "wedge": {
      const wedge = new Wedge({
        center,
        width: 1.6,
        height: 1.8,
        depth: 1.4,
        color,
      });
      wedge.outline = true;
      return wedge;
    }
    default: {
      const cuboid = new Cuboid({
        center,
        width: 1.6,
        height: 1.8,
        depth: 1.5,
        color,
      });
      cuboid.outline = true;
      return cuboid;
    }
  }
}

void bootstrapExample({
  title: "Boolean (Union / Intersection / Difference)",
  description:
    "Boolean operations over Cuboid, Sphere, Cylinder, and Wedge operands with kernel-generated outlines.",
  build: ({ scene }) => {
    let result: THREE.Mesh | null = null;

    const update = (state: Record<string, number | boolean | string>) => {
      const left = buildOperand(
        String(state.leftShape),
        new Vector3(-0.55, 0.9, 0),
        0x10b981
      );
      const right = buildOperand(
        String(state.rightShape),
        new Vector3(0.55, 0.9, 0),
        0xf97316
      );

      const operation = (String(state.operation) as BooleanOperation) ??
        BooleanOperation.Union;

      const boolean = new BooleanShape(left, right, operation, {
        epsilon: state.epsilon as number,
        snap: state.snap as number,
      });
      boolean.outline = true;

      boolean.material = new THREE.MeshStandardMaterial({
        color: 0x2563eb,
        transparent: true,
        opacity: 0.72,
      });

      scene.add(left);
      scene.add(right);
      result = replaceSceneObject(scene, result, boolean);

      left.removeFromParent();
      right.removeFromParent();
    };

    mountControls(
      "Boolean Controls",
      [
        {
          type: "select",
          key: "operation",
          label: "Operation",
          value: BooleanOperation.Union,
          options: [
            { label: "Union", value: BooleanOperation.Union },
            { label: "Intersection", value: BooleanOperation.Intersection },
            { label: "Difference", value: BooleanOperation.Difference },
          ],
        },
        {
          type: "select",
          key: "leftShape",
          label: "Left Shape",
          value: "cuboid",
          options: [
            { label: "Cuboid", value: "cuboid" },
            { label: "Sphere", value: "sphere" },
            { label: "Cylinder", value: "cylinder" },
            { label: "Wedge", value: "wedge" },
          ],
        },
        {
          type: "select",
          key: "rightShape",
          label: "Right Shape",
          value: "sphere",
          options: [
            { label: "Cuboid", value: "cuboid" },
            { label: "Sphere", value: "sphere" },
            { label: "Cylinder", value: "cylinder" },
            { label: "Wedge", value: "wedge" },
          ],
        },
        {
          type: "number",
          key: "epsilon",
          label: "Plane Epsilon",
          min: 0.000001,
          max: 0.005,
          step: 0.000001,
          value: 0.00001,
        },
        {
          type: "number",
          key: "snap",
          label: "Snap Grid",
          min: 0.000001,
          max: 0.005,
          step: 0.000001,
          value: 0.00001,
        },
      ],
      update
    );
  },
});
