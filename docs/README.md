# OpenGeometry documentation

This directory is the Mintlify site for [docs.opengeometry.io](https://docs.opengeometry.io). Keep
`docs.json` at this directory's root.

Every page lives in `opengeometry/`, because Mintlify builds a page's address from its file path:
`opengeometry/quickstart.mdx` is `/opengeometry/quickstart`. `docs.json`, `style.css`, the logo,
the favicon, `api-coverage.json` and `llms.txt` stay at the top. The redirects in `docs.json` send
the addresses of the 2.0 site to their new pages.

## Local development

Use Node.js 22 or later.

```sh
cd docs
npm ci
npm run dev
```

Mintlify prints the local preview address. Preview and strict validation download Mintlify's client
on the first run, so an internet connection is required.

Before you submit a change:

```sh
npm run format
npm run index
npm run check
```

`npm run check` checks formatting, that `api-coverage.json` and `llms.txt` are current, strict
Mintlify validation, and internal links with heading anchors. Run
`npm run links -- --check-external` when you change external links.

## Code in the pages

Every `ts` block in a page is type-checked against the built package by
`main/scripts/build/readme-code.test.mjs`, together with the README and MIGRATION.md. It runs in the
repository gate (`npm run check` at the root), because it needs `main/dist`. Write each block as a
complete module: its imports, `await OpenGeometry.create(...)`, then the code. Use `bash`, `js`,
`json` or `text` for blocks that must not be compiled.

## API coverage

`scripts/build-index.mjs` reads the exports of `main/opengeometry-three/index.ts` and checks that
each one is named on its API page. It writes `api-coverage.json` and `llms.txt`. When you add an
export, name it on an API page and add it to `PAGES` in the script if the default does not fit:
`OG_*` constants and parameter types go to the Constants page.

## Editing

- Add MDX pages under `opengeometry/` with `title` and `description` frontmatter and kebab-case
  paths. Link between pages with the full address, such as `/opengeometry/quickstart`.
- Add each page to its navigation group in `docs.json`, then run `npm run index`.
- `style.css` draws the tree lines of the Documentation tab. Its selector uses the Changelog route;
  update it if that route changes.
- Write in short sentences, one idea each, in the active voice. Define a term before you use it.
  Say what the reader sees and what to do.
- Describe only what the package does. Do not document 2.0 calls except on the migration page.
