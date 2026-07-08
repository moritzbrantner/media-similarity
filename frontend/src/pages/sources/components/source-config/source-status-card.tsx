import type { SourceConfigSource, SourceInventory } from "../../../../types";
import { sourceKindIcon } from "./source-kind-icon";

export function SourceStatusCard({
  inventory,
  source,
}: {
  inventory?: SourceInventory | null;
  source: SourceConfigSource;
}) {
  const Icon = sourceKindIcon(source.kind);
  const toneClass =
    {
      degraded: "border-amber-200 bg-amber-50 text-amber-900",
      empty: "border-amber-200 bg-amber-50 text-amber-900",
      invalid: "border-red-200 bg-red-50 text-red-900",
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
          <p
            className="mt-1 truncate text-xs text-neutral-500"
            title={source.normalized_uri ?? source.spec}
          >
            {source.normalized_uri ?? source.spec}
          </p>
          <div className="mt-2 flex flex-wrap gap-2">
            <span className="inline-flex rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
              {source.kind}
            </span>
            <span
              className={`inline-flex rounded-md border px-2 py-1 text-xs font-semibold ${toneClass}`}
            >
              {source.status.replaceAll("_", " ")}
            </span>
            {source.capabilities?.requires_credentials ? (
              <span className="inline-flex rounded-md border border-sky-200 bg-sky-50 px-2 py-1 text-xs font-semibold text-sky-800">
                credentials
              </span>
            ) : null}
          </div>
          <p className="mt-2 truncate font-mono text-[11px] text-neutral-500" title={source.id}>
            {source.id}
          </p>
          {source.detail ? <p className="mt-2 text-xs text-neutral-600">{source.detail}</p> : null}
          {source.diagnostics?.length ? (
            <ul className="mt-2 grid gap-1 text-xs text-neutral-600">
              {source.diagnostics.slice(0, 3).map((diagnostic) => (
                <li key={`${diagnostic.code}-${diagnostic.message}`}>
                  {diagnostic.severity}: {diagnostic.message}
                </li>
              ))}
            </ul>
          ) : null}
          {inventory ? (
            <div className="mt-3 rounded-md border border-neutral-200 bg-white p-2 text-xs text-neutral-700">
              <div className="flex flex-wrap gap-x-4 gap-y-1">
                <span>{inventory.scanned_count} item(s)</span>
                {inventory.truncated ? <span>truncated</span> : null}
                {inventory.degraded_model_roles.length > 0 ? (
                  <span>Feature degraded: {inventory.degraded_model_roles.join(", ")}</span>
                ) : null}
              </div>
              <div className="mt-2 flex flex-wrap gap-1">
                {Object.entries(inventory.media_kind_counts).map(([kind, count]) => (
                  <span
                    className="rounded border border-neutral-200 bg-neutral-50 px-1.5 py-0.5"
                    key={kind}
                  >
                    {kind}: {count}
                  </span>
                ))}
              </div>
            </div>
          ) : null}
        </div>
      </div>
    </article>
  );
}
