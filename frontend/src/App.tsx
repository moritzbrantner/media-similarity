import { Navigate, Route, Routes } from "react-router";
import { AlbumsRoute } from "./app/albums-route";
import { AppShell } from "./app/app-shell";
import { RegistryRoute } from "./app/registry-route";
import { SearchRoute } from "./app/search-route";
import { SourcesRoute } from "./app/sources-route";
import { WorkflowsRoute } from "./app/workflows-route";

export function App() {
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
