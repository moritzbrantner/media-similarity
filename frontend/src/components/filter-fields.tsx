import { Button } from "@moritzbrantner/ui";
import { Input } from "@moritzbrantner/ui";
import { Label } from "@moritzbrantner/ui";
import { NativeSelect } from "@moritzbrantner/ui";
import { ArrowUpDown, ChevronDown, SlidersHorizontal } from "lucide-react";
import type { ComponentProps } from "react";
import { useState } from "react";
import { DEFAULT_METADATA_FILTERS } from "../search/defaults";
import { countActiveFilters } from "../search/filtering";
import type { MetadataFilters, ResultSortMode } from "../search/types";

export type FieldInputProps = ComponentProps<typeof Input> & {
  label: string;
};

export type FieldSelectProps = ComponentProps<typeof NativeSelect> & {
  label: string;
};

function FieldInput({ className = "", id, label, ...props }: FieldInputProps) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <Input
        className={`mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200 ${className}`}
        id={id}
        {...props}
      />
    </div>
  );
}

function FieldSelect({ children, className = "", id, label, ...props }: FieldSelectProps) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <NativeSelect className={`mt-1 w-full ${className}`} id={id} {...props}>
        {children}
      </NativeSelect>
    </div>
  );
}

export function MetadataFiltersPanel({
  filters,
  onChange,
  onSaveAsAlbum,
  ocrTextQuery = "",
  sourceTypeOptions,
}: {
  filters: MetadataFilters;
  onChange: (filters: MetadataFilters) => void;
  onSaveAsAlbum?: () => void;
  ocrTextQuery?: string;
  sourceTypeOptions: string[];
}) {
  const [filtersExpanded, setFiltersExpanded] = useState(true);

  function updateFilter<Key extends keyof MetadataFilters>(key: Key, value: MetadataFilters[Key]) {
    onChange({ ...filters, [key]: value });
  }

  const activeFilterCount = countActiveFilters(filters);
  const canSaveAsAlbum = activeFilterCount > 0 || ocrTextQuery.trim().length > 0;

  return (
    <fieldset className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <legend className="flex w-full items-center justify-between gap-2 px-1 text-sm font-semibold text-neutral-900">
        <Button
          aria-controls="metadata-filter-options"
          aria-expanded={filtersExpanded}
          className="flex items-center gap-2 px-0 text-sm font-semibold text-neutral-900 transition hover:text-neutral-700"
          onClick={() => setFiltersExpanded((current) => !current)}
          type="button"
          variant="ghost"
        >
          <SlidersHorizontal className="size-4 text-neutral-600" aria-hidden="true" />
          <span>Metadata filters</span>
          {activeFilterCount > 0 ? (
            <span className="rounded-full bg-emerald-100 px-2 py-0.5 text-xs font-semibold text-emerald-800">
              {activeFilterCount}
            </span>
          ) : null}
          <ChevronDown
            className={`size-4 text-neutral-500 transition-transform ${
              filtersExpanded ? "rotate-180" : ""
            }`}
            aria-hidden="true"
          />
        </Button>
        <span className="flex items-center gap-2">
          {onSaveAsAlbum ? (
            <Button
              variant="ghost"
              className="text-xs font-semibold text-emerald-800 transition hover:text-emerald-950 disabled:text-neutral-400"
              disabled={!canSaveAsAlbum}
              onClick={onSaveAsAlbum}
              type="button"
            >
              Save as album
            </Button>
          ) : null}
          {activeFilterCount > 0 ? (
            <Button
              variant="ghost"
              className="text-xs font-semibold text-emerald-800 transition hover:text-emerald-950"
              onClick={() => onChange(DEFAULT_METADATA_FILTERS)}
              type="button"
            >
              Clear {activeFilterCount}
            </Button>
          ) : null}
        </span>
      </legend>

      <div
        className={`${filtersExpanded ? "mt-3 grid" : "hidden"} gap-3 md:grid-cols-2 xl:grid-cols-4`}
        id="metadata-filter-options"
      >
        <FieldInput
          id="name-query"
          label="Name or path"
          onChange={(event) => updateFilter("nameQuery", event.target.value)}
          placeholder="Filename or folder"
          type="search"
          value={filters.nameQuery}
        />

        <FieldSelect
          id="source-type"
          label="Source type"
          onChange={(event) => updateFilter("sourceType", event.target.value)}
          value={filters.sourceType}
        >
          <option value="all">All sources</option>
          {sourceTypeOptions.map((sourceType) => (
            <option key={sourceType} value={sourceType}>
              {sourceType}
            </option>
          ))}
        </FieldSelect>

        <FieldInput
          id="person-id"
          label="Person ID"
          onChange={(event) => updateFilter("personId", event.target.value)}
          placeholder="person-..."
          type="search"
          value={filters.personId}
        />

        <FieldSelect
          id="media-kind"
          label="Media type"
          onChange={(event) =>
            updateFilter("mediaKind", event.target.value as MetadataFilters["mediaKind"])
          }
          value={filters.mediaKind}
        >
          <option value="all">All media</option>
          <option value="static_image">Images only</option>
          <option value="animated_gif">GIFs only</option>
          <option value="video_scene">Video scenes only</option>
          <option value="audio">Audio only</option>
          <option value="pdf_document">PDF documents only</option>
          <option value="pdf_page">PDF pages only</option>
        </FieldSelect>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldSelect
            id="near-duplicate"
            label="Duplicate status"
            onChange={(event) =>
              updateFilter("nearDuplicate", event.target.value as MetadataFilters["nearDuplicate"])
            }
            value={filters.nearDuplicate}
          >
            <option value="all">All matches</option>
            <option value="only">Near duplicates only</option>
            <option value="exclude">Exclude near duplicates</option>
          </FieldSelect>

          <FieldSelect
            id="orientation"
            label="Orientation"
            onChange={(event) =>
              updateFilter("orientation", event.target.value as MetadataFilters["orientation"])
            }
            value={filters.orientation}
          >
            <option value="all">Any orientation</option>
            <option value="landscape">Landscape</option>
            <option value="portrait">Portrait</option>
            <option value="square">Square</option>
          </FieldSelect>
        </div>

        <FieldInput
          id="camera-query"
          label="Camera/lens"
          onChange={(event) => updateFilter("cameraQuery", event.target.value)}
          placeholder="Make, model, or lens"
          type="search"
          value={filters.cameraQuery}
        />

        <FieldInput
          id="keyword-query"
          label="Keyword"
          onChange={(event) => updateFilter("keywordQuery", event.target.value)}
          placeholder="Tag or subject"
          type="search"
          value={filters.keywordQuery}
        />

        <FieldSelect
          id="has-gps"
          label="GPS metadata"
          onChange={(event) =>
            updateFilter("hasGps", event.target.value as MetadataFilters["hasGps"])
          }
          value={filters.hasGps}
        >
          <option value="all">Any GPS metadata</option>
          <option value="yes">Has GPS</option>
          <option value="no">No GPS</option>
        </FieldSelect>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldInput
            id="date-from"
            label="Modified after"
            onChange={(event) => updateFilter("dateFrom", event.target.value)}
            type="date"
            value={filters.dateFrom}
          />

          <FieldInput
            id="date-to"
            label="Modified before"
            onChange={(event) => updateFilter("dateTo", event.target.value)}
            type="date"
            value={filters.dateTo}
          />
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldInput
            id="capture-date-from"
            label="Captured after"
            onChange={(event) => updateFilter("captureDateFrom", event.target.value)}
            type="date"
            value={filters.captureDateFrom}
          />

          <FieldInput
            id="capture-date-to"
            label="Captured before"
            onChange={(event) => updateFilter("captureDateTo", event.target.value)}
            type="date"
            value={filters.captureDateTo}
          />
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldInput
            id="min-size"
            label="Min file size (MB)"
            min={0}
            onChange={(event) => updateFilter("minSizeMb", event.target.value)}
            placeholder="Any"
            step="0.1"
            type="number"
            value={filters.minSizeMb}
          />

          <FieldInput
            id="max-size"
            label="Max file size (MB)"
            min={0}
            onChange={(event) => updateFilter("maxSizeMb", event.target.value)}
            placeholder="Any"
            step="0.1"
            type="number"
            value={filters.maxSizeMb}
          />
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldInput
            id="min-width"
            label="Minimum width"
            min={0}
            onChange={(event) => updateFilter("minWidth", event.target.value)}
            placeholder="Any"
            type="number"
            value={filters.minWidth}
          />

          <FieldInput
            id="min-height"
            label="Minimum height"
            min={0}
            onChange={(event) => updateFilter("minHeight", event.target.value)}
            placeholder="Any"
            type="number"
            value={filters.minHeight}
          />
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldInput
            id="max-width"
            label="Maximum width"
            min={0}
            onChange={(event) => updateFilter("maxWidth", event.target.value)}
            placeholder="Any"
            type="number"
            value={filters.maxWidth}
          />

          <FieldInput
            id="max-height"
            label="Maximum height"
            min={0}
            onChange={(event) => updateFilter("maxHeight", event.target.value)}
            placeholder="Any"
            type="number"
            value={filters.maxHeight}
          />
        </div>
      </div>
    </fieldset>
  );
}

export function ResultSortSelect({
  onChange,
  value,
}: {
  onChange: (sortMode: ResultSortMode) => void;
  value: ResultSortMode;
}) {
  return (
    <Label className="flex w-full items-center gap-2 sm:w-auto">
      <span className="flex shrink-0 items-center gap-2 text-sm font-semibold text-neutral-800">
        <ArrowUpDown className="size-4 text-neutral-600" aria-hidden="true" />
        Sort
      </span>
      <NativeSelect
        className="min-w-48 flex-1 sm:flex-none"
        onChange={(event) => onChange(event.target.value as ResultSortMode)}
        value={value}
      >
        <option value="phash_distance">pHash distance</option>
        <option value="vector_score">Visual score</option>
        <option value="captured_newest">Newest captured</option>
        <option value="modified_newest">Newest modified</option>
        <option value="size_largest">Largest file</option>
        <option value="filename">Filename</option>
      </NativeSelect>
    </Label>
  );
}
