# projectTo2D Data Format

This document describes how to inspect and consume scene projection data from `OGSceneManager`.

## API methods

- `projectTo2DCamera(sceneId, cameraJson, hlrJson?)` -> `Scene2D` JSON string
- `projectTo2DCameraPretty(sceneId, cameraJson, hlrJson?)` -> pretty `Scene2D` JSON string
- `projectTo2DLines(sceneId, cameraJson, hlrJson?)` -> normalized `Scene2DLines` JSON string
- `projectTo2DLinesPretty(sceneId, cameraJson, hlrJson?)` -> pretty normalized JSON string
- `projectCurrentTo2DCamera(...)` / `projectCurrentTo2DLines(...)` use current scene

## `Scene2D` shape

```json
{
  "name": "Main Scene",
  "paths": [
    {
      "segments": [
        {
          "Line": {
            "start": { "x": -0.2, "y": 0.1 },
            "end": { "x": 0.3, "y": 0.4 }
          }
        }
      ],
      "stroke_width": null,
      "stroke_color": null
    }
  ]
}
```

## `Scene2DLines` normalized shape (recommended for frontend rendering)

```json
{
  "name": "Main Scene",
  "lines": [
    {
      "start": { "x": -0.2, "y": 0.1 },
      "end": { "x": 0.3, "y": 0.4 },
      "stroke_width": null,
      "stroke_color": null
    }
  ]
}
```

## Frontend usage (Three.js lines)

```ts
const scene2dLinesJson = manager.projectTo2DLines(
  sceneId,
  JSON.stringify(camera),
  JSON.stringify({ hide_hidden_edges: true })
);
const scene2dLines = JSON.parse(scene2dLinesJson);

for (const line of scene2dLines.lines) {
  // line.start.x, line.start.y, line.end.x, line.end.y
  // map to your viewport scale and build Three.js LineSegments geometry
}
```

## Local inspection

```bash
cd main/opengeometry
cargo run --example scenegraph_projection_dump_json -- ./out/projection_dump
jq . ./out/projection_dump_scene2d.json
jq . ./out/projection_dump_lines2d.json
```
