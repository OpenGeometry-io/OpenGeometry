## 🚧 Under Development 🚧  

**This project is currently a work in progress!**  
Expect frequent updates, breaking changes, and exciting new features. 

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

- Scene Graph (List of Objects)
- Access to Scene Graph for other classes
- Bounding Boxes for BaseGeometry (min, max)
- Check Intersection with existing BBoxes - multiple cases - Partial Outside | Inside | Touching | Outside

### Test
#### Basic Example
- A basic example is available at `./test/index.html`

#### Advanced and Additional Examples
- Extensive Examples(Source Code) have a separate repo - https://github.com/OpenGeometry-io/OpenGeometry-examples
- The live demo is available at [Kernel Examples](https://demos.opengeometry.io/examples/kernel/index.html)