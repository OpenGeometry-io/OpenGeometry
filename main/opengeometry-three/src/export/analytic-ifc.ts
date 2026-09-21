import { getUUID } from "../utils/randomizer";

export type IfcPreparedValue =
  | { kind: "Ref"; id: number }
  | { kind: "List"; values: IfcPreparedValue[] }
  | { kind: "Real"; value: number }
  | { kind: "Integer"; value: number }
  | { kind: "Text" | "Enum"; value: string }
  | { kind: "Bool"; value: boolean }
  | { kind: "Null" | "Derived" };

export interface AnalyticExchangeBodyV2 {
  schema_version: 2;
  revision: string;
  length_unit: "metre";
  up_axis: "Z";
  coordinate_space: "world";
  representation_type: "AdvancedBrep" | "CSG";
  quality: { kind: "Analytic" } | { kind: "Approximate"; max_error: number };
  geometric_tolerance: number;
  exchange_error_bound: number;
  validation_level: string;
  entities: { id: number; entity_type: string; attributes: IfcPreparedValue[] }[];
  solids: number[];
  faces: {
    face: number; key: string; entity: number;
    provenance: {
      sources: { entity: string; body: string; key: string; face: number }[];
      role: "Authored" | "Preserved" | "Split" | "Cut" | "Coincident"; reversed: boolean;
    };
  }[];
}

function quoted(text: string): string {
  let escaped = "";
  for (const character of text) {
    const code = character.charCodeAt(0);
    if (character === "'") escaped += "''";
    else if (code === 92) escaped += "\\\\";
    else if (code >= 32 && code <= 126) escaped += character;
    else {
      const utf16 = Array.from({ length: character.length }, (_, i) => ("0000" + character.charCodeAt(i).toString(16).toUpperCase()).slice(-4)).join("");
      escaped += "\\X2\\" + utf16 + "\\X0\\";
    }
  }
  return "'" + escaped + "'";
}
function guid(): string {
  const alphabet = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";
  const uuid = getUUID().replace(/-/g, "");
  const first = parseInt(uuid.slice(0, 2), 16);
  let value = alphabet[Math.floor(first / 64)] + alphabet[first % 64];
  for (let i = 2; i < 32; i += 6) {
    let block = parseInt(uuid.slice(i, i + 6), 16);
    let digits = "";
    for (let j = 0; j < 4; j++) { digits = alphabet[block % 64] + digits; block = Math.floor(block / 64); }
    value += digits;
  }
  return value;
}

/** Serializes Rust-prepared geometry; this function does not fit curves or tessellate surfaces. */
export function analyticIfcText(body: AnalyticExchangeBodyV2, name = "Analytic body"): string {
  if (body.schema_version !== 2 || body.length_unit !== "metre" || body.up_axis !== "Z" || body.coordinate_space !== "world"
    || !Array.isArray(body.entities) || !body.entities.length || body.entities.length > 100_000
    || !Array.isArray(body.solids) || !body.solids.length || !Array.isArray(body.faces)
    || !/^(0|[1-9][0-9]{0,19})$/.test(body.revision) || (body.revision.length === 20 && body.revision > "18446744073709551615")
    || !Number.isFinite(body.geometric_tolerance) || body.geometric_tolerance <= 0
    || !Number.isFinite(body.exchange_error_bound) || body.exchange_error_bound < body.geometric_tolerance
    || typeof name !== "string" || name.length > 4096) throw new Error("Invalid analytic IFC exchange contract");
  const entities: string[] = [];
  let budget = 500_000;
  function encode(value: IfcPreparedValue, before: number, depth = 0): string {
    if (--budget < 0 || depth > 16 || !value || typeof value !== "object") throw new Error("IFC aggregate resource limit exceeded");
    switch (value.kind) {
      case "Ref":
        if (!Number.isInteger(value.id) || value.id <= 0 || value.id >= before) throw new Error("Invalid IFC entity reference");
        return "#" + value.id;
      case "List":
        if (!Array.isArray(value.values)) throw new Error("IFC aggregate must be an array");
        return "(" + value.values.map(v => encode(v, before, depth + 1)).join(",") + ")";
      case "Real":
        if (!Number.isFinite(value.value)) throw new Error("Non-finite IFC real value");
        return value.value.toExponential(17);
      case "Integer":
        if (!Number.isSafeInteger(value.value)) throw new Error("Invalid IFC integer value");
        return String(value.value);
      case "Text":
        if (typeof value.value !== "string" || value.value.length > 4096) throw new Error("Invalid IFC string");
        return quoted(value.value);
      case "Enum":
        if (!/^[A-Z][A-Z0-9_]*$/.test(value.value)) throw new Error("Invalid IFC enumeration");
        return "." + value.value + ".";
      case "Bool":
        if (typeof value.value !== "boolean") throw new Error("IFC logical value must be boolean");
        return value.value ? ".T." : ".F.";
      case "Null": return "$";
      case "Derived": return "*";
      default: throw new Error("Unsupported IFC prepared value");
    }
  }
  for (const entity of body.entities) {
    if (entity.id !== entities.length + 1 || !/^Ifc[A-Za-z0-9]+$/.test(entity.entity_type)
      || !Array.isArray(entity.attributes) || entity.attributes.length > 16) throw new Error("Invalid IFC prepared entity");
    entities.push("#" + entity.id + "=" + entity.entity_type.toUpperCase() + "(" + entity.attributes.map(value => encode(value, entity.id)).join(",") + ");");
  }
  const add = (expression: string) => { const id = entities.length + 1; entities.push("#" + id + "=" + expression + ";"); return id; };
  const origin = add("IFCCARTESIANPOINT((0.,0.,0.))");
  const z = add("IFCDIRECTION((0.,0.,1.))");
  const x = add("IFCDIRECTION((1.,0.,0.))");
  const placement = add("IFCAXIS2PLACEMENT3D(#" + origin + ",#" + z + ",#" + x + ")");
  const context = add("IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3," + body.exchange_error_bound.toExponential(17) + ",#" + placement + ",$)");
  const length = add("IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.)");
  const units = add("IFCUNITASSIGNMENT((#" + length + "))");
  add("IFCPROJECT(" + quoted(guid()) + ",$," + quoted(name) + ",$,$,$,$,(#" + context + "),#" + units + ")");
  const solidRefs = body.solids.map(id => {
    const expected = body.representation_type === "AdvancedBrep" ? "IfcAdvancedBrep" : "IfcBooleanResult";
    if (!Number.isInteger(id) || body.entities[id - 1]?.entity_type !== expected) throw new Error("Invalid IFC solid reference");
    return "#" + id;
  }).join(",");
  if (["AdvancedBrep", "CSG"].indexOf(body.representation_type) < 0) throw new Error("Invalid IFC representation type");
  const representation = add("IFCSHAPEREPRESENTATION(#" + context + ",'Body'," + quoted(body.representation_type) + ",(" + solidRefs + "))");
  const productShape = add("IFCPRODUCTDEFINITIONSHAPE($,$,(#" + representation + "))");
  const objectPlacement = add("IFCLOCALPLACEMENT($,#" + placement + ")");
  add("IFCBUILDINGELEMENTPROXY(" + quoted(guid()) + ",$," + quoted(name) + ",$,$,#" + objectPlacement + ",#" + productShape + ",$,.NOTDEFINED.)");
  return [
    "ISO-10303-21;", "HEADER;", "FILE_DESCRIPTION(('OpenGeometry analytic BRep v2'),'2;1');",
    "FILE_NAME(" + quoted(name + ".ifc") + ",'1970-01-01T00:00:00',(''),(''),'OpenGeometry','OpenGeometry','');",
    "FILE_SCHEMA(('IFC4'));", "ENDSEC;", "DATA;", ...entities, "ENDSEC;", "END-ISO-10303-21;", "",
  ].join("\n");
}
