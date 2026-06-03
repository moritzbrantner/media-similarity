import { Button } from "@moritzbrantner/ui";
import {
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  Cloud,
  Loader2,
} from "lucide-react";
import type { ModelRuntimeStatus } from "../../types";
import { Message } from "../status-message";

export function ModelStatusPanel({
  actionPendingRole,
  error,
  loading,
  models,
  onDownloadAll,
  onDownload,
  onEnable,
}: {
  actionPendingRole?: string;
  error: Error | null;
  loading: boolean;
  models: ModelRuntimeStatus[];
  onDownloadAll: () => void;
  onDownload: (role: string, model?: string | null) => void;
  onEnable: (role: string, model?: string | null) => void;
}) {
  const downloadingAll = actionPendingRole === "all";

  return (
    <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-sm font-semibold text-neutral-950">Model Status</h2>
        <Button
          variant="outline"
          className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
          disabled={downloadingAll || loading || models.length === 0}
          onClick={onDownloadAll}
          type="button"
        >
          {downloadingAll ? (
            <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <Cloud className="size-3.5" aria-hidden="true" />
          )}
          <span>Download every model</span>
        </Button>
      </div>
      {loading ? (
        <div className="mt-3 flex items-center gap-2 text-sm text-neutral-600">
          <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          <span>Checking models.</span>
        </div>
      ) : error ? (
        <div className="mt-3">
          <Message
            icon={<AlertCircle className="size-4" />}
            text={error.message}
            tone="error"
          />
        </div>
      ) : (
        <div className="mt-3 grid gap-3">
          {models.map((model) => (
            <ModelStatusCard
              key={model.role}
              model={model}
              onDownload={onDownload}
              onEnable={onEnable}
              pending={downloadingAll || actionPendingRole === model.role}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function ModelStatusCard({
  model,
  onDownload,
  onEnable,
  pending,
}: {
  model: ModelRuntimeStatus;
  onDownload: (role: string, model?: string | null) => void;
  onEnable: (role: string, model?: string | null) => void;
  pending: boolean;
}) {
  return (
    <article className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="text-sm font-semibold text-neutral-950">
            {model.label}
          </h3>
          <p
            className="mt-1 truncate text-xs text-neutral-600"
            title={model.configured}
          >
            {model.configured}
          </p>
        </div>
        <span
          className={`shrink-0 rounded-md border px-2 py-1 text-xs font-semibold ${
            model.active
              ? "border-emerald-200 bg-emerald-50 text-emerald-800"
              : model.blocking
                ? "border-red-200 bg-red-50 text-red-800"
                : model.cached
                  ? "border-sky-200 bg-sky-50 text-sky-800"
                  : "border-amber-200 bg-amber-50 text-amber-800"
          }`}
        >
          {model.active
            ? "active"
            : model.blocking
              ? "blocking"
              : model.cached
                ? "cached"
                : "missing"}
        </span>
      </div>
      {model.blocking ? (
        <div className="mt-2 flex items-start gap-2 rounded-md border border-red-200 bg-red-50 px-2 py-2 text-xs text-red-800">
          <AlertTriangle
            className="mt-0.5 size-3.5 shrink-0"
            aria-hidden="true"
          />
          <span>
            This required model blocks indexing and search until it is
            downloaded.
          </span>
        </div>
      ) : model.required_action ? (
        <div className="mt-2 flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-2 py-2 text-xs text-amber-800">
          <AlertCircle
            className="mt-0.5 size-3.5 shrink-0"
            aria-hidden="true"
          />
          <span>
            This model is enabled but needs {model.required_action} before
            analysis runs.
          </span>
        </div>
      ) : null}
      {model.detail ? (
        <p className="mt-2 text-xs text-neutral-600">{model.detail}</p>
      ) : null}
      <div className="mt-3 flex flex-wrap gap-2">
        <Button
          variant="outline"
          className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
          disabled={pending || model.cached}
          onClick={() => onDownload(model.role, model.configured)}
          type="button"
        >
          {pending && !model.cached ? (
            <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <Cloud className="size-3.5" aria-hidden="true" />
          )}
          <span>Download</span>
        </Button>
        <Button
          variant="outline"
          className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={pending || !model.cached}
          onClick={() => onEnable(model.role, model.configured)}
          type="button"
        >
          {pending && model.cached ? (
            <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <CheckCircle2 className="size-3.5" aria-hidden="true" />
          )}
          <span>Enable</span>
        </Button>
      </div>
    </article>
  );
}
