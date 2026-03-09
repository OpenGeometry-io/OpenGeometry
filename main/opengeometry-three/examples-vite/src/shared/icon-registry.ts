import type { IconDefinition } from "@fortawesome/fontawesome-svg-core";
import { icon } from "@fortawesome/fontawesome-svg-core";
import {
  faBezierCurve,
  faCompassDrafting,
  faCube,
  faDatabase,
  faDoorOpen,
  faDrawPolygon,
  faGlobe,
  faLayerGroup,
  faMountain,
  faRoad,
  faRoute,
  faShapes,
  faSlash,
  faUpRightAndDownLeftFromCenter,
  faVectorSquare,
  faWarehouse,
} from "@fortawesome/free-solid-svg-icons";

const iconBySlug: Record<string, IconDefinition> = {
  line: faSlash,
  polyline: faDrawPolygon,
  arc: faCompassDrafting,
  rectangle: faVectorSquare,
  curve: faBezierCurve,
  polygon: faDrawPolygon,
  "polygon-suite": faLayerGroup,
  cuboid: faCube,
  cylinder: faDatabase,
  wedge: faMountain,
  opening: faDoorOpen,
  sweep: faRoad,
  sphere: faGlobe,
  offset: faUpRightAndDownLeftFromCenter,
  "wall-from-offsets": faWarehouse,
  "sweep-path-profile": faRoute,
  "sweep-hilbert-profiles": faRoute,
};

export function getExampleIconMarkup(slug: string): string {
  const key = slug.split("/").pop() ?? slug;
  const selected = iconBySlug[key] ?? faShapes;
  return icon(selected).html.join("");
}
