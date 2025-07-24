#### B-Rep bodies can be classified as follows:
- Solid body – a body which has volume in 3D space. Think of a cube, sphere, or any other solid object.
- Shell body – a body which has surface in 3D space, but no volume. Think of a thin-walled object like a car body or a ship hull.
- Sheet body – a body which has surface in 2D space, but no volume. Think of a flat sheet of paper or a thin plate.
- Wireframe body – a body which has edges in 3D space, but no surface or volume. Think of a wireframe model of a 3D object, like a skeleton of a car or a building or a simple line drawing.
- Acorn body – a body which has a single point in 3D space, but no surface or volume. Think of a point in space, like a star or a planet.

#### Constraints/Solvers
- Geometric constraints – constraints that define the geometric relationships between the entities in a B-Rep body. For example, a constraint that defines that two edges are parallel or perpendicular, or that a point lies on a surface.
- Dimensional constraints – constraints that define the dimensions of the entities in a B-Rep body. For example, a constraint that defines the length of an edge or the radius of a circle.
- Solvers – algorithms that solve the constraints and compute the positions and orientations of the entities in a B-Rep body. For example, a solver that computes the position of a point on a surface given the constraints, or a solver that computes the intersection of two edges given the constraints.
- Parametric constraints – constraints that define the parameters of the entities in a B-Rep body. For example, a constraint that defines the angle between two edges or the distance between two points.

There are several constraints that can be applied to B-Rep bodies. 
Common basic constraints include -
- Distance and angle
- Coincidence
- Concentricity
- Perpendicularity or parallelism
- Tangency

Once the constraints are all applied, the actual positions of the components are then computed automatically by a CAD system component known as constraint solver

#### CAD File Formats
File formats can be monolithic or multi-file. 

- Monolithic formats contain all the information about the geometry, topology, and constraints in a single file, while modular formats separate the information into different files or modules.
Monolithic formats are easier to use and share, but can be larger and less flexible.

- Multifile formats allow for more complex and modular designs, but can be harder to manage and share. Each assembly or part can be stored in a separate file, and the relationships between them can be defined using references.

#### Common CAD file formats include
- STEP (Standard for the Exchange of Product Data) – a widely used standard for representing 3D models in a neutral format that can be exchanged between different CAD systems. It supports both solid and surface models, and can represent complex geometries with high precision.
- IGES (Initial Graphics Exchange Specification) – an older standard for representing 3D models in a neutral format. It supports both solid and surface models, but is less widely used than STEP. It is often used for exchanging models between legacy systems or for simple geometries.
- STL (Stereolithography) – a file format commonly used for 3D printing and rapid prototyping. It represents a 3D model as a collection of triangular facets, and is not suitable for representing complex geometries or parametric models. It is often used for exporting models from CAD systems to 3D printers or other applications that require a simple mesh representation.
- BREP (Boundary Representation) – a file format that represents 3D models as a collection of surfaces, edges, and vertices. It is often used for representing complex geometries in CAD systems, and can be used to create solid models by defining the boundaries of the solid. BREP files can be used to exchange models between different CAD systems, and can be used to create parametric models that can be modified by changing the parameters of the surfaces and edges.
- Parasolid – a proprietary file format developed by Siemens for representing 3D models in a neutral format. It supports both solid and surface models, and can represent complex geometries with high precision. It is often used in industrial applications and can be used to exchange models between different CAD systems.
- ACIS – a proprietary file format developed by Spatial for representing 3D models in a neutral format. It supports both solid and surface models, and can represent complex geometries with high precision. It is often used in industrial applications and can be used to exchange models between different CAD systems.

#### Layers
- Layers are used to organize the geometry and topology of a B-Rep body into logical groups. Each layer can contain different types of entities, such as edges, faces, or vertices, and can be used to control the visibility and appearance of the entities in the model.
E.g. Furniture layer, MEP layer, Electrical layer, etc.
E.g. Furniture layer can contain additonal layers like Walls, Windows, Doors, etc.

#### Metadata
- Metadata is broadly a part of PMI (Product Manufacturing Information) and is used to store additional information about the entities in a B-Rep body.
- Annotation or 2D drawing can be included in the metadata, such as dimensions, labels, or notes.