import type { ExampleCategory } from "./example-contract";

export const categoryLabels: Record<ExampleCategory, string> = {
  primitives: "Primitives",
  shapes: "Shapes",
  operations: "Operations",
};

export interface ExampleMetadata {
  slug: string;
  category: ExampleCategory;
  title: string;
  description: string;
  statusLabel: string;
  chips: string[];
  footerText: string;
}

export const examples: ExampleMetadata[] = [
  {
    "slug": "primitives/arc",
    "category": "primitives",
    "title": "Arc",
    "description": "Circular arc with angle span and segmentation control.",
    "statusLabel": "ready",
    "chips": [
      "Radius",
      "Span"
    ],
    "footerText": "Radius, Span"
  },
  {
    "slug": "primitives/curve",
    "category": "primitives",
    "title": "Curve",
    "description": "Control-point curve for route and profile sketching.",
    "statusLabel": "ready",
    "chips": [
      "Sag",
      "Lift"
    ],
    "footerText": "Sag, Lift"
  },
  {
    "slug": "primitives/line",
    "category": "primitives",
    "title": "Line",
    "description": "Two-point line primitive with direct endpoint control.",
    "statusLabel": "ready",
    "chips": [
      "Length",
      "Angle"
    ],
    "footerText": "Length, Angle"
  },
  {
    "slug": "primitives/polyline",
    "category": "primitives",
    "title": "Polyline",
    "description": "Open and closed path definitions for profile work.",
    "statusLabel": "ready",
    "chips": [
      "Closure",
      "Span"
    ],
    "footerText": "Closure, Span"
  },
  {
    "slug": "primitives/rectangle",
    "category": "primitives",
    "title": "Rectangle",
    "description": "Parametric rectangular primitive for base profiles.",
    "statusLabel": "ready",
    "chips": [
      "Width",
      "Breadth"
    ],
    "footerText": "Width, Breadth"
  },
  {
    "slug": "shapes/cuboid",
    "category": "shapes",
    "title": "Cuboid",
    "description": "Rectangular solid for rooms, equipment blocks and massing.",
    "statusLabel": "ready",
    "chips": [
      "Width",
      "Height",
      "Depth"
    ],
    "footerText": "Width, Height, Depth"
  },
  {
    "slug": "shapes/cylinder",
    "category": "shapes",
    "title": "Cylinder",
    "description": "Cylindrical volume for ducts, pipes and mechanical shafts.",
    "statusLabel": "ready",
    "chips": [
      "Radius",
      "Height",
      "Segments"
    ],
    "footerText": "Radius, Height, Segments"
  },
  {
    "slug": "shapes/opening",
    "category": "shapes",
    "title": "Opening",
    "description": "Opening helper volume for void and penetration previews.",
    "statusLabel": "ready",
    "chips": [
      "Width",
      "Height",
      "Depth"
    ],
    "footerText": "Width, Height, Depth"
  },
  {
    "slug": "shapes/polygon",
    "category": "shapes",
    "title": "Polygon",
    "description": "Planar polygon triangulation for surfaces and slabs.",
    "statusLabel": "ready",
    "chips": [
      "Sides",
      "Radius"
    ],
    "footerText": "Sides, Radius"
  },
  {
    "slug": "shapes/polygon-suite",
    "category": "shapes",
    "title": "Polygon Suite",
    "description": "Dataset-backed polygon validation with concave, performance, and multi-hole cases.",
    "statusLabel": "ready",
    "chips": [
      "Holes",
      "Concavity"
    ],
    "footerText": "Holes, Concavity"
  },
  {
    "slug": "shapes/sphere",
    "category": "shapes",
    "title": "Sphere",
    "description": "UV sphere for equipment envelopes and clearance studies.",
    "statusLabel": "ready",
    "chips": [
      "Radius",
      "Segments"
    ],
    "footerText": "Radius, Segments"
  },
  {
    "slug": "shapes/sweep",
    "category": "shapes",
    "title": "Sweep",
    "description": "Profile along path sweep for framing and custom sections.",
    "statusLabel": "ready",
    "chips": [
      "Path",
      "Caps"
    ],
    "footerText": "Path, Caps"
  },
  {
    "slug": "shapes/wedge",
    "category": "shapes",
    "title": "Wedge",
    "description": "Tapered solid for ramps and sloped technical elements.",
    "statusLabel": "ready",
    "chips": [
      "Width",
      "Height",
      "Depth"
    ],
    "footerText": "Width, Height, Depth"
  },
  {
    "slug": "operations/offset",
    "category": "operations",
    "title": "Offset",
    "description": "Offset generation with acute-corner and bevel parameters.",
    "statusLabel": "ready",
    "chips": [
      "Offset",
      "Bevel"
    ],
    "footerText": "Offset, Bevel"
  },
  {
    "slug": "operations/sweep-path-profile",
    "category": "operations",
    "title": "Sweep Path + Profile",
    "description": "Operation-level sweep from path primitive + profile primitive.",
    "statusLabel": "ready",
    "chips": [
      "Path",
      "Caps"
    ],
    "footerText": "Path, Caps"
  },
  {
    "slug": "operations/sweep-hilbert-profiles",
    "category": "operations",
    "title": "Sweep Hilbert Profiles",
    "description": "Locked Hilbert3D path with switchable kernel and custom section profiles.",
    "statusLabel": "ready",
    "chips": [
      "Hilbert Path",
      "Profiles",
      "Sweep"
    ],
    "footerText": "Profile Type, Caps, Outlines"
  },
  {
    "slug": "operations/wall-from-offsets",
    "category": "operations",
    "title": "Wall from Offsets",
    "description": "Composite wall profile assembled from offset centerlines.",
    "statusLabel": "ready",
    "chips": [
      "Thickness"
    ],
    "footerText": "Thickness"
  }
];

export function getExampleBySlug(slug: string): ExampleMetadata {
  const match = examples.find((example) => example.slug === slug);
  if (!match) {
    throw new Error("Unknown example slug: " + slug);
  }

  return match;
}

export function getExamplesByCategory(category: ExampleCategory): ExampleMetadata[] {
  return examples.filter((example) => example.category === category);
}
