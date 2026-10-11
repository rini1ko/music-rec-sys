import { EmptyState } from "./ui/EmptyState";

interface Props {
  /** True when the URL holds something that is not a listener index. */
  invalid: boolean;
  /** What the page shows once a listener is picked. */
  purpose: string;
}

export function NoListener({ invalid, purpose }: Props) {
  return invalid ? (
    <EmptyState
      title="This is not a listener number"
      description="The address should end with a listener index, e.g. /recommendations/12. Pick a listener from the list instead."
    />
  ) : (
    <EmptyState title="Pick a listener" description={`Choose someone from the list to ${purpose}.`} />
  );
}
