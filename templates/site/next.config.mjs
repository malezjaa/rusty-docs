import { createMDX } from 'fumadocs-mdx/next';
import { readFileSync } from 'node:fs';

const withMDX = createMDX();
const site = JSON.parse(
  readFileSync(new URL('./rusty-docs-site.json', import.meta.url), 'utf8'),
);

/** @type {import('next').NextConfig} */
const config = {
  output: 'export',
  trailingSlash: true,
  ...(site.basePath !== '/' ? { basePath: site.basePath } : {}),
  reactStrictMode: true,
};

export default withMDX(config);
