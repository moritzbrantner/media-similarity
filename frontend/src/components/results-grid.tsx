import { FileImage, Loader2 } from "lucide-react";
import type { SearchResult } from "../types";
import { ResultCard } from "./results/result-card";

export function ResultsGrid({
  deletingId,
  onDelete,
  onUpdateTags,
  pending,
  results,
  searched,
  tagSavingId,
}: {
  deletingId?: string;
  onDelete?: (id: string) => void;
  onUpdateTags?: (id: string, tags: string[]) => void;
  pending: boolean;
  results: SearchResult[];
  searched: boolean;
  tagSavingId?: string;
}) {
  if (pending) {
    return (
      <div className="grid min-h-44 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600">
        <Loader2 className="size-7 animate-spin" aria-label="Loading search results" />
      </div>
    );
  }

  if (!searched) {
    return <EmptyResults text="Choose a query image, video, audio, or PDF and run a search." />;
  }

  if (results.length === 0) {
    return <EmptyResults text="No indexed media matched this query." />;
  }

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {results.map((result) => (
        <ResultCard
          deleting={deletingId === result.image.id}
          key={result.image.id}
          onDelete={onDelete}
          onUpdateTags={onUpdateTags}
          result={result}
          tagSaving={tagSavingId === result.image.id}
        />
      ))}
    </div>
  );
}

function EmptyResults({ text }: { text: string }) {
  return (
    <div className="grid min-h-44 place-items-center rounded-lg border border-neutral-300 bg-white p-8 text-center text-sm text-neutral-500">
      <div className="flex flex-col items-center gap-3">
        <FileImage className="size-8" aria-hidden="true" />
        <span>{text}</span>
      </div>
    </div>
  );
}
