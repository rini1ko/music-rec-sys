import { useParams } from "react-router";
import { parseIndex } from "../lib/paths";

/**
 * The listener from the `:userId` part of the URL.
 * `undefined` means no listener is picked, `null` means the URL holds something that is not an index.
 */
export function useUserParam(): number | null | undefined {
  const { userId } = useParams();
  return parseIndex(userId);
}
