'use client';

import { Check, Link2 } from 'lucide-react';
import { useState } from 'react';

export function ApiMemberPermalink({
  id,
  label,
}: {
  id: string;
  label: string;
}) {
  const [status, setStatus] = useState<'copied' | 'failed' | null>(null);

  const copyLink = async () => {
    const url = new URL(window.location.href);
    url.hash = id;

    try {
      await navigator.clipboard.writeText(url.toString());
      setStatus('copied');
      window.setTimeout(() => setStatus(null), 1500);
    } catch {
      setStatus('failed');
      window.setTimeout(() => setStatus(null), 1500);
    }
  };

  return (
    <button
      aria-label={
        status === 'copied'
          ? 'Link copied'
          : status === 'failed'
            ? 'Could not copy link'
            : `Copy link to ${label}`
      }
      className="api-member__permalink"
      onClick={copyLink}
      title={
        status === 'copied'
          ? 'Link copied'
          : status === 'failed'
            ? 'Could not copy link'
            : 'Copy link'
      }
      type="button"
    >
      {status === 'copied' ? (
        <Check aria-hidden="true" />
      ) : (
        <Link2 aria-hidden="true" />
      )}
    </button>
  );
}
