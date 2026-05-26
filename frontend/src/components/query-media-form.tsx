import { Button, Input, Label } from "@moritzbrantner/ui";
import { FileText, Loader2, Search, Upload, X } from "lucide-react";
import type { FormEvent } from "react";

import type { IndexResponse } from "../types";
import { StatusMessage } from "./status-message";

type QueryMediaFormProps = {
  file: File | null;
  indexError: Error | null;
  lastIndex: IndexResponse | null;
  limit: number;
  ocrTextQuery: string;
  onFileChange: (file: File | null) => void;
  onLimitChange: (value: string) => void;
  onOcrTextQueryChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  searchError: Error | null;
  searchPending: boolean;
};

export function QueryMediaForm({
  file,
  indexError,
  lastIndex,
  limit,
  ocrTextQuery,
  onFileChange,
  onLimitChange,
  onOcrTextQueryChange,
  onSubmit,
  searchError,
  searchPending,
}: QueryMediaFormProps) {
  return (
    <form
      className="flex flex-col gap-4 rounded-lg border border-neutral-300 bg-white p-4 shadow-sm"
      onSubmit={onSubmit}
    >
      <div>
        <Label className="text-sm font-semibold text-neutral-900" htmlFor="query-image">
          Query media
        </Label>
        <Label
          className="mt-2 flex min-h-32 cursor-pointer flex-col items-center justify-center gap-2 rounded-md border border-dashed border-neutral-400 bg-neutral-50 px-4 py-5 text-center transition hover:border-emerald-600 hover:bg-emerald-50"
          htmlFor="query-image"
        >
          <Upload className="size-6 text-neutral-600" aria-hidden="true" />
          <span className="max-w-full truncate text-sm font-medium text-neutral-800">
            {file?.name ?? "Choose an image, video, audio, or PDF"}
          </span>
          <span className="text-xs text-neutral-500">
            PNG, JPEG, GIF, WebP, BMP, TIFF, MP4, MOV, WebM, MKV, AVI, MP3, WAV, FLAC, M4A, AAC,
            OGG, Opus, or PDF
          </span>
        </Label>
        <Input
          accept="image/*,video/*,audio/*,application/pdf,.pdf"
          className="sr-only"
          id="query-image"
          onChange={(event) => onFileChange(event.target.files?.[0] ?? null)}
          type="file"
        />
      </div>

      <div>
        <Label className="text-sm font-semibold text-neutral-900" htmlFor="limit">
          Result limit
        </Label>
        <Input
          className="mt-2 h-10 w-full rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
          id="limit"
          max={100}
          min={1}
          onChange={(event) => onLimitChange(event.target.value)}
          type="number"
          value={limit}
        />
      </div>

      <div>
        <Label className="text-sm font-semibold text-neutral-900" htmlFor="ocr-text-query">
          Text in media
        </Label>
        <div className="mt-2 flex h-10 items-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 transition focus-within:border-emerald-700 focus-within:ring-2 focus-within:ring-emerald-200">
          <FileText className="size-4 shrink-0 text-neutral-500" aria-hidden="true" />
          <Input
            className="min-w-0 flex-1 bg-transparent outline-none"
            id="ocr-text-query"
            onChange={(event) => onOcrTextQueryChange(event.target.value)}
            placeholder="Invoice, title, sign"
            type="search"
            value={ocrTextQuery}
          />
        </div>
      </div>

      <div className="flex gap-2">
        <Button
          className="inline-flex h-10 flex-1 items-center justify-center gap-2 rounded-md bg-emerald-700 px-4 text-sm font-semibold text-white shadow-sm transition hover:bg-emerald-800 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={!file || searchPending}
          type="submit"
        >
          {searchPending ? (
            <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          ) : (
            <Search className="size-4" aria-hidden="true" />
          )}
          <span>Search</span>
        </Button>
        {file ? (
          <Button
            aria-label="Clear selected media"
            variant="outline"
            className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-neutral-500 hover:bg-neutral-50"
            onClick={() => onFileChange(null)}
            title="Clear selected media"
            type="button"
          >
            <X className="size-4" aria-hidden="true" />
          </Button>
        ) : null}
      </div>

      <StatusMessage
        indexError={indexError}
        lastIndex={lastIndex}
        searchError={searchError}
        searchPending={searchPending}
      />
    </form>
  );
}
