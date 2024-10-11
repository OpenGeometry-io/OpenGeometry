import { CSS2DObject } from "three/examples/jsm/renderers/CSS2DRenderer.js";

function spotLabelElement() {
  const spotLabelElement = document.createElement("div");
  spotLabelElement.style.position = "absolute";
  spotLabelElement.style.width = "3px";
  spotLabelElement.style.height = "3px";
  spotLabelElement.style.backgroundColor = "blue";
  spotLabelElement.style.pointerEvents = "none";
  // spotLabelElement.style.borderRadius = "50%";
  spotLabelElement.style.border = "1px solid black";
  return spotLabelElement;
}

export class SpotLabel extends CSS2DObject {
  constructor() {
    const spotLabel = spotLabelElement();
    super(spotLabel);
  }
}
