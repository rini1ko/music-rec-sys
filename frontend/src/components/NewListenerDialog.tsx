import { useEffect, useRef, useState, type FormEvent } from "react";
import { api } from "../api/client";
import type { UserProfile } from "../api/types";
import { messageOf, useAsync } from "../hooks/useAsync";
import { buttonClasses, inputClasses, labelClasses } from "../lib/styles";
import { ErrorBox } from "./Feedback";

interface Props {
  onClose: () => void;
  onCreated: (user: UserProfile) => void;
}

/**
 * Creates a listener without plays (C4). A native modal <dialog>: the browser
 * keeps focus inside and reports Escape. Render it only while it is open, so
 * the form starts empty every time.
 */
export function NewListenerDialog({ onClose, onCreated }: Props) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const countryRef = useRef<HTMLInputElement>(null);
  const [country, setCountry] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // cold start matches the country by exact name, so suggest the names from the data
  const countries = useAsync(async () => {
    const names = (await api.users(200)).items.map((user) => user.country);
    return [...new Set(names.filter((name): name is string => name !== null))].sort();
  }, "countries");

  useEffect(() => {
    const dialog = dialogRef.current;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog?.showModal();
    countryRef.current?.focus();
    return () => {
      dialog?.close();
      // React removes the dialog before this runs, so the browser cannot give focus back itself
      opener?.focus();
    };
  }, []);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      onCreated(await api.createUser(country.trim() || undefined));
    } catch (failure) {
      setError(messageOf(failure));
      setSaving(false);
    }
  }

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="new-listener-title"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      className="m-auto w-[calc(100%-2rem)] max-w-md rounded-xl border border-line bg-surface p-6 text-fg"
    >
      <h2 id="new-listener-title" className="text-lg font-semibold">
        New listener
      </h2>
      <p className="mt-1 text-sm text-fg-muted">
        Without plays they get a cold start: tracks popular in their country, or among all listeners.
      </p>
      <form onSubmit={submit} className="mt-5 space-y-4">
        <label className="block space-y-1.5">
          <span className={labelClasses}>Country (optional)</span>
          <input
            ref={countryRef}
            list="new-listener-countries"
            value={country}
            onChange={(event) => setCountry(event.target.value)}
            maxLength={56}
            autoComplete="off"
            placeholder="For example, Germany"
            className={inputClasses}
          />
          <datalist id="new-listener-countries">
            {(countries.data ?? []).map((name) => (
              <option key={name} value={name} />
            ))}
          </datalist>
        </label>
        {error && <ErrorBox message={error} />}
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className={buttonClasses.secondary}>
            Cancel
          </button>
          <button type="submit" disabled={saving} className={buttonClasses.primary}>
            {saving ? "Creating..." : "Create listener"}
          </button>
        </div>
      </form>
    </dialog>
  );
}
