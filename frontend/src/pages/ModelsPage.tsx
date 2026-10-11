import { ModelPanel } from "../components/ModelPanel";
import { useDocumentTitle } from "../hooks/useDocumentTitle";

export function ModelsPage() {
  useDocumentTitle("Models");
  return <ModelPanel />;
}
