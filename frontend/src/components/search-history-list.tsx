import { Button } from "@moritzbrantner/ui/components/button";
import { History } from "lucide-react";

import { formatHistoryTime } from "../jobs/job-utils";
import type { SearchHistoryItem } from "../search/types";

type SearchHistoryListProps = {
  activeSearchId: string | null;
  history: SearchHistoryItem[];
  onSelect: (item: SearchHistoryItem) => void;
};

export function SearchHistoryList({ activeSearchId, history, onSelect }: SearchHistoryListProps) {
  return (
    <aside className="h-fit rounded-lg border border-neutral-300 bg-white p-3 shadow-sm">
      <div className="flex items-center gap-2 px-1 pb-3 text-sm font-semibold text-neutral-950">
        <History className="size-4 text-neutral-600" aria-hidden="true" />
        <span>Search History</span>
      </div>

      {history.length === 0 ? (
        <div className="rounded-md border border-dashed border-neutral-300 bg-neutral-50 px-3 py-5 text-center text-sm text-neutral-500">
          No searches yet.
        </div>
      ) : (
        <ol className="flex flex-col gap-2">
          {history.map((item) => (
            <li key={item.id}>
              <Button
                aria-pressed={item.id === activeSearchId}
                variant={item.id === activeSearchId ? "default" : "outline"}
                className={`flex w-full min-w-0 flex-col gap-1 rounded-md border px-3 py-2 text-left transition ${
                  item.id === activeSearchId
                    ? "border-emerald-700 bg-emerald-50 text-emerald-950"
                    : "border-neutral-200 bg-white text-neutral-900 hover:border-neutral-400 hover:bg-neutral-50"
                }`}
                onClick={() => onSelect(item)}
                title={`${item.fileName}, ${item.response.count} result(s)`}
                type="button"
              >
                <span className="truncate text-sm font-semibold">{item.fileName}</span>
                <span className="flex items-center justify-between gap-2 text-xs text-neutral-600">
                  <span>{formatHistoryTime(item.searchedAt)}</span>
                  <span>
                    {item.response.scenes?.length
                      ? `${item.response.scenes.length} scene(s)`
                      : `${item.response.count} result(s)`}
                  </span>
                </span>
                <span className="text-xs text-neutral-500">Limit {item.limit}</span>
                {item.ocrTextQuery ? (
                  <span className="truncate text-xs text-neutral-500">
                    Text: {item.ocrTextQuery}
                  </span>
                ) : null}
              </Button>
            </li>
          ))}
        </ol>
      )}
    </aside>
  );
}
