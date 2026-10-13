import { NoListener } from "../components/NoListener";
import { RecommendationList } from "../components/RecommendationList";
import { useUserParam } from "../hooks/useUserParam";

export function RecommendationsPage() {
  const userId = useUserParam();

  if (typeof userId !== "number") {
    return <NoListener invalid={userId === null} purpose="see what they should hear next" />;
  }
  return <RecommendationList key={userId} userId={userId} />;
}
