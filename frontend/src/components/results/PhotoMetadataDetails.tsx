import type { PhotoMetadata } from "./result-formatting";

export function PhotoMetadataDetails({ metadata }: { metadata: PhotoMetadata }) {
  return (
    <details className="rounded-md border border-neutral-200 bg-neutral-50 p-3 text-sm">
      <summary className="cursor-pointer font-semibold text-neutral-800">Photo metadata</summary>
      <dl className="mt-3 grid gap-2">
        {metadata.raw.map((entry, index) => (
          <div className="grid gap-1" key={`${entry.namespace}-${entry.key}-${index}`}>
            <dt className="text-xs font-semibold uppercase text-neutral-500">
              {entry.namespace} · {entry.label || entry.key}
            </dt>
            <dd className="break-words text-neutral-900">{entry.value}</dd>
          </div>
        ))}
      </dl>
    </details>
  );
}
