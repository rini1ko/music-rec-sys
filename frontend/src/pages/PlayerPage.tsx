import { useState } from "react";
import { NoListener } from "../components/NoListener";
import { PlayerPanel } from "../components/PlayerPanel";
import { RecommendationList } from "../components/RecommendationList";
import { useDocumentTitle } from "../hooks/useDocumentTitle";
import { useUserParam } from "../hooks/useUserParam";

export function PlayerPage() {
  const userId = useUserParam();
  useDocumentTitle(typeof userId === "number" ? `Player for user ${userId}` : "Player");

  if (typeof userId !== "number") {
    return <NoListener invalid={userId === null} purpose="play tracks for them" />;
  }
  // a new key resets the plays counter when another listener is picked
  return <Player key={userId} userId={userId} />;
}

function Player({ userId }: { userId: number }) {
  const [plays, setPlays] = useState(0);

  return (
    <div className="space-y-8">
      <PlayerPanel userId={userId} onListened={() => setPlays((value) => value + 1)} />
      <RecommendationList userId={userId} refresh={plays} linkToPlayer={false} />
    </div>
  );
}
