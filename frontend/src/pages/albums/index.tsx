import { useAppShellContext } from "../_layout";
import { SmartAlbumsPage } from "./smart-albums-page";

export function AlbumsRoute() {
  const shell = useAppShellContext();

  return (
    <SmartAlbumsPage initialDraft={shell.albumDraft} onDraftConsumed={shell.consumeAlbumDraft} />
  );
}
