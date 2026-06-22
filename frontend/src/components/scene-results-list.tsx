import { Button } from "@moritzbrantner/ui";
import { FileAudio, FileText, FileVideo } from "lucide-react";
import { filterResults } from "../search/filtering";
import { sortResults } from "../search/sorting";
import type { MetadataFilters, ResultSortMode } from "../search/types";
import type { SearchSceneResponse } from "../types";
import { ResultsGrid } from "./results-grid";
import { formatSeconds } from "./results/result-formatting";

export function SceneResultsList({
  deletingId,
  filters,
  onDelete,
  onUpdateTags,
  onSelectScene,
  resultLimit,
  scenes,
  selectedSceneIndex,
  sortMode,
  tagSavingId,
}: {
  deletingId?: string;
  filters: MetadataFilters;
  onDelete?: (id: string) => void;
  onUpdateTags?: (id: string, tags: string[]) => void;
  onSelectScene: (sceneIndex: number) => void;
  resultLimit: number;
  scenes: SearchSceneResponse[];
  selectedSceneIndex: number | null;
  sortMode: ResultSortMode;
  tagSavingId?: string;
}) {
  const selectedScene =
    scenes.find((scene) => scene.scene_index === selectedSceneIndex) ?? scenes[0];
  const selectedResults = selectedScene
    ? sortResults(filterResults(selectedScene.results, filters), sortMode).slice(0, resultLimit)
    : [];
  const isAudioBits = scenes.some((scene) => scene.scene_kind === "audio_bit");
  const isPdfPages = scenes.some((scene) => scene.scene_kind === "pdf_page");
  const segmentLabel = isPdfPages ? "Page" : isAudioBits ? "Bit" : "Scene";
  const SegmentIcon = isPdfPages ? FileText : isAudioBits ? FileAudio : FileVideo;

  return (
    <div className="flex flex-col gap-5">
      <div className="rounded-lg border border-neutral-300 bg-white p-3 shadow-sm">
        <div className="mb-2 flex items-center gap-2 text-sm font-semibold text-neutral-950">
          <SegmentIcon className="size-4 text-neutral-600" aria-hidden="true" />
          <span>Query segment</span>
        </div>
        <div className="flex gap-2 overflow-x-auto pb-1">
          {scenes.map((scene) => (
            <Button
              aria-pressed={scene.scene_index === selectedScene?.scene_index}
              variant={scene.scene_index === selectedScene?.scene_index ? "default" : "outline"}
              className={`inline-flex h-10 shrink-0 items-center justify-center rounded-md border px-3 text-sm font-semibold transition ${
                scene.scene_index === selectedScene?.scene_index
                  ? "border-emerald-700 bg-emerald-50 text-emerald-950"
                  : "border-neutral-300 bg-white text-neutral-800 hover:border-neutral-500 hover:bg-neutral-50"
              }`}
              key={scene.scene_index}
              onClick={() => onSelectScene(scene.scene_index)}
              type="button"
            >
              {scene.page_label ?? `${segmentLabel} ${scene.scene_index + 1}`}
              {!isPdfPages
                ? ` · ${formatSeconds(scene.start_seconds)}-${formatSeconds(scene.end_seconds)}`
                : ""}
              {scene.speaker_label ? ` · ${scene.speaker_label}` : ""}
            </Button>
          ))}
        </div>
      </div>

      {selectedScene ? (
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <div className="mb-3 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <h3 className="text-sm font-semibold text-neutral-950">
                {selectedScene.page_label ?? `${segmentLabel} ${selectedScene.scene_index + 1}`}
              </h3>
              <p className="text-xs text-neutral-600">
                {isPdfPages
                  ? selectedScene.page_number
                    ? `Page ${selectedScene.page_number}`
                    : "PDF page"
                  : `${formatSeconds(selectedScene.start_seconds)}-${formatSeconds(
                      selectedScene.end_seconds,
                    )}`}
                {!isPdfPages && isAudioBits
                  ? selectedScene.speaker_label
                    ? ` · ${selectedScene.speaker_label}`
                    : ""
                  : !isPdfPages
                    ? ` · frames ${selectedScene.start_frame}-${selectedScene.end_frame}`
                    : ""}
              </p>
            </div>
            {selectedScene.clip_url ? (
              <a
                className="inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
                href={selectedScene.clip_url}
                rel="noreferrer"
                target="_blank"
              >
                <FileVideo className="size-4" aria-hidden="true" />
                <span>Open query clip</span>
              </a>
            ) : null}
          </div>
          <ResultsGrid
            deletingId={deletingId}
            onDelete={onDelete}
            onUpdateTags={onUpdateTags}
            pending={false}
            results={selectedResults}
            searched
            tagSavingId={tagSavingId}
          />
        </section>
      ) : null}
    </div>
  );
}
