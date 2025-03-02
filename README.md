# OpenGeometry

#### Initialize Library

- `const openGeometry = new OpenGeometry()`
- `await openGeometry.setup()`

#### Create Polygon
- `const oPoly = new OPolygon()`

##### Add Points to Polygon
- addVertex(x:number, y:number, z:number) - Add Point to the Given Polygon

##### Generate Flat 2D Mesh
- generateMesh() - Created a Flat 2D Mesh from the given Vertices
- Minimum 3 Vertices are needed, else ask users for it

### Extrude
- Extrude is a operation which elevates the given 2D polygon by a given height
- `polygon.extrude(height: number)`
- returns a Buffer Geometry
