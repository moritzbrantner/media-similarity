import type { SupportedSourceType } from "../../types";
import { sourceKindIcon } from "./source-kind-icon";

export function SupportedSourceTypeRow({ sourceType }: { sourceType: SupportedSourceType }) {
  const Icon = sourceKindIcon(sourceType.kind);

  return (
    <div className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <div className="flex items-start gap-3">
        <Icon className="mt-0.5 size-4 shrink-0 text-neutral-600" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <h3 className="text-sm font-semibold text-neutral-950">{sourceType.label}</h3>
            <span
              className={`shrink-0 rounded-md border px-2 py-1 text-xs font-semibold ${
                sourceType.implemented
                  ? "border-emerald-200 bg-emerald-50 text-emerald-800"
                  : "border-amber-200 bg-amber-50 text-amber-800"
              }`}
            >
              {sourceType.implemented ? "available" : "planned"}
            </span>
          </div>
          <p className="mt-1 truncate text-xs text-neutral-600" title={sourceType.example}>
            {sourceType.example}
          </p>
        </div>
      </div>
    </div>
  );
}
