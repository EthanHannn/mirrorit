export function DiagnosticsSection({ diagnostics }: { diagnostics: string[] }) {
  if (!diagnostics.length) {
    return null;
  }

  return (
    <section
      aria-labelledby="diagnostics-heading"
      className="mt-6 border-l border-warning bg-warning/5 px-4 py-3"
    >
      <h3 id="diagnostics-heading" className="text-sm font-medium">
        需要注意
      </h3>
      <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
        {diagnostics.map((diagnostic) => (
          <li key={diagnostic}>{diagnostic}</li>
        ))}
      </ul>
    </section>
  );
}
