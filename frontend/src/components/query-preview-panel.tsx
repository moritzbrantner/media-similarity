import { FileAudio, FileText, ImageIcon } from "lucide-react";

type QueryPreviewPanelProps = {
  previewIsAudio: boolean;
  previewIsPdf: boolean;
  previewIsVideo: boolean;
  previewUrl: string | null;
};

export function QueryPreviewPanel({
  previewIsAudio,
  previewIsPdf,
  previewIsVideo,
  previewUrl,
}: QueryPreviewPanelProps) {
  return (
    <section className="grid min-h-72 overflow-hidden rounded-lg border border-neutral-300 bg-white shadow-sm">
      {previewUrl ? (
        previewIsVideo ? (
          <video
            className="h-full max-h-[420px] w-full bg-black object-contain"
            controls
            src={previewUrl}
          />
        ) : previewIsAudio ? (
          <div className="flex h-full min-h-72 flex-col items-center justify-center gap-4 bg-neutral-50 p-8">
            <FileAudio className="size-12 text-neutral-500" aria-hidden="true" />
            <audio className="w-full max-w-xl" controls src={previewUrl} />
          </div>
        ) : (
          <img
            alt="Query preview"
            className="h-full max-h-[420px] w-full object-contain"
            src={previewUrl}
          />
        )
      ) : (
        <div className="flex flex-col items-center justify-center gap-3 bg-neutral-50 p-8 text-center text-neutral-500">
          {previewIsPdf ? (
            <FileText className="size-12" aria-hidden="true" />
          ) : (
            <ImageIcon className="size-12" aria-hidden="true" />
          )}
          <span className="text-sm font-medium">
            {previewIsPdf ? "PDF query selected" : "No query media selected"}
          </span>
        </div>
      )}
    </section>
  );
}
