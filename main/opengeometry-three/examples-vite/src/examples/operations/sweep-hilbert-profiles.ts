import { Arc, Polyline, Rectangle, Sweep, Vector3 } from "@og-three";
import * as THREE from "three";
import { hilbert3D } from "three/examples/jsm/utils/GeometryUtils.js";
import { defineExample } from "../../shared/example-contract";
import { mountControls, replaceSceneObject } from "../../shared/runtime";

type KernelVertex = {
  position: {
    x: number;
    y: number;
    z: number;
  };
};

type KernelBrep = {
  vertices: KernelVertex[];
};

type ProfileType =
  | "arc-pipe"
  | "rectangle"
  | "architectural-molding"
  | "trapezoid"
  | "diamond"
  | "hexagon";

const LOCKED_HILBERT_SIZE = 8;
const LOCKED_HILBERT_ITERATIONS = 1;

function toVector3List(vertices: KernelVertex[]): Vector3[] {
  return vertices.map((vertex) => {
    const { x, y, z } = vertex.position;
    return new Vector3(x, y, z);
  });
}

function readLineGeometryPoints(object: THREE.Object3D): Vector3[] {
  const geometryOwner = object as THREE.Line;
  const positions = geometryOwner.geometry?.getAttribute("position");
  if (!positions || positions.itemSize !== 3) {
    throw new Error("Profile preview geometry is missing line positions.");
  }

  const points: Vector3[] = [];
  for (let index = 0; index < positions.count; index += 1) {
    points.push(
      new Vector3(
        positions.getX(index),
        positions.getY(index),
        positions.getZ(index)
      )
    );
  }

  if (points.length >= 3) {
    const first = points[0];
    const last = points[points.length - 1];
    const dx = first.x - last.x;
    const dy = first.y - last.y;
    const dz = first.z - last.z;
    if (dx * dx + dy * dy + dz * dz <= 1.0e-12) {
      points.pop();
    }
  }

  return points;
}

function scaleProfileShape(points: Array<[number, number]>, width: number, depth: number): Vector3[] {
  return points.map(([x, z]) => new Vector3(x * width, 0, z * depth));
}

function buildHilbertPath(size: number, iterations: number): Vector3[] {
  const rawPoints = hilbert3D(new THREE.Vector3(0, 0, 0), size, iterations);
  const minY = rawPoints.reduce((lowest, point) => Math.min(lowest, point.y), Number.POSITIVE_INFINITY);
  const lift = Number.isFinite(minY) ? Math.max(0.25 - minY, 0) : 0;

  return rawPoints.map((point) => new Vector3(point.x, point.y + lift, point.z));
}

function createPreviewLoop(points: Vector3[], color: number): THREE.LineLoop {
  const positions: number[] = [];
  for (const point of points) {
    positions.push(point.x, point.y, point.z);
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  return new THREE.LineLoop(
    geometry,
    new THREE.LineBasicMaterial({ color })
  );
}

function buildCustomProfile(profileType: Exclude<ProfileType, "arc-pipe" | "rectangle">, width: number, depth: number): Vector3[] {
  const normalizedPointsByType: Record<Exclude<ProfileType, "arc-pipe" | "rectangle">, Array<[number, number]>> = {
    "architectural-molding": [
      [-0.5, -0.5],
      [0.5, -0.5],
      [0.5, -0.16],
      [0.18, -0.16],
      [0.28, 0.04],
      [0.12, 0.26],
      [-0.08, 0.4],
      [-0.28, 0.5],
      [-0.5, 0.5],
    ],
    trapezoid: [
      [-0.5, -0.5],
      [0.5, -0.5],
      [0.24, 0.5],
      [-0.24, 0.5],
    ],
    diamond: [
      [0, -0.5],
      [0.5, 0],
      [0, 0.5],
      [-0.5, 0],
    ],
    hexagon: [
      [-0.25, -0.5],
      [0.25, -0.5],
      [0.5, 0],
      [0.25, 0.5],
      [-0.25, 0.5],
      [-0.5, 0],
    ],
  };

  return scaleProfileShape(normalizedPointsByType[profileType], width, depth);
}

function buildProfilePoints(profileType: ProfileType, width: number, depth: number): Vector3[] {
  if (profileType === "arc-pipe") {
    const radius = Math.min(width, depth) * 0.5;
    const profilePrimitive = new Arc({
      center: new Vector3(0, 0, 0),
      radius,
      startAngle: 0,
      endAngle: Math.PI * 2,
      segments: 48,
      color: 0xc2410c,
    });

    return readLineGeometryPoints(profilePrimitive);
  }

  if (profileType === "rectangle") {
    const profilePrimitive = new Rectangle({
      center: new Vector3(0, 0, 0),
      width,
      breadth: depth,
      color: 0xc2410c,
    });

    const profileBrep = profilePrimitive.getBrep() as KernelBrep;
    return toVector3List(profileBrep.vertices);
  }

  return buildCustomProfile(profileType, width, depth);
}

export default defineExample({
  slug: "operations/sweep-hilbert-profiles",
  category: "operations",
  title: "Sweep Hilbert Profiles",
  description: "Locked Hilbert3D path with switchable kernel and custom section profiles.",
  statusLabel: "ready",
  chips: ["Hilbert Path", "Profiles", "Sweep"],
  footerText: "Profile Type, Caps, Outlines",
  build: ({ scene }) => {
    let current: THREE.Group | null = null;
    let cachedProfileKey = "";
    let cachedProfilePoints: Vector3[] = [];
    const lockedPathPoints = buildHilbertPath(
      LOCKED_HILBERT_SIZE,
      LOCKED_HILBERT_ITERATIONS
    );

    function getProfilePoints(profileType: ProfileType, width: number, depth: number): Vector3[] {
      const nextKey = `${profileType}:${width}:${depth}`;
      if (nextKey !== cachedProfileKey) {
        cachedProfileKey = nextKey;
        cachedProfilePoints = buildProfilePoints(profileType, width, depth);
      }

      return cachedProfilePoints.map((point) => point.clone());
    }

    mountControls(
      "Hilbert Sweep Parameters",
      [
        {
          type: "select",
          key: "profileType",
          label: "Profile Type",
          value: "arc-pipe",
          options: [
            { label: "Arc Pipe", value: "arc-pipe" },
            { label: "Rectangle", value: "rectangle" },
            { label: "Architectural Molding", value: "architectural-molding" },
            { label: "Trapezoid", value: "trapezoid" },
            { label: "Diamond", value: "diamond" },
            { label: "Hexagon", value: "hexagon" },
          ],
        },
        { type: "number", key: "profileWidth", label: "Profile Width", min: 0.15, max: 1.5, step: 0.05, value: 0.6 },
        { type: "number", key: "profileDepth", label: "Profile Depth", min: 0.15, max: 1.5, step: 0.05, value: 0.45 },
        { type: "boolean", key: "capStart", label: "Cap Start", value: true },
        { type: "boolean", key: "capEnd", label: "Cap End", value: true },
        { type: "boolean", key: "outline", label: "Outline", value: false },
        { type: "boolean", key: "fatOutlines", label: "Fat Outlines", value: false },
        { type: "number", key: "outlineWidth", label: "Outline Width", min: 1, max: 12, step: 0.5, value: 4 },
      ],
      (state) => {
        const profileType = state.profileType as ProfileType;
        const profileWidth = state.profileWidth as number;
        const profileDepth = state.profileDepth as number;
        const pathPoints = lockedPathPoints.map((point) => point.clone());
        const profilePoints = getProfilePoints(profileType, profileWidth, profileDepth);

        const pathPrimitive = new Polyline({
          points: pathPoints.map((point) => point.clone()),
          color: 0x4b5563,
        });

        const previewObject = createPreviewLoop(profilePoints, 0xc2410c);

        const sweep = new Sweep({
          path: pathPoints.map((point) => point.clone()),
          profile: profilePoints.map((point) => point.clone()),
          color: 0x0f766e,
          capStart: state.capStart as boolean,
          capEnd: state.capEnd as boolean,
          fatOutlines: state.fatOutlines as boolean,
          outlineWidth: state.outlineWidth as number,
        });
        sweep.outline = state.outline as boolean;

        const previewOffset = LOCKED_HILBERT_SIZE * 0.95;
        pathPrimitive.position.y += 0.01;
        previewObject.position.set(-previewOffset, 0.02, -previewOffset);

        const group = new THREE.Group();
        group.add(pathPrimitive);
        group.add(previewObject);
        group.add(sweep);

        current = replaceSceneObject(scene, current, group);
      }
    );
  },
});
