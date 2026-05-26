import type { SourceConfigSource } from "../../types";
import { sourceKindIcon } from "./source-kind-icon";

export function SourceStatusCard({ source }: { source: SourceConfigSource }) {
  const Icon = sourceKindIcon(source.kind);
  const toneClass =
    {
      not_implemented: "border-amber-200 bg-amber-50 text-amber-900",
      ready: "border-emerald-200 bg-emerald-50 text-emerald-900",
      unavailable: "border-red-200 bg-red-50 text-red-900",
      unsupported: "border-red-200 bg-red-50 text-red-900",
    }[source.status] ?? "border-neutral-200 bg-neutral-50 text-neutral-800";

  return (
    <article className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <div className="flex items-start gap-3">
        <Icon className="mt-0.5 size-4 shrink-0 text-neutral-600" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-semibold text-neutral-950" title={source.spec}>
            {source.spec}
          </h3>
          <div className="mt-2 flex flex-wrap gap-2">
            <span className="inline-flex rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
              {source.kind}
            </span>
            <span
              className={`inline-flex rounded-md border px-2 py-1 text-xs font-semibold ${toneClass}`}
            >
              {source.status.replaceAll("_", " ")}
            </span>
          </div>
          {source.detail ? <p className="mt-2 text-xs text-neutral-600">{source.detail}</p> : null}
        </div>
      </div>
    </article>
  );
}
