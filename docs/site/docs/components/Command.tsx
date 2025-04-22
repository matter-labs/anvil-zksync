export function Command({ children }: { children: React.ReactNode }) {
  return (
    <code className="font-mono bg-muted/60 px-1.5 py-0.5 rounded text-sm">
      {children}
    </code>
  );
}