'use client';

import { ChevronRight } from 'lucide-react';
import { useEffect, useRef, type ReactNode } from 'react';

export function ApiImplDisclosure({
  id,
  summary,
  children,
}: {
  id?: string;
  summary: ReactNode;
  children?: ReactNode;
}) {
  const detailsRef = useRef<HTMLDetailsElement>(null);

  useEffect(() => {
    const openLinkedMember = () => {
      const rawId = window.location.hash.slice(1);
      if (!rawId) return;

      let targetId = rawId;
      try {
        targetId = decodeURIComponent(rawId);
      } catch {
        return;
      }
      const target = document.getElementById(targetId);
      const details = detailsRef.current;
      if (!target || !details || (target !== details && !details.contains(target))) return;

      details.open = true;
      requestAnimationFrame(() => target.scrollIntoView({ block: 'start' }));
    };

    openLinkedMember();
    window.addEventListener('hashchange', openLinkedMember);
    return () => window.removeEventListener('hashchange', openLinkedMember);
  }, []);

  return (
    <details className="api-impl" id={id} ref={detailsRef}>
      <summary className="api-impl__summary">
        <ChevronRight aria-hidden="true" className="api-impl__chevron" />
        {summary}
      </summary>
      {children ? <div className="api-impl__items">{children}</div> : null}
    </details>
  );
}
