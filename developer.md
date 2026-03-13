# Developer Documentation

### Project Structure
- `main/opengeometry`: Core Rust project that compiles to WebAssembly.
- `main/opengeometry-three`: Three.js wrapper around the core WebAssembly package.
- `dist`: Distribution folder containing the built packages.

Rust is used for performance-critical geometry computations, while Three.js is used for rendering in web applications. 
We use `wasm-pack` to compile the Rust code to WebAssembly and generate JavaScript bindings.
- Install `wasm-pack` using `brew install wasm-pack` (macOS).

Primitives and Shapes are created using Rust and exposed to JavaScript via WebAssembly.

### Local Development

#### Building Cargo Project
- Run `npm run build-core` to build the Rust project and generate WebAssembly bindings.

#### Building Three.js Package
The Three.js package depends on the core WebAssembly package. Therefore, ensure to build the core package first.
1. Run `npm run build` to produce the root distribution bundle in `dist/`.
2. Run `npm run build-example-three` to build the standalone example catalog in `main/opengeometry-three/examples-dist/`.

#### Testing Examples
1. After building the examples, open `main/opengeometry-three/examples-dist/index.html` through a static file server or use the Vite preview command from `main/opengeometry-three`.
2. Use the catalog pages directly for validation instead of copying artifacts into sibling local repositories.

**Project Structure**

```
OpenGeometry/
├── dist/
├── main/
│   ├── opengeometry/
│   └── opengeometry-three/
└── main/opengeometry-three/examples-dist/
```

### Release Process
1. Update the version in `package.json` and `main/opengeometry/Cargo.toml`.
2. Push the changes to the `main` branch.
3. Pushing to `main` will trigger a GitHub Action that builds the project and creates a release on GitHub.
4. Github action handles publishing to npm and crate.
