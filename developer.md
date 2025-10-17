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
1. Run `npm run build-local` to build the project and copy the output to the local OpenPlans repository.
2. Make sure to have the OpenPlans repository cloned locally for this to work.

#### Testing Examples
1. After building the project, run `npm run make-examples` to copy the built files to the OpenGeometry-Examples repository.
2. Open the examples in a web server (e.g., using Live Server in VSCode) to test the changes.
- Note that `OpenGeometry-Examples` should be cloned locally for this to work and the folder structure should match.

**Project Structure**

```
OpenGeometry/
├── dist/
├── main/
│   ├── opengeometry/
│   └── opengeometry-three/
├── OpenGeometry-Examples/
│   ├── core/
│   └── src/
└── OpenPlans/
    ├── src/
    └── kernel/
```

### Release Process
1. Update the version in `package.json` and `main/opengeometry/Cargo.toml`.
2. Push the changes to the `main` branch.
3. Pushing to `main` will trigger a GitHub Action that builds the project and creates a release on GitHub.
4. Github action handles publishing to npm and crate.
