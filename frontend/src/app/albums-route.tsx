import { SmartAlbumsPage } from "../features/albums/pages/smart-albums-page";
import { useAppShellContext } from "./app-shell";

export function AlbumsRoute() {
  const shell = useAppShellContext();

  return (
    <SmartAlbumsPage initialDraft={shell.albumDraft} onDraftConsumed={shell.consumeAlbumDraft} />
  );
}
