import { Navigate, Route, Routes } from "react-router";
import { paths } from "./lib/paths";
import { AppShell } from "./pages/AppShell";
import { ArtistPage } from "./pages/ArtistPage";
import { ArtistsPage } from "./pages/ArtistsPage";
import { DashboardPage } from "./pages/DashboardPage";
import { HistoryPage } from "./pages/HistoryPage";
import { ListenerLayout } from "./pages/ListenerLayout";
import { ModelsPage } from "./pages/ModelsPage";
import { NotFoundPage } from "./pages/NotFoundPage";
import { PlayerPage } from "./pages/PlayerPage";
import { RadioPage } from "./pages/RadioPage";
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
          <Route path="radio/:userId?" element={<RadioPage />} />
          <Route path="history/:userId?" element={<HistoryPage />} />
          <Route path="sandbox/:userId?" element={<SandboxPage />} />
        </Route>
        <Route path="dashboard" element={<DashboardPage />} />
        <Route path="artists" element={<ArtistsPage />} />
        <Route path="artists/:artistId" element={<ArtistPage />} />
        <Route path="tracks/:trackId" element={<SimilarPage />} />
        <Route path="models" element={<ModelsPage />} />
        <Route path="*" element={<NotFoundPage />} />
      </Route>
    </Routes>
  );
}
