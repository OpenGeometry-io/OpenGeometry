from pathlib import Path

import ifcopenshell


def main() -> None:
    ifc_path = Path("main/opengeometry/target/export-validation/validation-cuboid.ifc")
    if not ifc_path.exists():
        raise SystemExit(f"IFC fixture not found: {ifc_path}")

    model = ifcopenshell.open(str(ifc_path))
    if model is None:
        raise SystemExit("IfcOpenShell failed to open IFC fixture")

    projects = model.by_type("IfcProject")
    products = model.by_type("IfcProduct")
    if not projects:
        raise SystemExit("IFC fixture missing IfcProject")
    if not products:
        raise SystemExit("IFC fixture missing IfcProduct")

    print(f"Validated IFC fixture: projects={len(projects)} products={len(products)}")


if __name__ == "__main__":
    main()
