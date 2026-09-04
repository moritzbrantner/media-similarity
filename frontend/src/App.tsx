import { Navigate, Route, Routes } from "react-router";
import { AppShell } from "./pages/_layout";
import { SearchRoute } from "./pages";
import { AlbumsRoute } from "./pages/albums";
import { RegistryRoute } from "./pages/registry";
import { SourcesRoute } from "./pages/sources";
import { WorkflowsRoute } from "./pages/workflows";
import { ShowcasePage } from "./showcase/ShowcasePage";

declare const __MEDIA_SIMILARITY_SHOWCASE__: boolean;

export function App() {
  if (__MEDIA_SIMILARITY_SHOWCASE__) {
    return <ShowcasePage />;
  }

  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<SearchRoute />} />
        <Route path="albums" element={<AlbumsRoute />} />
        <Route path="registry" element={<RegistryRoute />} />
        <Route path="sources" element={<SourcesRoute />} />
        <Route path="workflows" element={<WorkflowsRoute />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}
