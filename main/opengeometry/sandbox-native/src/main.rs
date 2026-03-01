use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Instant;

use opengeometry::brep::Brep;
use opengeometry::primitives::arc::OGArc;
use opengeometry::primitives::cuboid::OGCuboid;
use opengeometry::primitives::curve::OGCurve;
use opengeometry::primitives::cylinder::OGCylinder;
use opengeometry::primitives::line::OGLine;
use opengeometry::primitives::polygon::OGPolygon;
use opengeometry::primitives::polyline::OGPolyline;
use opengeometry::primitives::rectangle::OGRectangle;
use opengeometry::primitives::sphere::OGSphere;
use opengeometry::primitives::sweep::OGSweep;
use opengeometry::primitives::wedge::OGWedge;
use openmaths::Vector3;
use serde::Serialize;

#[derive(Serialize)]
struct PrimitiveStats {
    vertices: usize,
    edges: usize,
    faces: usize,
    elapsed_ms: f64,
    brep_preview: String,
}

#[derive(Serialize)]
struct PrimitiveResponse {
    kind: String,
    triangles: Vec<f64>,
    lines: Vec<f64>,
    stats: PrimitiveStats,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

struct ServerPaths {
    three_module: PathBuf,
    orbit_controls: PathBuf,
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:7878")?;
    let paths = ServerPaths {
        three_module: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../node_modules/three/build/three.module.js"),
        orbit_controls: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../node_modules/three/examples/jsm/controls/OrbitControls.js"),
    };

    println!("OpenGeometry native sandbox running on http://127.0.0.1:7878");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle_connection(stream, &paths) {
                    eprintln!("request error: {}", err);
                }
            }
            Err(err) => eprintln!("connection error: {}", err),
        }
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream, paths: &ServerPaths) -> std::io::Result<()> {
    let mut buffer = [0_u8; 16384];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();

    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");

    if method != "GET" {
        return write_response(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"Method Not Allowed",
        );
    }

    let (path, query) = split_target(target);

    match path {
        "/" | "/index.html" => write_response(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            INDEX_HTML.as_bytes(),
        ),
        "/app.js" => write_response(
            &mut stream,
            200,
            "application/javascript; charset=utf-8",
            APP_JS.as_bytes(),
        ),
        "/vendor/three.module.js" => serve_file(
            &mut stream,
            &paths.three_module,
            "application/javascript; charset=utf-8",
        ),
        "/vendor/OrbitControls.js" => serve_file(
            &mut stream,
            &paths.orbit_controls,
            "application/javascript; charset=utf-8",
        ),
        "/api/primitive" => {
            let params = parse_query(query);
            let payload = match build_primitive(&params) {
                Ok(response) => serde_json::to_vec(&response).unwrap_or_else(|_| {
                    serde_json::to_vec(&ErrorResponse {
                        error: "Failed to serialize response".to_string(),
                    })
                    .unwrap()
                }),
                Err(err) => serde_json::to_vec(&ErrorResponse { error: err }).unwrap(),
            };

            write_response(&mut stream, 200, "application/json", &payload)
        }
        _ => write_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"Not Found",
        ),
    }
}

fn split_target(target: &str) -> (&str, &str) {
    if let Some((path, query)) = target.split_once('?') {
        (path, query)
    } else {
        (target, "")
    }
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut output = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }

        let (key, value) = if let Some((k, v)) = pair.split_once('=') {
            (k, v)
        } else {
            (pair, "")
        };

        output.insert(key.to_string(), value.to_string());
    }
    output
}

fn get_f64(params: &HashMap<String, String>, key: &str, fallback: f64) -> f64 {
    params
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(fallback)
}

fn get_u32(params: &HashMap<String, String>, key: &str, fallback: u32) -> u32 {
    params
        .get(key)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(fallback)
}

fn parse_f64_buffer(payload: &str) -> Result<Vec<f64>, String> {
    serde_json::from_str(payload).map_err(|err| format!("Failed to parse geometry buffer: {}", err))
}

fn line_buffer_from_brep(brep: &Brep) -> Vec<f64> {
    let mut line_buffer = Vec::with_capacity(brep.edges.len() * 6);

    for edge in &brep.edges {
        let start = brep.vertices.get(edge.v1 as usize);
        let end = brep.vertices.get(edge.v2 as usize);

        if let (Some(start), Some(end)) = (start, end) {
            line_buffer.push(start.position.x);
            line_buffer.push(start.position.y);
            line_buffer.push(start.position.z);

            line_buffer.push(end.position.x);
            line_buffer.push(end.position.y);
            line_buffer.push(end.position.z);
        }
    }

    line_buffer
}

fn truncate_preview(value: String, max_len: usize) -> String {
    if value.len() <= max_len {
        return value;
    }

    let mut clipped = value.chars().take(max_len).collect::<String>();
    clipped.push_str("...");
    clipped
}

fn finalize_response(
    kind: &str,
    started: Instant,
    brep: &Brep,
    brep_serialized: String,
    geometry_serialized: String,
) -> Result<PrimitiveResponse, String> {
    let triangles = if brep.faces.is_empty() {
        Vec::new()
    } else {
        parse_f64_buffer(&geometry_serialized)?
    };

    let lines = line_buffer_from_brep(brep);

    Ok(PrimitiveResponse {
        kind: kind.to_string(),
        triangles,
        lines,
        stats: PrimitiveStats {
            vertices: brep.vertices.len(),
            edges: brep.edges.len(),
            faces: brep.faces.len(),
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
            brep_preview: truncate_preview(brep_serialized, 800),
        },
    })
}

fn build_primitive(params: &HashMap<String, String>) -> Result<PrimitiveResponse, String> {
    let kind = params.get("kind").map(String::as_str).unwrap_or("cuboid");

    let size = get_f64(params, "size", 1.0).max(0.05);
    let radius = get_f64(params, "radius", 0.75).max(0.05);
    let height = get_f64(params, "height", 1.5).max(0.05);
    let width = get_f64(params, "width", 1.2).max(0.05);
    let depth = get_f64(params, "depth", 1.0).max(0.05);
    let angle = get_f64(params, "angle", std::f64::consts::PI * 2.0).max(0.05);
    let path_scale = get_f64(params, "pathScale", 1.0).max(0.05);
    let segments = get_u32(params, "segments", 24).max(3);
    let width_segments = get_u32(params, "widthSegments", 24).max(3);
    let height_segments = get_u32(params, "heightSegments", 16).max(2);

    match kind {
        "line" => {
            let started = Instant::now();
            let mut line = OGLine::new("sandbox-line".to_string());
            line.set_config(
                Vector3::new(-size * 1.8, 0.0, -size * 0.5),
                Vector3::new(size * 1.8, 0.0, size * 0.9),
            );
            line.generate_geometry();

            finalize_response(
                "line",
                started,
                line.brep(),
                line.get_brep_serialized(),
                line.get_geometry_serialized(),
            )
        }
        "polyline" => {
            let started = Instant::now();
            let mut polyline = OGPolyline::new("sandbox-polyline".to_string());
            polyline.set_config(vec![
                Vector3::new(-2.0 * size, 0.0, -1.2 * size),
                Vector3::new(-1.0 * size, 0.0, -0.2 * size),
                Vector3::new(0.0 * size, 0.0, -0.8 * size),
                Vector3::new(1.2 * size, 0.0, 0.9 * size),
                Vector3::new(2.0 * size, 0.0, 1.1 * size),
            ]);

            finalize_response(
                "polyline",
                started,
                polyline.brep(),
                polyline.get_brep_serialized(),
                polyline.get_geometry_serialized(),
            )
        }
        "arc" => {
            let started = Instant::now();
            let mut arc = OGArc::new("sandbox-arc".to_string());
            arc.set_config(
                Vector3::new(0.0, 0.0, 0.0),
                radius,
                0.0,
                angle,
                segments,
            );
            arc.generate_geometry();

            let geometry_serialized = arc.get_geometry_serialized();
            let brep_serialized = arc.get_brep_serialized();
            finalize_response(
                "arc",
                started,
                arc.brep(),
                brep_serialized,
                geometry_serialized,
            )
        }
        "rectangle" => {
            let started = Instant::now();
            let mut rectangle = OGRectangle::new("sandbox-rectangle".to_string());
            rectangle.set_config(Vector3::new(0.0, 0.0, 0.0), width * 2.0, depth * 2.0);
            rectangle.generate_geometry();

            finalize_response(
                "rectangle",
                started,
                rectangle.brep(),
                rectangle.get_brep_serialized(),
                rectangle.get_geometry_serialized(),
            )
        }
        "curve" => {
            let started = Instant::now();
            let mut curve = OGCurve::new("sandbox-curve".to_string());
            curve.set_config(vec![
                Vector3::new(-2.2 * size, 0.0, -0.8 * size),
                Vector3::new(-1.0 * size, 0.0, -1.6 * size),
                Vector3::new(0.5 * size, 0.0, -0.6 * size),
                Vector3::new(2.0 * size, 0.0, -1.0 * size),
            ]);

            finalize_response(
                "curve",
                started,
                curve.brep(),
                curve.get_brep_serialized(),
                curve.get_geometry_serialized(),
            )
        }
        "polygon" => {
            let started = Instant::now();
            let mut polygon = OGPolygon::new("sandbox-polygon".to_string());
            polygon.set_config(vec![
                Vector3::new(-1.8 * size, 0.0, -0.7 * size),
                Vector3::new(-0.4 * size, 0.0, -1.3 * size),
                Vector3::new(1.1 * size, 0.0, -0.2 * size),
                Vector3::new(0.5 * size, 0.0, 1.5 * size),
                Vector3::new(-1.6 * size, 0.0, 1.0 * size),
            ]);

            let geometry_serialized = polygon.get_geometry_serialized();
            let brep_serialized = polygon.get_brep_serialized();
            finalize_response(
                "polygon",
                started,
                polygon.brep(),
                brep_serialized,
                geometry_serialized,
            )
        }
        "cuboid" => {
            let started = Instant::now();
            let mut cuboid = OGCuboid::new("sandbox-cuboid".to_string());
            cuboid.set_config(Vector3::new(0.0, height * 0.5, 0.0), width, height, depth);

            finalize_response(
                "cuboid",
                started,
                cuboid.brep(),
                cuboid.get_brep_serialized(),
                cuboid.get_geometry_serialized(),
            )
        }
        "cylinder" => {
            let started = Instant::now();
            let mut cylinder = OGCylinder::new("sandbox-cylinder".to_string());
            cylinder.set_config(
                Vector3::new(0.0, height * 0.5, 0.0),
                radius,
                height,
                angle,
                segments,
            );

            let geometry_serialized = cylinder.get_geometry_serialized();
            let brep_serialized = cylinder.get_brep_serialized();
            finalize_response(
                "cylinder",
                started,
                cylinder.brep(),
                brep_serialized,
                geometry_serialized,
            )
        }
        "wedge" => {
            let started = Instant::now();
            let mut wedge = OGWedge::new("sandbox-wedge".to_string());
            wedge.set_config(Vector3::new(0.0, height * 0.5, 0.0), width * 1.6, height, depth);

            finalize_response(
                "wedge",
                started,
                wedge.brep(),
                wedge.get_brep_serialized(),
                wedge.get_geometry_serialized(),
            )
        }
        "sweep" => {
            let started = Instant::now();
            let mut sweep = OGSweep::new("sandbox-sweep".to_string());
            sweep.set_config_with_caps(
                vec![
                    Vector3::new(-2.0 * path_scale, 0.0, -1.2 * path_scale),
                    Vector3::new(-1.1 * path_scale, 0.7 * path_scale, -0.2 * path_scale),
                    Vector3::new(0.0 * path_scale, 1.4 * path_scale, 0.7 * path_scale),
                    Vector3::new(1.1 * path_scale, 2.0 * path_scale, 0.1 * path_scale),
                    Vector3::new(2.0 * path_scale, 2.4 * path_scale, -0.8 * path_scale),
                ],
                vec![
                    Vector3::new(-0.25 * size, 0.0, -0.2 * size),
                    Vector3::new(0.25 * size, 0.0, -0.2 * size),
                    Vector3::new(0.25 * size, 0.0, 0.2 * size),
                    Vector3::new(-0.25 * size, 0.0, 0.2 * size),
                ],
                true,
                true,
            );

            finalize_response(
                "sweep",
                started,
                sweep.brep(),
                sweep.get_brep_serialized(),
                sweep.get_geometry_serialized(),
            )
        }
        "sphere" => {
            let started = Instant::now();
            let mut sphere = OGSphere::new("sandbox-sphere".to_string());
            sphere.set_config(
                Vector3::new(0.0, radius, 0.0),
                radius,
                width_segments,
                height_segments,
            );

            finalize_response(
                "sphere",
                started,
                sphere.brep(),
                sphere.get_brep_serialized(),
                sphere.get_geometry_serialized(),
            )
        }
        "opening" => {
            let started = Instant::now();
            let mut opening = OGCuboid::new("sandbox-opening".to_string());
            opening.set_config(
                Vector3::new(0.0, height * 0.5, 0.0),
                width,
                height,
                depth * 0.25,
            );

            finalize_response(
                "opening",
                started,
                opening.brep(),
                opening.get_brep_serialized(),
                opening.get_geometry_serialized(),
            )
        }
        _ => Err(format!("Unsupported kind '{}'.", kind)),
    }
}

fn serve_file(
    stream: &mut TcpStream,
    path: &PathBuf,
    content_type: &str,
) -> std::io::Result<()> {
    if !path.exists() {
        return write_response(
            stream,
            404,
            "text/plain; charset=utf-8",
            format!("File not found: {}", path.display()).as_bytes(),
        );
    }

    let body = fs::read(path)?;
    write_response(stream, 200, content_type, &body)
}

fn write_response(
    stream: &mut TcpStream,
    status_code: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let status_text = match status_code {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Internal Server Error",
    };

    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status_code,
        status_text,
        content_type,
        body.len()
    );

    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>OpenGeometry Native Sandbox</title>
    <style>
      :root {
        --bg: #f8fafc;
        --panel: #ffffff;
        --text: #0f172a;
        --muted: #475569;
        --border: #cbd5e1;
      }
      * { box-sizing: border-box; }
      html, body { margin: 0; width: 100%; height: 100%; background: var(--bg); color: var(--text); font: 13px/1.45 "Helvetica Neue", "Segoe UI", sans-serif; }
      #root { display: grid; grid-template-columns: 340px 1fr; width: 100%; height: 100%; }
      #panel {
        border-right: 1px solid var(--border);
        background: var(--panel);
        padding: 12px;
        overflow: auto;
      }
      #viewport { position: relative; }
      #app { width: 100%; height: 100%; }
      .row { margin-bottom: 10px; }
      label { display: block; margin-bottom: 4px; font-weight: 600; }
      input, select {
        width: 100%;
        border: 1px solid var(--border);
        border-radius: 7px;
        padding: 6px 8px;
        background: #fff;
      }
      .meta {
        margin-top: 12px;
        border: 1px solid var(--border);
        border-radius: 8px;
        background: #f8fafc;
        padding: 8px;
      }
      .meta pre {
        margin: 0;
        white-space: pre-wrap;
        word-break: break-word;
      }
      .title { margin: 0 0 8px; font-size: 16px; }
      .subtitle { margin: 0 0 12px; color: var(--muted); }
      .info-badge {
        position: absolute;
        top: 10px;
        left: 10px;
        background: rgba(255,255,255,0.92);
        border: 1px solid var(--border);
        border-radius: 8px;
        padding: 8px 10px;
      }
    </style>
  </head>
  <body>
    <div id="root">
      <aside id="panel">
        <h1 class="title">OpenGeometry Sandbox</h1>
        <p class="subtitle">Native Rust backend + visual GUI debugger</p>

        <div class="row">
          <label for="kind">Primitive / Shape</label>
          <select id="kind">
            <option value="line">line</option>
            <option value="polyline">polyline</option>
            <option value="arc">arc</option>
            <option value="rectangle">rectangle</option>
            <option value="curve">curve</option>
            <option value="polygon">polygon</option>
            <option value="cuboid" selected>cuboid</option>
            <option value="cylinder">cylinder</option>
            <option value="wedge">wedge</option>
            <option value="sweep">sweep</option>
            <option value="sphere">sphere</option>
            <option value="opening">opening</option>
          </select>
        </div>

        <div class="row">
          <label for="preset">Preset</label>
          <select id="preset">
            <option value="default" selected>default</option>
            <option value="architectural">architectural</option>
            <option value="dense">dense segments</option>
            <option value="sweepy">sweep-heavy</option>
          </select>
        </div>

        <div class="row"><label for="size">size</label><input id="size" type="text" inputmode="decimal" value="1.0" /></div>
        <div class="row"><label for="width">width</label><input id="width" type="text" inputmode="decimal" value="1.2" /></div>
        <div class="row"><label for="height">height</label><input id="height" type="text" inputmode="decimal" value="1.5" /></div>
        <div class="row"><label for="depth">depth</label><input id="depth" type="text" inputmode="decimal" value="1.0" /></div>
        <div class="row"><label for="radius">radius</label><input id="radius" type="text" inputmode="decimal" value="0.75" /></div>
        <div class="row"><label for="segments">segments</label><input id="segments" type="text" inputmode="numeric" value="24" /></div>
        <div class="row"><label for="widthSegments">widthSegments</label><input id="widthSegments" type="text" inputmode="numeric" value="24" /></div>
        <div class="row"><label for="heightSegments">heightSegments</label><input id="heightSegments" type="text" inputmode="numeric" value="16" /></div>
        <div class="row"><label for="angle">angle (radians)</label><input id="angle" type="text" inputmode="decimal" value="6.28318530718" /></div>
        <div class="row"><label for="pathScale">pathScale</label><input id="pathScale" type="text" inputmode="decimal" value="1.0" /></div>

        <div class="meta">
          <strong>Debug</strong>
          <pre id="debug">waiting for first render...</pre>
        </div>
      </aside>

      <main id="viewport">
        <div id="app"></div>
        <div class="info-badge">Orbit: left-drag | Pan: right-drag | Zoom: wheel</div>
      </main>
    </div>

    <script type="importmap">
      {
        "imports": {
          "three": "/vendor/three.module.js"
        }
      }
    </script>
    <script type="module" src="/app.js"></script>
  </body>
</html>
"#;

const APP_JS: &str = r#"import * as THREE from '/vendor/three.module.js';
import { OrbitControls } from '/vendor/OrbitControls.js';

const app = document.getElementById('app');
const debugEl = document.getElementById('debug');

const scene = new THREE.Scene();
scene.background = new THREE.Color(0xf8fafc);

const camera = new THREE.PerspectiveCamera(55, window.innerWidth / window.innerHeight, 0.1, 5000);
camera.position.set(5.5, 4.5, 6.5);

const renderer = new THREE.WebGLRenderer({ antialias: true });
renderer.setPixelRatio(window.devicePixelRatio);
renderer.setSize(window.innerWidth, window.innerHeight);
app.appendChild(renderer.domElement);

const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
controls.target.set(0, 0.8, 0);
controls.update();

scene.add(new THREE.GridHelper(32, 32, 0x94a3b8, 0xd1d5db));
scene.add(new THREE.AmbientLight(0xffffff, 0.65));

const key = new THREE.DirectionalLight(0xffffff, 0.9);
key.position.set(6, 9, 5);
scene.add(key);

const fill = new THREE.DirectionalLight(0xffffff, 0.35);
fill.position.set(-6, 4, -4);
scene.add(fill);

const primitiveGroup = new THREE.Group();
scene.add(primitiveGroup);

function getInputValue(id) {
  const el = document.getElementById(id);
  if (!el) {
    return '';
  }

  // Keep parsing robust even if browsers preserve escaped/quoted attribute values.
  return String(el.value)
    .replaceAll('"', '')
    .replaceAll('\\', '')
    .replaceAll(',', '.');
}

function collectParams() {
  return {
    kind: getInputValue('kind'),
    size: getInputValue('size'),
    width: getInputValue('width'),
    height: getInputValue('height'),
    depth: getInputValue('depth'),
    radius: getInputValue('radius'),
    segments: getInputValue('segments'),
    widthSegments: getInputValue('widthSegments'),
    heightSegments: getInputValue('heightSegments'),
    angle: getInputValue('angle'),
    pathScale: getInputValue('pathScale'),
  };
}

function disposeNode(node) {
  if (node.geometry) {
    node.geometry.dispose();
  }
  if (node.material) {
    if (Array.isArray(node.material)) {
      node.material.forEach((mat) => mat.dispose());
    } else {
      node.material.dispose();
    }
  }
}

function clearPrimitiveGroup() {
  for (const child of [...primitiveGroup.children]) {
    primitiveGroup.remove(child);
    disposeNode(child);
  }
}

function renderBuffers(payload) {
  clearPrimitiveGroup();

  if (Array.isArray(payload.triangles) && payload.triangles.length >= 9) {
    const meshGeometry = new THREE.BufferGeometry();
    meshGeometry.setAttribute('position', new THREE.Float32BufferAttribute(payload.triangles, 3));
    meshGeometry.computeVertexNormals();

    const mesh = new THREE.Mesh(
      meshGeometry,
      new THREE.MeshStandardMaterial({ color: 0x60a5fa, transparent: true, opacity: 0.65 })
    );
    primitiveGroup.add(mesh);
  }

  if (Array.isArray(payload.lines) && payload.lines.length >= 6) {
    const lineGeometry = new THREE.BufferGeometry();
    lineGeometry.setAttribute('position', new THREE.Float32BufferAttribute(payload.lines, 3));
    const lineMaterial = new THREE.LineBasicMaterial({ color: 0x111827 });
    const lines = new THREE.LineSegments(lineGeometry, lineMaterial);
    primitiveGroup.add(lines);
  }
}

async function refreshPrimitive() {
  const params = new URLSearchParams(collectParams());
  const response = await fetch(`/api/primitive?${params.toString()}`);
  const payload = await response.json();

  if (payload.error) {
    debugEl.textContent = payload.error;
    clearPrimitiveGroup();
    return;
  }

  renderBuffers(payload);
  debugEl.textContent = JSON.stringify(payload.stats, null, 2);
}

function applyPreset(name) {
  const updates = {
    default: {
      size: '1.0', width: '1.2', height: '1.5', depth: '1.0', radius: '0.75',
      segments: '24', widthSegments: '24', heightSegments: '16',
      angle: String(Math.PI * 2), pathScale: '1.0'
    },
    architectural: {
      size: '1.3', width: '2.4', height: '2.8', depth: '0.35', radius: '0.55',
      segments: '28', widthSegments: '24', heightSegments: '18',
      angle: String(Math.PI * 2), pathScale: '1.1'
    },
    dense: {
      size: '1.0', width: '1.4', height: '1.6', depth: '1.2', radius: '1.0',
      segments: '56', widthSegments: '56', heightSegments: '30',
      angle: String(Math.PI * 2), pathScale: '1.0'
    },
    sweepy: {
      size: '0.8', width: '0.9', height: '1.2', depth: '0.9', radius: '0.65',
      segments: '24', widthSegments: '28', heightSegments: '18',
      angle: String(Math.PI * 1.6), pathScale: '1.8'
    }
  };

  const preset = updates[name] || updates.default;
  for (const [key, value] of Object.entries(preset)) {
    const el = document.getElementById(key);
    if (el) {
      el.value = value;
    }
  }
}

const numericIds = [
  'size', 'width', 'height', 'depth', 'radius', 'segments',
  'widthSegments', 'heightSegments', 'angle', 'pathScale'
];

for (const id of numericIds) {
  const el = document.getElementById(id);
  if (!el) {
    continue;
  }

  // Prefer dot decimals regardless of browser locale.
  el.setAttribute('lang', 'en-US');
  el.setAttribute('inputmode', 'decimal');

  el.addEventListener('input', () => {
    if (typeof el.value === 'string' && el.value.includes(',')) {
      el.value = el.value.replaceAll(',', '.');
    }
    refreshPrimitive().catch((err) => {
      debugEl.textContent = String(err);
    });
  });
}

const kindEl = document.getElementById('kind');
if (kindEl) {
  kindEl.addEventListener('change', () => {
    refreshPrimitive().catch((err) => {
      debugEl.textContent = String(err);
    });
  });
}

const presetEl = document.getElementById('preset');
if (presetEl) {
  presetEl.addEventListener('change', () => {
    applyPreset(presetEl.value);
    refreshPrimitive().catch((err) => {
      debugEl.textContent = String(err);
    });
  });
}

window.addEventListener('resize', () => {
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(window.innerWidth, window.innerHeight);
});

function animate() {
  requestAnimationFrame(animate);
  controls.update();
  renderer.render(scene, camera);
}
animate();

applyPreset('default');
refreshPrimitive().catch((err) => {
  debugEl.textContent = String(err);
});
"#;
