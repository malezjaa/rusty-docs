'use client';

import { Check, Copy } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

export function MarkdownCopyButton({ markdown }: { markdown: string }) {
  const [status, setStatus] = useState<'idle' | 'copied' | 'error'>('idle');
  const resetTimer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(resetTimer.current), []);

  async function copyMarkdown() {
    try {
      if (navigator.clipboard?.writeText) {
        try {
          await navigator.clipboard.writeText(markdown);
        } catch {
          if (!copyWithFallback(markdown)) throw new Error('Clipboard is unavailable');
        }
      } else if (!copyWithFallback(markdown)) {
        throw new Error('Clipboard is unavailable');
      }
      setStatus('copied');
    } catch {
      setStatus('error');
    }
    window.clearTimeout(resetTimer.current);
    resetTimer.current = window.setTimeout(() => setStatus('idle'), 1800);
  }

  const label =
    status === 'copied' ? 'Copied Markdown' : status === 'error' ? 'Copy failed' : 'Copy Markdown';

  return (
    <button
      aria-live="polite"
      className="markdown-copy-button"
      onClick={copyMarkdown}
      type="button"
    >
      {status === 'copied' ? (
        <Check aria-hidden="true" size={15} />
      ) : (
        <Copy aria-hidden="true" size={15} />
      )}
      {label}
    </button>
  );
}

function copyWithFallback(value: string) {
  const textarea = document.createElement('textarea');
  textarea.value = value;
  textarea.setAttribute('readonly', '');
  textarea.style.position = 'fixed';
  textarea.style.opacity = '0';
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand('copy');
  textarea.remove();
  return copied;
}
