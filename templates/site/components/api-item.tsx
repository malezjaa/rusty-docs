import Link from "next/link";
import {
  AtSign,
  ArrowUpRight,
  Box,
  Braces,
  CircleDot,
  Code2,
  Database,
  FileCode,
  Folder,
  GitMerge,
  Hash,
  Layers,
  List,
  Package,
  Rows3,
  Shapes,
  SquareFunction,
  Terminal,
  Type,
  Workflow,
  type LucideIcon,
} from "lucide-react";
import type { CSSProperties, ReactNode } from "react";
import { codeToTokens } from "shiki";
import { ApiImplDisclosure } from "./api-impl-disclosure";
import { ApiMemberPermalink } from "./api-member-permalink";
import siteConfig from "@/lib/config";

const itemKinds: Record<string, { label: string; icon: LucideIcon }> = {
  crate: { label: "Crate", icon: Package },
  binary: { label: "Binary", icon: Terminal },
  module: { label: "Module", icon: Folder },
  struct: { label: "Struct", icon: Box },
  enum: { label: "Enum", icon: List },
  trait: { label: "Trait", icon: Shapes },
  trait_alias: { label: "Trait alias", icon: Shapes },
  type_alias: { label: "Type alias", icon: Type },
  primitive: { label: "Primitive", icon: Type },
  constant: { label: "Constant", icon: Hash },
  static: { label: "Static", icon: Database },
  union: { label: "Union", icon: GitMerge },
  function: { label: "Function", icon: SquareFunction },
  macro: { label: "Macro", icon: Braces },
  proc_macro: { label: "Proc macro", icon: Braces },
  proc_attribute: { label: "Attribute macro", icon: AtSign },
  proc_derive: { label: "Derive macro", icon: FileCode },
  reexport: { label: "Re-export", icon: ArrowUpRight },
  field: { label: "Field", icon: Rows3 },
  variant: { label: "Variant", icon: CircleDot },
  method: { label: "Method", icon: SquareFunction },
  associated: { label: "Associated item", icon: Layers },
  implementation: { label: "Implementation", icon: Workflow },
};

function kindInfo(kind: string) {
  return itemKinds[kind] ?? { label: "Item", icon: Code2 };
}

export function ApiItemMark({
  kind,
  compact = false,
}: {
  kind: string;
  compact?: boolean;
}) {
  const { icon: Icon } = kindInfo(kind);

  return (
    <Icon
      aria-hidden="true"
      className={`api-kind-icon${compact ? " api-kind-icon--compact" : ""}`}
      data-kind={kind}
    />
  );
}

export function ApiItemLabel({
  kind,
  name,
  compact = false,
}: {
  kind: string;
  name: ReactNode;
  compact?: boolean;
}) {
  const info = kindInfo(kind);

  return (
    <span
      className={`api-item-label${compact ? " api-item-label--compact" : ""}`}
      data-kind={kind}
    >
      <ApiItemMark kind={kind} compact={compact} />
      <span>{name}</span>
      <span className="sr-only">{info.label}</span>
    </span>
  );
}

export function ApiItemLink({
  href,
  kind,
  name,
  rustPath,
}: {
  href: string;
  kind: string;
  name: string;
  rustPath: string;
}) {
  return (
    <Link
      className="api-item-link"
      data-kind={kind}
      href={href}
      title={rustPath}
    >
      <ApiItemLabel kind={kind} name={name} />
    </Link>
  );
}

export function ApiMember({
  kind,
  signature,
  id,
  children,
  showIcon = true,
}: {
  kind: string;
  signature: string;
  id?: string;
  children?: ReactNode;
  showIcon?: boolean;
}) {
  return (
    <>
      <div
        id={id}
        className={
          showIcon
            ? "api-member__signature"
            : "api-member__signature api-member__signature--no-icon"
        }
        data-kind={kind}
      >
        {showIcon ? (
          <span className="api-member__icon-code">
            <ApiItemMark compact kind={kind} />
          </span>
        ) : null}
        <pre className="api-member__code">
          <RustSignature signature={signature} />
        </pre>
        {id ? (
          <ApiMemberPermalink
            id={id}
            label={parseSignatureLinks(signature).code}
          />
        ) : null}
      </div>
      {children ? (
        <div
          className={
            showIcon
              ? "api-member__description"
              : "api-member__description api-member__description--no-icon"
          }
        >
          {children}
        </div>
      ) : null}
    </>
  );
}

export function ApiImpl({
  id,
  signature,
  children,
}: {
  id?: string;
  signature: string;
  children?: ReactNode;
}) {
  return (
    <ApiImplDisclosure
      defaultOpen={siteConfig.api.defaultImplsOpen}
      id={id}
      summary={
        <span className="api-member__code api-impl__code">
          <RustSignature signature={signature} />
        </span>
      }
    >
      {children}
    </ApiImplDisclosure>
  );
}

export function ApiSignature({ signature }: { signature: string }) {
  return (
    <pre className="api-member__code">
      <RustSignature signature={signature} />
    </pre>
  );
}

type SignatureLinkRange = {
  start: number;
  end: number;
  href: string;
  external: boolean;
};

function parseSignatureLinks(signature: string) {
  const links: SignatureLinkRange[] = [];
  let code = "";
  let cursor = 0;
  const marker = /\uE000([^\uE001]+)\uE001([^\uE002]*)\uE002/gu;

  for (const match of signature.matchAll(marker)) {
    const start = match.index ?? cursor;
    code += signature.slice(cursor, start);
    const label = match[2];
    const spec = match[1];
    const separator = spec.indexOf(":");
    const kind = separator < 0 ? "" : spec.slice(0, separator);
    const href = separator < 0 ? "" : spec.slice(separator + 1);
    const labelStart = code.length;
    code += label;
    if ((kind === "L" || kind === "E") && href) {
      links.push({
        start: labelStart,
        end: code.length,
        href,
        external: kind === "E",
      });
    }
    cursor = start + match[0].length;
  }
  code += signature.slice(cursor);
  return { code, links };
}

async function RustSignature({ signature }: { signature: string }) {
  const parsed = parseSignatureLinks(signature);
  const highlighted = await codeToTokens(parsed.code, {
    lang: "rust",
    themes: { light: "github-light", dark: "github-dark" },
    defaultColor: false,
  });
  const rendered: ReactNode[] = [];
  let cursor = 0;
  let activeLink: {
    link: SignatureLinkRange;
    children: ReactNode[];
    end: number;
  } | null = null;
  let tokenIndex = 0;

  const flushLink = () => {
    if (!activeLink) return;
    const { link, children, end } = activeLink;
    rendered.push(
      link.external ? (
        <a
          className="api-signature-type-link"
          href={link.href}
          key={`link-${link.start}-${end}`}
          rel="noopener noreferrer"
          target="_blank"
        >
          {children}
        </a>
      ) : (
        <Link
          className="api-signature-type-link"
          href={link.href}
          key={`link-${link.start}-${end}`}
        >
          {children}
        </Link>
      ),
    );
    activeLink = null;
  };

  for (const token of highlighted.tokens.flat()) {
    const tokenStart = token.offset;
    const tokenEnd = tokenStart + token.content.length;
    if (tokenStart > cursor) {
      flushLink();
      rendered.push(parsed.code.slice(cursor, tokenStart));
    }

    const link = parsed.links.find(
      (range) => tokenStart < range.end && tokenEnd > range.start,
    );
    const tokenNode = (
      <span
        key={`token-${tokenIndex++}`}
        style={token.htmlStyle as CSSProperties}
      >
        {token.content}
      </span>
    );
    if (
      link &&
      activeLink &&
      activeLink.link.href === link.href &&
      activeLink.link.external === link.external &&
      activeLink.end === tokenStart
    ) {
      activeLink.children.push(tokenNode);
      activeLink.end = tokenEnd;
    } else if (link) {
      flushLink();
      activeLink = { link, children: [tokenNode], end: tokenEnd };
    } else {
      flushLink();
      rendered.push(tokenNode);
    }
    cursor = tokenEnd;
  }
  flushLink();
  if (cursor < parsed.code.length) {
    rendered.push(parsed.code.slice(cursor));
  }

  return <code className="shiki api-rust-signature">{rendered}</code>;
}
