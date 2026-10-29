import { useEffect, useState } from "react";

import type { UnlistenFn } from "@tauri-apps/api/event";

/**
 * A list kept in sync with a Rust runtime: loads it once, then follows the
 * runtime's change events. `load` and `subscribe` must be stable references.
 */
export function useLiveList<T>(
  load: () => Promise<T[]>,
  subscribe: (handler: (items: T[]) => void) => Promise<UnlistenFn>,
) {
  const [items, setItems] = useState<T[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const unlisten = subscribe((next) => {
      if (active) setItems(next);
    });
    load()
      .then((initial) => {
        if (active) setItems((current) => current ?? initial);
      })
      .catch((err: unknown) => {
        if (active) setError(String(err));
      });
    return () => {
      active = false;
      void unlisten.then((stop) => stop());
    };
  }, [load, subscribe]);

  return { items, error };
}
