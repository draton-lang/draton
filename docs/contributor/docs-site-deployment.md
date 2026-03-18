---
title: Docs site deployment
sidebar_position: 40
---

# Docs site deployment

The documentation site is built with Docusaurus and deployed through GitHub Actions to GitHub Pages.

## Deployment model

The repo uses the GitHub Pages Actions flow:

1. install Node dependencies
2. build the Docusaurus site
3. upload the generated `build/` output as a Pages artifact
4. deploy that artifact to GitHub Pages

This is the public docs site path for the repository. It is separate from the release asset workflows.

## Custom domain

The docs site is deployed to:

- `https://docs.draton.lhqm.io.vn`

That domain is fixed directly in:

- `docusaurus.config.ts`
- `static/CNAME`

Workflow behavior:

- GitHub Actions builds the Docusaurus site
- the generated Pages artifact includes `CNAME`
- Docusaurus uses `baseUrl: /`

Repo-side deployment still requires GitHub Pages to be configured to use GitHub Actions as the source, and DNS for `docs.draton.lhqm.io.vn` must point to GitHub Pages.

## Local development

Install dependencies:

```sh
npm install
```

Run local preview:

```sh
npm run start
```

Build the static site:

```sh
npm run build
```

Local builds still work with the production URL in config because Pages deployment uses a root `baseUrl`.

## Contributor rules for docs changes

When editing docs:

- preserve the locked language philosophy
- keep examples aligned with actual parser/typechecker behavior
- do not introduce alternate canonical syntax
- keep self-host status truthful
- update both narrative docs and policy docs when a syntax/tooling truth changes

The docs site is public-facing, but it is still a language-engineering document set, not a marketing layer detached from implementation truth.
