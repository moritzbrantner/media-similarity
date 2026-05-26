import { Button } from "@moritzbrantner/ui";
import {
  AlertCircle,
  CheckCircle2,
  Database,
  Loader2,
  Search,
  Settings,
  SlidersHorizontal,
  Users,
} from "lucide-react";

import type { AppView } from "../search/types";
import type { HealthResponse } from "../types";

type AppHeaderProps = {
  activeView: AppView;
  health: HealthResponse | undefined;
  healthError: boolean;
  healthLoading: boolean;
  indexActive: boolean;
  indexPending: boolean;
  onIndex: () => void;
  onViewChange: (view: AppView) => void;
  sourcesLabel: string;
};

const navItems: Array<{
  icon: typeof Search;
  label: string;
  pressedLabel: string;
  view: AppView;
}> = [
  { icon: Search, label: "Search", pressedLabel: "Open query page", view: "search" },
  { icon: Users, label: "Registry", pressedLabel: "Open inverse index", view: "inverse-index" },
  { icon: Settings, label: "Sources", pressedLabel: "Open media configuration", view: "configure" },
  {
    icon: SlidersHorizontal,
    label: "Indexing",
    pressedLabel: "Open indexing configuration",
    view: "indexing",
  },
];

export function AppHeader({
  activeView,
  health,
  healthError,
  healthLoading,
  indexActive,
  indexPending,
  onIndex,
  onViewChange,
  sourcesLabel,
}: AppHeaderProps) {
  return (
    <header className="flex flex-col gap-4 border-b border-neutral-300 pb-5 lg:flex-row lg:items-start lg:justify-between">
      <div className="min-w-0">
        <div className="flex items-center gap-2 text-sm font-medium text-emerald-700">
          {healthLoading ? (
            <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          ) : healthError ? (
            <AlertCircle className="size-4" aria-hidden="true" />
          ) : (
            <CheckCircle2 className="size-4" aria-hidden="true" />
          )}
          <span>{health?.status?.toUpperCase() ?? "STATUS"}</span>
        </div>
        <h1 className="mt-2 text-3xl font-semibold leading-tight tracking-normal text-neutral-950">
          Image Similarity Service
        </h1>
        <p className="mt-2 max-w-4xl truncate text-sm text-neutral-600" title={sourcesLabel}>
          Sources: {sourcesLabel}
        </p>
      </div>

      <div className="flex flex-col gap-2 sm:flex-row lg:items-center">
        <div className="flex min-h-10 flex-wrap rounded-md border border-neutral-300 bg-white p-1 shadow-sm">
          {navItems.map((item) => {
            const Icon = item.icon;
            return (
              <Button
                aria-label={item.pressedLabel}
                aria-pressed={activeView === item.view}
                className={`inline-flex items-center justify-center gap-2 rounded px-3 text-sm font-semibold transition ${
                  activeView === item.view
                    ? "bg-neutral-900 text-white"
                    : "text-neutral-700 hover:bg-neutral-100"
                }`}
                key={item.view}
                onClick={() => onViewChange(item.view)}
                type="button"
              >
                <Icon className="size-4" aria-hidden="true" />
                <span>{item.label}</span>
              </Button>
            );
          })}
        </div>
        <Button
          variant="outline"
          className="inline-flex h-10 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-400 bg-white px-4 text-sm font-semibold text-neutral-900 shadow-sm transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
          disabled={indexPending || indexActive}
          onClick={onIndex}
          type="button"
        >
          {indexPending || indexActive ? (
            <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          ) : (
            <Database className="size-4" aria-hidden="true" />
          )}
          <span>Index Sources</span>
        </Button>
      </div>
    </header>
  );
}
