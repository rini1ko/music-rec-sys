import { useState } from "react";
import { RecommendationList } from "./components/RecommendationList";
import { StatusPill } from "./components/StatusPill";
import { UserPicker } from "./components/UserPicker";

export default function App() {
  const [userId, setUserId] = useState<number | null>(null);

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100">
      <header className="border-b border-slate-800">
        <div className="mx-auto flex max-w-5xl items-center justify-between px-6 py-4">
          <h1 className="text-lg font-semibold tracking-tight">Music recommender</h1>
          <StatusPill />
        </div>
      </header>

      <main className="mx-auto grid max-w-5xl gap-8 px-6 py-8 md:grid-cols-[18rem_1fr]">
        <UserPicker selected={userId} onSelect={setUserId} />
        <RecommendationList userId={userId} />
      </main>
    </div>
  );
}