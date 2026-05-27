import { Button } from "@moritzbrantner/ui/components/button";
import { AlertCircle, FileText, ImageIcon, Loader2, Mic2, RotateCw, Users } from "lucide-react";
import type { ReactNode } from "react";
import { mediaKindLabel } from "../lib/format";
import type { InverseIndexLocation, InverseIndexResponse } from "../types";
import { Message } from "./status-message";

export function InverseIndexPage({
  data,
  error,
  loading,
  onRefresh,
  refreshing,
}: {
  data: InverseIndexResponse | null;
  error: Error | null;
  loading: boolean;
  onRefresh: () => void;
  refreshing: boolean;
}) {
  if (loading && !data) {
    return (
      <div className="grid min-h-96 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600 shadow-sm">
        <Loader2 className="size-7 animate-spin" aria-label="Loading inverse index" />
      </div>
    );
  }

  if (error) {
    return <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />;
  }

  const people = sortPeopleEntries(data?.people ?? []);
  const speakers = sortSpeakerEntries(data?.speakers ?? []);

  return (
    <section className="flex flex-col gap-5">
      <div className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div className="min-w-0">
            <h2 className="text-lg font-semibold text-neutral-950">Inverse Index</h2>
            <p className="mt-1 text-sm text-neutral-600">
              Key findings grouped by identity with the indexed media locations where they occur.
            </p>
          </div>
          <Button
            variant="outline"
            className="inline-flex h-10 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
            disabled={refreshing}
            onClick={onRefresh}
            type="button"
          >
            {refreshing ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <RotateCw className="size-4" aria-hidden="true" />
            )}
            <span>Refresh</span>
          </Button>
        </div>

        <dl className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <RegistryMetric label="Indexed media" value={data?.indexed_media ?? 0} />
          <RegistryMetric label="People" value={people.length} />
          <RegistryMetric label="Speakers" value={speakers.length} />
          <RegistryMetric label="Decode warnings" value={data?.errors.length ?? 0} />
        </dl>
      </div>

      {data?.errors.length ? (
        <Message
          icon={<AlertCircle className="size-4" />}
          text={`${data.errors.length} indexed payload(s) could not be read. The registry shows the remaining media.`}
          tone="warn"
        />
      ) : null}

      <div className="grid gap-5 xl:grid-cols-2">
        <RegistrySection
          emptyText="No indexed people yet. Enable face analysis, index sources, then refresh this page."
          entries={people}
          icon={<Users className="size-4 text-neutral-600" aria-hidden="true" />}
          kind="person"
          title="Depicted People"
        />
        <RegistrySection
          emptyText="No recognized speakers yet. Enable audio analysis, index audio sources, then refresh this page."
          entries={speakers}
          icon={<Mic2 className="size-4 text-neutral-600" aria-hidden="true" />}
          kind="speaker"
          title="Recognized Speakers"
        />
      </div>
    </section>
  );
}

function RegistryMetric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2">
      <dt className="text-xs font-semibold text-neutral-500">{label}</dt>
      <dd className="mt-1 text-2xl font-semibold text-neutral-950">{value}</dd>
    </div>
  );
}

function RegistrySection({
  emptyText,
  entries,
  icon,
  kind,
  title,
}: {
  emptyText: string;
  entries: Array<InverseIndexResponse["people"][number] | InverseIndexResponse["speakers"][number]>;
  icon: ReactNode;
  kind: "person" | "speaker";
  title: string;
}) {
  return (
    <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
      <div className="flex items-center gap-2">
        {icon}
        <h3 className="text-sm font-semibold text-neutral-950">{title}</h3>
      </div>

      {entries.length === 0 ? (
        <div className="mt-4 rounded-md border border-dashed border-neutral-300 bg-neutral-50 px-4 py-8 text-center text-sm text-neutral-500">
          {emptyText}
        </div>
      ) : (
        <div className="mt-4 grid gap-3">
          {entries.map((entry) => (
            <RegistryEntryCard entry={entry} key={`${kind}-${entry.id}`} kind={kind} />
          ))}
        </div>
      )}
    </section>
  );
}

function RegistryEntryCard({
  entry,
  kind,
}: {
  entry: InverseIndexResponse["people"][number] | InverseIndexResponse["speakers"][number];
  kind: "person" | "speaker";
}) {
  const isSpeaker = kind === "speaker";
  const primaryCount = isSpeaker
    ? `${(entry as InverseIndexResponse["speakers"][number]).segment_count} segment(s)`
    : `${(entry as InverseIndexResponse["people"][number]).face_count} face(s)`;

  return (
    <article className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0">
          <h4 className="truncate text-sm font-semibold text-neutral-950" title={entry.id}>
            {entry.label?.trim() || entry.id}
          </h4>
          <p className="mt-1 truncate text-xs text-neutral-600">{entry.id}</p>
        </div>
        <div className="flex shrink-0 flex-wrap gap-2">
          <span className="rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
            {entry.media_count} media
          </span>
          <span className="rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
            {primaryCount}
          </span>
          <span className="rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
            {formatPercent(entry.confidence)}
          </span>
        </div>
      </div>

      {isSpeaker ? (
        <p className="mt-2 text-xs text-neutral-600">
          {formatDurationSeconds((entry as InverseIndexResponse["speakers"][number]).total_seconds)}
        </p>
      ) : null}

      <div className="mt-3 grid max-h-96 gap-2 overflow-auto pr-1">
        {entry.locations.map((location) => (
          <RegistryLocationRow key={`${entry.id}-${location.media_id}`} location={location} />
        ))}
      </div>
    </article>
  );
}

function RegistryLocationRow({ location }: { location: InverseIndexLocation }) {
  const previewUrl = location.thumbnail_url;
  const openUrl = location.scene_clip_url ?? location.media_url;

  return (
    <div className="grid gap-3 rounded-md border border-neutral-200 bg-white p-2 sm:grid-cols-[64px_minmax(0,1fr)_auto]">
      <div className="grid aspect-square place-items-center overflow-hidden rounded bg-neutral-200">
        {previewUrl ? (
          <img alt="" className="h-full w-full object-cover" loading="lazy" src={previewUrl} />
        ) : (
          <ImageIcon className="size-6 text-neutral-500" aria-hidden="true" />
        )}
      </div>
      <div className="min-w-0">
        <h5 className="truncate text-sm font-semibold text-neutral-950" title={location.filename}>
          {location.filename}
        </h5>
        <p className="mt-1 truncate text-xs text-neutral-600" title={location.relative_path}>
          {location.relative_path}
        </p>
        <div className="mt-2 flex flex-wrap gap-2 text-xs">
          <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
            {mediaKindLabel(location.media_kind)}
          </span>
          <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
            {location.occurrence_count} hit(s)
          </span>
          {location.frame_indices.length ? (
            <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
              Frames {compactNumberList(location.frame_indices)}
            </span>
          ) : null}
          {location.start_seconds !== null && location.end_seconds !== null ? (
            <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
              {formatSeconds(location.start_seconds)}-{formatSeconds(location.end_seconds)}
            </span>
          ) : null}
          {location.page_number ? (
            <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
              Page {location.page_number}
            </span>
          ) : null}
        </div>
      </div>
      {openUrl ? (
        <a
          className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
          href={openUrl}
          rel="noreferrer"
          target="_blank"
        >
          <FileText className="size-4" aria-hidden="true" />
          <span>Open</span>
        </a>
      ) : null}
    </div>
  );
}

function sortPeopleEntries(people: InverseIndexResponse["people"]) {
  return [...people].sort(
    (left, right) =>
      right.media_count - left.media_count ||
      right.face_count - left.face_count ||
      registryName(left).localeCompare(registryName(right), undefined, {
        sensitivity: "base",
      }),
  );
}

function sortSpeakerEntries(speakers: InverseIndexResponse["speakers"]) {
  return [...speakers].sort(
    (left, right) =>
      right.media_count - left.media_count ||
      right.total_seconds - left.total_seconds ||
      registryName(left).localeCompare(registryName(right), undefined, {
        sensitivity: "base",
      }),
  );
}

function registryName(entry: { id: string; label: string | null }) {
  return entry.label?.trim() || entry.id;
}

function compactNumberList(values: number[]) {
  if (values.length <= 4) {
    return values.join(", ");
  }

  return `${values.slice(0, 4).join(", ")} +${values.length - 4}`;
}

function formatDurationSeconds(seconds: number) {
  if (seconds < 60) {
    return `${seconds.toFixed(1)}s total`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = Math.round(seconds % 60);
  return `${minutes}m ${remainingSeconds}s total`;
}

function formatSeconds(seconds: number) {
  return `${seconds.toFixed(1)}s`;
}

function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}
