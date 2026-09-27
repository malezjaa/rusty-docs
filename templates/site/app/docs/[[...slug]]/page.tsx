import { docsLlms, source } from '@/lib/source';
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
  ViewOptionsPopover,
} from 'fumadocs-ui/layouts/docs/page';
import { notFound } from 'next/navigation';
import Link from 'next/link';
import { cloneElement, isValidElement, type ComponentProps, type ReactNode } from 'react';
import { getMDXComponents } from '@/components/mdx';
import type { Metadata } from 'next';
import { createRelativeLink } from 'fumadocs-ui/mdx';
import {
  getApiItemKindForSlugs,
  getApiItemKindForUrl,
  getPageImageUrl,
  getPageMarkdownUrl,
  siteUrl,
} from '@/lib/shared';
import { ApiItemLabel, ApiItemMark } from '@/components/api-item';
import { MarkdownCopyButton } from '@/components/markdown-copy-button';
import type { TOCItemType } from 'fumadocs-core/toc';

export default async function Page(props: PageProps<'/docs/[[...slug]]'>) {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  const MDX = page.data.body;
  const markdownUrl = getPageMarkdownUrl(page).url;
  const markdown = await docsLlms.page(page);
  const itemKind = getApiItemKindForSlugs(page.slugs);
  const toc = mergeApiToc(page.data.toc, page.data.apiToc);
  const RelativeLink = createRelativeLink(source, page);
  function ApiAwareLink(
    props: ComponentProps<'a'> & { 'data-kind'?: string },
  ) {
    if (!props.href) return <a {...props} />;
    if (props.target === "_blank") return <a {...props} />;
    const kind = getApiItemKindForUrl(props.href) ?? props['data-kind'];
    if (kind) {
      return (
        <Link className="api-reference-link" data-kind={kind} href={props.href}>
          {withApiKindIcon(props.children, kind)}
        </Link>
      );
    }
    return <RelativeLink {...props} href={props.href} />;
  }

  return (
    <DocsPage toc={toc} full={page.data.full}>
      <DocsTitle>
        {itemKind ? (
          <ApiItemLabel kind={itemKind} name={page.data.title} />
        ) : (
          page.data.title
        )}
      </DocsTitle>
      <DocsDescription className="mb-0">{page.data.description}</DocsDescription>
      <div className="flex flex-row gap-2 items-center border-b pb-6">
        <MarkdownCopyButton markdown={markdown} />
        <ViewOptionsPopover markdownUrl={markdownUrl} />
      </div>
      <DocsBody>
        <MDX
          components={getMDXComponents({
            a: ApiAwareLink,
          })}
        />
      </DocsBody>
    </DocsPage>
  );
}

function mergeApiToc(
  toc: TOCItemType[],
  apiToc: {
    title: string;
    url: string;
    depth: number;
    insertAt: number;
    shiftNext: number;
    shiftDepth: number;
  }[],
): TOCItemType[] {
  const depthShifts = Array.from({ length: toc.length }, () => 0);
  for (const item of apiToc) {
    const end = Math.min(item.insertAt + item.shiftNext, toc.length);
    for (let index = Math.min(item.insertAt, toc.length); index < end; index += 1) {
      depthShifts[index] += item.shiftDepth;
    }
  }

  const merged: TOCItemType[] = [];
  let apiIndex = 0;
  for (let headingIndex = 0; headingIndex <= toc.length; headingIndex += 1) {
    while (apiIndex < apiToc.length && apiToc[apiIndex].insertAt <= headingIndex) {
      const item = apiToc[apiIndex];
      merged.push({ title: item.title, url: item.url, depth: item.depth });
      apiIndex += 1;
    }

    if (headingIndex < toc.length) {
      const item = toc[headingIndex];
      merged.push({
        ...item,
        depth: Math.min(item.depth + depthShifts[headingIndex], 6),
      });
    }
  }

  while (apiIndex < apiToc.length) {
    const item = apiToc[apiIndex];
    merged.push({ title: item.title, url: item.url, depth: item.depth });
    apiIndex += 1;
  }
  return merged;
}

function withApiKindIcon(children: ReactNode, kind: string): ReactNode {
  if (isValidElement<{ children?: ReactNode }>(children) && children.type === 'code') {
    return cloneElement(children, {
      children: (
        <>
          <ApiItemMark compact kind={kind} />
          {children.props.children}
        </>
      ),
    });
  }

  return (
    <>
      <ApiItemMark compact kind={kind} />
      {children}
    </>
  );
}

export async function generateStaticParams() {
  return source.generateParams();
}

export async function generateMetadata(props: PageProps<'/docs/[[...slug]]'>): Promise<Metadata> {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  return {
    title: page.data.title,
    description: page.data.description,
    ...(siteUrl ? { openGraph: { images: getPageImageUrl(page).url } } : {}),
  };
}
