# Rusty Docs site

This Next.js project uses the official Fumadocs static starter.

Rusty Docs copies custom Markdown and MDX files into `content/docs/content`.
The generated static site is written to `out/`. Rusty Docs replaces
`content/docs` on each build, so edit the configured source content directory
in the Cargo workspace instead of these generated files.

Run the development server with `npm run dev`, or build the static site with
`npm run build`.
