import { Button } from "@moritzbrantner/ui";
import { AlertCircle, CheckCircle2, Database, Loader2 } from "lucide-react";
import { NavLink } from "react-router";

import { appRouteNavItems } from "../app/routes";
import type { HealthResponse } from "../types";

type AppHeaderProps = {
  health: HealthResponse | undefined;
  healthError: boolean;
  healthLoading: boolean;
  indexActive: boolean;
  indexPending: boolean;
  onIndex: () => void;
  sourcesLabel: string;
};

export function AppHeader({
  health,
  healthError,
  healthLoading,
  indexActive,
  indexPending,
  onIndex,
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

      <div className="flex w-full flex-col gap-2 lg:w-auto lg:items-end">
        <nav
          aria-label="Primary"
          className="grid grid-cols-2 gap-1 rounded-md border border-neutral-300 bg-white p-1 shadow-sm sm:flex sm:flex-wrap sm:justify-end"
        >
          {appRouteNavItems.map((item) => {
            const Icon = item.icon;
            return (
              <NavLink
                aria-label={item.screenReaderLabel}
                className={({ isActive }) =>
                  `inline-flex h-9 min-w-0 items-center justify-center gap-2 rounded px-3 text-sm font-semibold transition ${
                    isActive ? "bg-neutral-900 text-white" : "text-neutral-700 hover:bg-neutral-100"
                  }`
                }
                end={item.path === "/"}
                key={item.id}
                to={item.path}
              >
                <Icon className="size-4" aria-hidden="true" />
                <span className="truncate">{item.label}</span>
              </NavLink>
            );
          })}
        </nav>
        <Button
          variant="outline"
          className="inline-flex h-10 w-full shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-400 bg-white px-4 text-sm font-semibold text-neutral-900 shadow-sm transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60 sm:w-auto"
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
