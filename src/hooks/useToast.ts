import { useCallback, useEffect, useRef, useState } from "react";

export function useToast() {
  const [toast, setToast] = useState<{ id: number; text: string } | null>(null);
  const ids = useRef(Date.now());
  const notify = useCallback((text: string) => setToast({ id: ++ids.current, text }), []);
  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(null), 3000);
    return () => window.clearTimeout(timer);
  }, [toast]);

  return { toast, notify };
}
