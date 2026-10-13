import { Navigate, Route, Routes } from "react-router";
import { ModelPanel } from "./components/ModelPanel";
import { paths } from "./lib/paths";
import { AppShell } from "./pages/AppShell";
import { DashboardPage } from "./pages/DashboardPage";
import { ListenerLayout } from "./pages/ListenerLayout";
import { NotFoundPage } from "./pages/NotFoundPage";
import { PlayerPage } from "./pages/PlayerPage";
import { RecommendationsPage } from "./pages/RecommendationsPage";
import { SandboxPage } from "./pages/SandboxPage";
import { SimilarPage } from "./pages/SimilarPage";

export default function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<Navigate to={paths.listener("recommendations", null)} replace />} />
        <Route element={<ListenerLayout />}>
          <Route path="recommendations/:userId?" element={<RecommendationsPage />} />
          <Route path="player/:userId?" element={<PlayerPage />} />
          <Route path="sandbox/:userId?" element={<SandboxPage />} />
        </Route>
        <Route path="dashboard" element={<DashboardPage />} />
        <Route path="tracks/:trackId" element={<SimilarPage />} />
        <Route path="models" element={<ModelPanel />} />
        <Route path="*" element={<NotFoundPage />} />
      </Route>
    </Routes>
  );
}
