import type { EditableSmartAlbum } from "../types";

export function DraftAlbumListItem({ draft }: { draft: EditableSmartAlbum }) {
  return (
    <div className="rounded-md border border-dashed border-emerald-500 bg-emerald-50 px-3 py-2 text-sm font-semibold text-emerald-950">
      {draft.name.trim() || "Unsaved album"}
    </div>
  );
}
