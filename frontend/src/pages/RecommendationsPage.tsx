import { NoListener } from "../components/NoListener";
import { RecommendationList } from "../components/RecommendationList";
import { useDocumentTitle } from "../hooks/useDocumentTitle";
import { useUserParam } from "../hooks/useUserParam";

export function RecommendationsPage() {
  const userId = useUserParam();
  useDocumentTitle(typeof userId === "number" ? `Recommendations for user ${userId}` : "Recommendations");

  if (typeof userId !== "number") {
    return <NoListener invalid={userId === null} purpose="see what they should hear next" />;
  }
  return <RecommendationList key={userId} userId={userId} />;
}
