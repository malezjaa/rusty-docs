import defaultMdxComponents from 'fumadocs-ui/mdx';
import type { MDXComponents } from 'mdx/types';
import type { ComponentProps } from 'react';
import { ApiImpl, ApiItemLink, ApiMember } from './api-item';

const DefaultImage = defaultMdxComponents.img;

function normalizeImageDimension(value: unknown): number | undefined {
  if (value === undefined || value === null || value === '') return undefined;

  const dimension = Number(value);
  if (!Number.isFinite(dimension) || dimension <= 0) return undefined;

  return Math.max(1, Math.round(dimension));
}

function MdxImage({ width, height, ...props }: ComponentProps<'img'>) {
  return (
    <DefaultImage
      {...props}
      width={normalizeImageDimension(width)}
      height={normalizeImageDimension(height)}
    />
  );
}

export function getMDXComponents(components?: MDXComponents) {
  return {
    ...defaultMdxComponents,
    img: MdxImage,
    ApiImpl,
    ApiItemLink,
    ApiMember,
    ...components,
  } satisfies MDXComponents;
}

export const useMDXComponents = getMDXComponents;

declare global {
  type MDXProvidedComponents = ReturnType<typeof getMDXComponents>;
}
