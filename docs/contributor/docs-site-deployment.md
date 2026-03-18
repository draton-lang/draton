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

The site is meant to run behind a custom domain.

Repository variable required:

- `DOCS_CUSTOM_DOMAIN`

Workflow behavior:

- when the variable is set, the workflow builds with `DOCS_SITE_URL=https://<domain>`
- it writes a `CNAME` file into the site output
- Docusaurus uses `baseUrl: /`

This avoids hardcoding a potentially wrong domain in source while still keeping deploy behavior explicit.

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

Local builds do not require the custom-domain variable. They fall back to a localhost-safe site URL.

## Contributor rules for docs changes

When editing docs:

- preserve the locked language philosophy
- keep examples aligned with actual parser/typechecker behavior
- do not introduce alternate canonical syntax
- keep self-host status truthful
- update both narrative docs and policy docs when a syntax/tooling truth changes

The docs site is public-facing, but it is still a language-engineering document set, not a marketing layer detached from implementation truth.
