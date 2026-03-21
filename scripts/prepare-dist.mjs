/* eslint-env node */
import { cp, mkdir, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(scriptDir, "..");
const distDir = path.join(rootDir, "dist");
const kernelPkgDir = path.join(rootDir, "main", "opengeometry", "pkg");
const distKernelPkgDir = path.join(distDir, "opengeometry", "pkg");

const distRootKernelFiles = [
  "opengeometry_bg.wasm",
  "opengeometry_bg.wasm.d.ts",
];

const distKernelSubpathFiles = [
  "opengeometry.js",
  "opengeometry.d.ts",
  "opengeometry_bg.wasm.d.ts",
  "opengeometry_bg.wasm",
];

async function listFilesRecursively(dirPath) {
  const entries = await readdir(dirPath);
  const files = [];

  for (const entry of entries) {
    const entryPath = path.join(dirPath, entry);
    const entryStats = await stat(entryPath);
    if (entryStats.isDirectory()) {
      files.push(...(await listFilesRecursively(entryPath)));
      continue;
    }
    files.push(entryPath);
  }

  return files;
}

function toPosixSpecifier(relativePath) {
  const specifier = relativePath.split(path.sep).join("/");
  return specifier.startsWith(".") ? specifier : `./${specifier}`;
}

function ensureExplicitJsExtension(specifier) {
  if (!specifier.startsWith(".")) {
    return specifier;
  }

  const extension = path.extname(specifier);
  if (extension.length > 0) {
    return specifier;
  }

  if (specifier.endsWith("/")) {
    return `${specifier}index.js`;
  }

  return `${specifier}.js`;
}

async function rewriteKernelImportSpecifiers() {
  const declarationFiles = (await listFilesRecursively(distDir)).filter((filePath) =>
    filePath.endsWith(".d.ts")
  );
  const kernelTypeTarget = path.join(distDir, "opengeometry", "pkg", "opengeometry");
  const importPattern = /(["'])(?:\.\/)?(?:\.\.\/)+opengeometry\/pkg\/opengeometry\1/g;
  const fromPattern = /(from\s+["'])(\.{1,2}\/[^"']+)(["'])/g;
  const dynamicImportPattern = /(import\(\s*["'])(\.{1,2}\/[^"')]+)(["']\s*\))/g;

  for (const filePath of declarationFiles) {
    const contents = await readFile(filePath, "utf8");
    const withKernelSpecifierRewrite = contents.replace(importPattern, (_fullMatch, quote) => {
      const relative = path.relative(path.dirname(filePath), kernelTypeTarget);
      return `${quote}${toPosixSpecifier(relative)}${quote}`;
    });
    const withFromExtensions = withKernelSpecifierRewrite.replace(
      fromPattern,
      (_fullMatch, prefix, specifier, suffix) =>
        `${prefix}${ensureExplicitJsExtension(specifier)}${suffix}`
    );
    const replacement = withFromExtensions.replace(
      dynamicImportPattern,
      (_fullMatch, prefix, specifier, suffix) =>
        `${prefix}${ensureExplicitJsExtension(specifier)}${suffix}`
    );

    if (replacement !== contents) {
      await writeFile(filePath, replacement, "utf8");
    }
  }
}

function buildDistPackageJson(rootPackageJson) {
  return {
    name: rootPackageJson.name,
    version: rootPackageJson.version,
    description: rootPackageJson.description,
    author: rootPackageJson.author,
    license: rootPackageJson.license,
    type: "module",
    main: "./index.js",
    types: "./index.d.ts",
    exports: {
      ".": {
        types: "./index.d.ts",
        import: "./index.js",
        default: "./index.js",
      },
      "./opengeometry/pkg/opengeometry": {
        types: "./opengeometry/pkg/opengeometry.d.ts",
        import: "./opengeometry/pkg/opengeometry.js",
        default: "./opengeometry/pkg/opengeometry.js",
      },
      "./opengeometry_bg.wasm": "./opengeometry_bg.wasm",
      "./package.json": "./package.json",
    },
    keywords: rootPackageJson.keywords ?? [],
    repository: rootPackageJson.repository,
    bugs: rootPackageJson.bugs,
    homepage: rootPackageJson.homepage,
    peerDependencies: rootPackageJson.peerDependencies ?? {},
    dependencies: rootPackageJson.dependencies ?? {},
  };
}

async function main() {
  await mkdir(distDir, { recursive: true });
  await mkdir(distKernelPkgDir, { recursive: true });

  const distEntries = await readdir(distDir);
  for (const entry of distEntries) {
    if (!entry.endsWith(".tgz")) {
      continue;
    }
    await rm(path.join(distDir, entry), { force: true });
  }

  for (const fileName of distRootKernelFiles) {
    await cp(path.join(kernelPkgDir, fileName), path.join(distDir, fileName), { force: true });
  }

  for (const fileName of distKernelSubpathFiles) {
    await cp(path.join(kernelPkgDir, fileName), path.join(distKernelPkgDir, fileName), {
      force: true,
    });
  }

  await cp(path.join(rootDir, "README.md"), path.join(distDir, "README.md"), { force: true });

  await rewriteKernelImportSpecifiers();

  const rootPackageJson = JSON.parse(
    await readFile(path.join(rootDir, "package.json"), "utf8")
  );
  const distPackageJson = buildDistPackageJson(rootPackageJson);
  await writeFile(
    path.join(distDir, "package.json"),
    `${JSON.stringify(distPackageJson, null, 2)}\n`,
    "utf8"
  );
}

main().catch((error) => {
  console.error("Failed to prepare dist package:", error);
  process.exit(1);
});
