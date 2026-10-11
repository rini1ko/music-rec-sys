import { useState, type FormEvent } from "react";
import { api } from "../api/client";
import type { NewUser, UserProfile } from "../api/types";
import { messageOf, useAsync } from "../hooks/useAsync";
import { ErrorBox } from "./Feedback";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { Field } from "./ui/Field";
import { inputClasses } from "./ui/styles";

const MIN_AGE = 1;
const MAX_AGE = 120;
/** The countries in the hint list come from this many most active listeners. */
const COUNTRY_SAMPLE = 200;

interface Props {
  open: boolean;
  onClose: () => void;
  onCreated: (user: UserProfile) => void;
}

export function NewListenerDialog({ open, onClose, onCreated }: Props) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="New listener"
      description="A listener without plays gets a cold start: tracks popular in their country, or among all listeners."
    >
      <NewListenerForm onCancel={onClose} onCreated={onCreated} />
    </Dialog>
  );
}

function parseAge(text: string): { age?: number; error: string | null } {
  if (text.trim() === "") return { error: null };
  const age = Number(text);
  if (!Number.isInteger(age) || age < MIN_AGE || age > MAX_AGE) {
    return { error: `Age must be a whole number from ${MIN_AGE} to ${MAX_AGE}.` };
  }
  return { age, error: null };
}

function NewListenerForm({
  onCancel,
  onCreated,
}: {
  onCancel: () => void;
  onCreated: (user: UserProfile) => void;
}) {
  const [country, setCountry] = useState("");
  const [age, setAge] = useState("");
  const [gender, setGender] = useState("");
  const [touched, setTouched] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const countries = useAsync(
    async () => {
      const users = await api.users(COUNTRY_SAMPLE);
      const names = users.items.map((user) => user.country).filter((name): name is string => name !== null);
      return [...new Set(names)].sort((left, right) => left.localeCompare(right));
    },
    "countries",
  );

  const parsedAge = parseAge(age);
  const ageError = touched ? parsedAge.error : null;

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setTouched(true);
    if (parsedAge.error !== null) return;

    const body: NewUser = {
      country: country.trim() || undefined,
      age: parsedAge.age,
      gender: gender || undefined,
    };
    setSaving(true);
    setSaveError(null);
    try {
      onCreated(await api.createUser(body));
    } catch (error) {
      setSaveError(messageOf(error));
      setSaving(false);
    }
  }

  return (
    <form onSubmit={submit} noValidate className="space-y-4">
      <Field
        htmlFor="new-listener-country"
        label="Country"
        hint="Optional. Pick one from the data to get local tracks in the cold start."
      >
        <input
          id="new-listener-country"
          list="new-listener-countries"
          value={country}
          onChange={(event) => setCountry(event.target.value)}
          placeholder="For example, Germany"
          maxLength={56}
          autoComplete="off"
          data-autofocus
          aria-describedby="new-listener-country-hint"
          className={inputClasses}
        />
        <datalist id="new-listener-countries">
          {(countries.data ?? []).map((name) => (
            <option key={name} value={name} />
          ))}
        </datalist>
      </Field>

      <div className="grid gap-4 sm:grid-cols-2">
        <Field htmlFor="new-listener-age" label="Age" hint="Optional." error={ageError}>
          <input
            id="new-listener-age"
            type="number"
            inputMode="numeric"
            min={MIN_AGE}
            max={MAX_AGE}
            value={age}
            onChange={(event) => setAge(event.target.value)}
            onBlur={() => setTouched(true)}
            aria-invalid={ageError !== null}
            aria-describedby={ageError !== null ? "new-listener-age-error" : "new-listener-age-hint"}
            className={inputClasses}
          />
        </Field>

        <Field htmlFor="new-listener-gender" label="Gender" hint="Optional.">
          <select
            id="new-listener-gender"
            value={gender}
            onChange={(event) => setGender(event.target.value)}
            aria-describedby="new-listener-gender-hint"
            className={inputClasses}
          >
            <option value="">Not set</option>
            <option value="male">Male</option>
            <option value="female">Female</option>
          </select>
        </Field>
      </div>

      {saveError && <ErrorBox message={saveError} />}

      <div className="flex justify-end gap-2 pt-2">
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" busy={saving}>
          Create listener
        </Button>
      </div>
    </form>
  );
}
