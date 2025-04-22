// components/cli.tsx  (only the Copy helper changed)
import React, { useState } from "react";
import { Copy as CopyIcon, Check } from "lucide-react";

interface CopyProps {
  /** Text that will be copied to the clipboard */
  text: string;
  /**
   * When `block` is true we render as an absolute‑positioned button
   * in the parent `.relative` container – perfect for <pre><code> blocks.
   * When false (default) we render inline (used after <Command />).
   */
  block?: boolean;
  className?: string;
}

export function Copy({ text, block = false, className = "" }: CopyProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    } catch (_) {
      /* ignore */
    }
  };

  const base =
    "inline-flex items-center gap-1 text-xs opacity-60 hover:opacity-100 transition";
  const overlay =
    "absolute top-2 right-2 bg-muted/70 backdrop-blur px-2 py-1 rounded shadow";

  return (
    <button
      onClick={handleCopy}
      className={`${base} ${block ? overlay : ""} ${className}`}
      title="Copy to clipboard"
    >
      {copied ? (
        <>
          <Check size={14} className="stroke-green-500" />
          Copied
        </>
      ) : (
        <>
          <CopyIcon size={14} />
          Copy
        </>
      )}
    </button>
  );
}
