import * as THREE from "three";
import { CSS2DObject } from "three/examples/jsm/renderers/CSS2DRenderer.js";
import { Event } from "./utils/event";

/**
 * Whenever you want something to work with pencil you should add it to pencil object
 */

export type PencilMode = "draw" | "erase" | "select" | "cursor";

export class Pencil {
  private container: HTMLElement;
  private scene: THREE.Scene;
  private raycaster: THREE.Raycaster = new THREE.Raycaster();
  pencilMeshes: THREE.Mesh[] = [];

  cursor: CSS2DObject | undefined;
  onCursorDown: Event<THREE.Vector3> = new Event();
  onCursorMove: Event<THREE.Vector3> = new Event();
  onElementSelected: Event<THREE.Mesh> = new Event();

  pencilMode: PencilMode = "cursor";

  // dummy plane can be ignored when we have at least one object in the scene
  private dummyPlane: THREE.Mesh | undefined;
  
  constructor(container: HTMLElement, scene: THREE.Scene, private camera: THREE.Camera) {
    this.container = container;
    this.scene = scene;
    this.setup();
  }

  set mode(mode: PencilMode) {
    this.pencilMode = mode;
  }

  get drawingCanvas() {
    return this.dummyPlane;
  }

  setup() {
    this.setupCursor();
    this.setupCursorEvent();

    // A Dummy Ground Plane
    const geometry = new THREE.PlaneGeometry(100, 100);
    const material = new THREE.MeshBasicMaterial({ color: 0xffff00, side: THREE.DoubleSide, transparent: true, opacity: 0 });
    const plane = new THREE.Mesh(geometry, material);
    plane.rotation.x = Math.PI / 2;
    this.scene.add(plane);
    plane.visible = false;
    this.dummyPlane = plane;
  }

  groundVisible(visible: boolean) {
    if (this.dummyPlane) {
      this.dummyPlane.visible = visible;
    }
  }

  setupCursor() {
    const cursorElement = document.createElement("div");
    cursorElement.style.position = "absolute";
    cursorElement.style.width = "1px";
    cursorElement.style.height = "60px";
    cursorElement.style.backgroundColor = "red";
    cursorElement.style.pointerEvents = "none";

    const horizontalLine = document.createElement("div");
    horizontalLine.style.position = "absolute";
    horizontalLine.style.width = "60px";
    horizontalLine.style.height = "1px";
    horizontalLine.style.left = "-30px";
    horizontalLine.style.top = "30px";
    horizontalLine.style.backgroundColor = "red";
    horizontalLine.style.pointerEvents = "none";
    cursorElement.appendChild(horizontalLine);

    this.container.style.cursor = "none";

    const cursorMesh = new CSS2DObject(cursorElement);
    cursorMesh.name = "cursor";
    cursorMesh.position.set(0, 0, 0);
    this.scene.add(cursorMesh);
    this.cursor = cursorMesh;
  }

  setupCursorEvent() {
    this.container.addEventListener("mousemove", (event) => {
      const rect = this.container.getBoundingClientRect();
      const x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
      const y = -((event.clientY - rect.top) / rect.height) * 2 + 1;

      this.raycaster.setFromCamera(new THREE.Vector2(x, y), this.camera);
      const intersects = this.raycaster.intersectObjects([this.dummyPlane!, ...this.pencilMeshes]);

      if (intersects.length > 0) {
        const intersect = intersects[0];
        const point = intersect.point;
        this.cursor?.position.set(point.x, point.y, point.z);
        this.onCursorMove.trigger(point);
      }
    });

    this.container.addEventListener("mousedown", (event) => {
      if (this.pencilMode === "cursor") {
        const rect = this.container.getBoundingClientRect();
        const x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
        const y = -((event.clientY - rect.top) / rect.height) * 2 + 1;

        this.raycaster.setFromCamera(new THREE.Vector2(x, y), this.camera);
        const intersects = this.raycaster.intersectObjects([this.dummyPlane!, ...this.pencilMeshes]);

        if (intersects.length > 0) {
          const intersect = intersects[0];
          const point = intersect.point;
          this.onCursorDown.trigger(point);
          this.onElementSelected.trigger(intersect.object as THREE.Mesh);
        }
      }
    });
  }
}
