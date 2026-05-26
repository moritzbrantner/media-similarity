import { Alert, AlertDescription } from "@moritzbrantner/ui";
import { AlertCircle, CheckCircle2, Loader2, RotateCw } from "lucide-react";
import type { ReactNode } from "react";
import type { IndexResponse } from "../types";

export function StatusMessage({
  indexError,
  lastIndex,
  searchError,
  searchPending,
}: {
  indexError: Error | null;
  lastIndex: IndexResponse | null;
  searchError: Error | null;
  searchPending: boolean;
}) {
  if (searchPending) {
    return (
      <Message
        icon={<Loader2 className="size-4 animate-spin" />}
        text="Searching indexed media."
        tone="info"
      />
    );
  }

  if (searchError) {
    return (
      <Message icon={<AlertCircle className="size-4" />} text={searchError.message} tone="error" />
    );
  }

  if (indexError) {
    return (
      <Message icon={<AlertCircle className="size-4" />} text={indexError.message} tone="error" />
    );
  }

  if (lastIndex) {
    const tone = lastIndex.failed > 0 ? "warn" : "ok";
    const text = `Indexed ${lastIndex.indexed} media item(s), skipped ${lastIndex.skipped}, pruned ${lastIndex.pruned}, failed ${lastIndex.failed}.`;
    return <Message icon={<CheckCircle2 className="size-4" />} text={text} tone={tone} />;
  }

  return (
    <Message
      icon={<RotateCw className="size-4" />}
      text="Index sources before searching fresh data."
      tone="info"
    />
  );
}

export function Message({
  icon,
  text,
  tone,
}: {
  icon: ReactNode;
  text: string;
  tone: "error" | "info" | "ok" | "warn";
}) {
  const toneClass = {
    error: "border-red-200 bg-red-50 text-red-800",
    info: "border-neutral-200 bg-neutral-50 text-neutral-700",
    ok: "border-emerald-200 bg-emerald-50 text-emerald-800",
    warn: "border-amber-200 bg-amber-50 text-amber-800",
  }[tone];

  return (
    <Alert
      variant={tone === "error" ? "destructive" : "default"}
      className={`flex min-h-11 items-start gap-2 rounded-md border px-3 py-2 text-sm ${toneClass}`}
    >
      <span className="mt-0.5 shrink-0" aria-hidden="true">
        {icon}
      </span>
      <AlertDescription>{text}</AlertDescription>
    </Alert>
  );
}
