import { Button } from "@moritzbrantner/ui/components/button";
import { Input } from "@moritzbrantner/ui/components/input";
import { Label } from "@moritzbrantner/ui/components/label";
import { NativeSelect } from "@moritzbrantner/ui/components/native-select";
import { Trash2 } from "lucide-react";
import type { SupportedSourceType } from "../../types";
import type { SourceDraft } from "./source-draft";

export function SourceDraftRow({
  index,
  onRemove,
  onUpdate,
  source,
  supportedTypes,
}: {
  index: number;
  onRemove: () => void;
  onUpdate: (patch: Partial<SourceDraft>) => void;
  source: SourceDraft;
  supportedTypes: SupportedSourceType[];
}) {
  const inputId = `source-spec-${source.id}`;
  const selectId = `source-kind-${source.id}`;
  const selectedSourceType = supportedTypes.find((sourceType) => sourceType.kind === source.kind);
  const plannedReadOnly = selectedSourceType ? !selectedSourceType.implemented : false;
  const hasKnownType = source.kind === "custom" || selectedSourceType !== undefined;

  return (
    <div className="grid gap-3 rounded-md border border-neutral-200 bg-neutral-50 p-3 md:grid-cols-[180px_minmax(0,1fr)_40px]">
      <div>
        <Label className="text-xs font-semibold text-neutral-700" htmlFor={selectId}>
          Source {index + 1}
        </Label>
        <NativeSelect
          className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200 disabled:cursor-not-allowed disabled:bg-neutral-100 disabled:text-neutral-500"
          disabled={plannedReadOnly}
          id={selectId}
          onChange={(event) => onUpdate({ kind: event.target.value })}
          value={source.kind}
        >
          {supportedTypes.map((sourceType) => (
            <option
              disabled={!sourceType.implemented}
              key={sourceType.kind}
              value={sourceType.kind}
            >
              {sourceType.label}
              {sourceType.implemented ? "" : " (planned)"}
            </option>
          ))}
          {!hasKnownType ? <option value={source.kind}>{source.kind}</option> : null}
          <option value="custom">Custom</option>
        </NativeSelect>
      </div>
      <div className="min-w-0">
        <Label className="text-xs font-semibold text-neutral-700" htmlFor={inputId}>
          Source spec
        </Label>
        <Input
          className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200 read-only:cursor-not-allowed read-only:bg-neutral-100 read-only:text-neutral-500"
          id={inputId}
          onChange={(event) => onUpdate({ spec: event.target.value })}
          placeholder="/images or minio://bucket/prefix"
          readOnly={plannedReadOnly}
          value={source.spec}
        />
      </div>
      <div className="flex items-end">
        <Button
          aria-label={`Remove source ${index + 1}`}
          variant="outline"
          className="inline-flex h-10 w-10 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-red-300 hover:bg-red-50 hover:text-red-700"
          onClick={onRemove}
          title="Remove source"
          type="button"
        >
          <Trash2 className="size-4" aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}
