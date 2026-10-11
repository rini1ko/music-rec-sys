import { useState } from "react";
import { ModelPanel } from "./components/ModelPanel";
import { PlayerPanel } from "./components/PlayerPanel";
import { RecommendationList } from "./components/RecommendationList";
import { StatusPill } from "./components/StatusPill";
import { UserPicker } from "./components/UserPicker";

type Tab = "recommend" | "player" | "models";

const TABS: { id: Tab; label: string }[] = [
  { id: "recommend", label: "Recommendations" },
  { id: "player", label: "Player" },
  { id: "models", label: "Models" },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("recommend");
  const [userId, setUserId] = useState<number | null>(null);
  const [refresh, setRefresh] = useState(0);

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100">
      <header className="border-b border-slate-800">
        <div className="mx-auto flex max-w-5xl flex-wrap items-center justify-between gap-4 px-6 py-4">
          <h1 className="text-lg font-semibold tracking-tight">Music recommender</h1>
          <nav className="flex gap-1" aria-label="Sections">
            {TABS.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setTab(item.id)}
                aria-current={tab === item.id ? "page" : undefined}
                className={`rounded-md px-3 py-1.5 text-sm ${
                  tab === item.id
                    ? "bg-slate-800 text-slate-100"
                    : "text-slate-400 hover:text-slate-200"
                }`}
              >
                {item.label}
              </button>
            ))}
          </nav>
          <StatusPill />
        </div>
      </header>

      <main className="mx-auto max-w-5xl px-6 py-8">
        {tab === "models" ? (
          <ModelPanel />
        ) : (
          <div className="grid gap-8 md:grid-cols-[18rem_1fr]">
            <UserPicker selected={userId} onSelect={setUserId} />
            <div className="space-y-8">
              {tab === "player" && (
                <PlayerPanel
                  key={userId ?? "none"}
                  userId={userId}
                  onListened={() => setRefresh((value) => value + 1)}
                />
              )}
              <RecommendationList userId={userId} refresh={refresh} />
            </div>
          </div>
        )}
      </main>
    </div>
  );
}