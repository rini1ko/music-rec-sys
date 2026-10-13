import { Empty } from "./Feedback";

interface Props {
  /** True when the URL holds something that is not a listener index. */
  invalid: boolean;
  /** What the page shows once a listener is picked. */
  purpose: string;
}

export function NoListener({ invalid, purpose }: Props) {
  return invalid ? (
    <Empty title="This is not a listener number">
      The address should end with a listener index, e.g. /recommendations/12.
    </Empty>
  ) : (
    <Empty title="Pick a listener">Choose someone from the list to {purpose}.</Empty>
  );
}
