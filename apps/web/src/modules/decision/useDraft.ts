'use client';

import { useCallback, useEffect, useRef, useState } from 'react';

import type { ActionError } from '@/components/shared/api';

import { getDecision, saveDecision } from './decision.service';
import { conflictOf, type Conflict, type Decision } from './schema';

/** Saves wait for a pause in editing, so a burst of changes is one save. */
export const AUTOSAVE_DELAY_MS = 1200;

export type DraftStatus =
  | { kind: 'saved'; at: string }
  | { kind: 'dirty' }
  | { kind: 'saving' }
  | { kind: 'conflict'; conflict: Conflict | null }
  | { kind: 'error'; error: ActionError }
  | { kind: 'readOnly' };

/**
 * The draft being edited, saved automatically after each pause. One save runs at
 * a time and names the revision it started from: when someone else saved in
 * between, autosave stops and the person chooses (keep theirs or keep mine);
 * nothing is overwritten silently.
 */
export function useDraft(projectId: string, decision: Decision, editable: boolean, onSaved: () => void) {
  const [graph, setGraph] = useState<unknown>(decision.content);
  const [status, setStatus] = useState<DraftStatus>(
    editable ? { kind: 'saved', at: decision.updatedAt } : { kind: 'readOnly' },
  );
  const revision = useRef(decision.revision);
  const saved = useRef(JSON.stringify(decision.content));
  // The newest content not yet sent, and whether a save is on its way.
  const pending = useRef<unknown>(null);
  const inFlight = useRef(false);
  const blocked = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  // The caller's callback may change on every render; saves use the latest one.
  const savedCallback = useRef(onSaved);
  useEffect(() => {
    savedCallback.current = onSaved;
  }, [onSaved]);

  const flush = useCallback(async () => {
    clearTimeout(timer.current);
    if (inFlight.current || blocked.current) return;
    inFlight.current = true;
    // Changes made while a save runs are sent right after it, one save at a time.
    while (pending.current !== null) {
      const content = pending.current;
      pending.current = null;
      setStatus({ kind: 'saving' });
      const result = await saveDecision(projectId, decision.id, content, revision.current);
      if (result.ok) {
        revision.current = result.data.revision;
        saved.current = JSON.stringify(content);
        savedCallback.current();
        if (pending.current === null) setStatus({ kind: 'saved', at: result.data.updatedAt });
        continue;
      }
      // Keep what was not saved; a newer change wins over the one that failed.
      pending.current ??= content;
      if (result.error.code === 'DECISION_CONFLICT') {
        blocked.current = true;
        setStatus({ kind: 'conflict', conflict: conflictOf(result.error.details) });
      } else {
        setStatus({ kind: 'error', error: result.error });
      }
      break;
    }
    inFlight.current = false;
  }, [projectId, decision.id]);

  const change = useCallback(
    (next: unknown) => {
      setGraph(next);
      if (!editable) return;
      // The editor reports its first render too; only real changes are saved.
      if (pending.current === null && !inFlight.current && JSON.stringify(next) === saved.current) return;
      pending.current = next;
      if (blocked.current) return;
      setStatus({ kind: 'dirty' });
      clearTimeout(timer.current);
      timer.current = setTimeout(() => void flush(), AUTOSAVE_DELAY_MS);
    },
    [editable, flush],
  );

  /** Saves mine over theirs, knowingly. */
  const keepMine = useCallback(() => {
    if (status.kind !== 'conflict' || !status.conflict) return;
    revision.current = status.conflict.revision;
    blocked.current = false;
    void flush();
  }, [status, flush]);

  /** Drops my unsaved changes and loads what was saved. */
  const loadTheirs = useCallback(async () => {
    const latest = await getDecision(projectId, decision.id);
    if (!latest.ok) {
      setStatus({ kind: 'error', error: latest.error });
      return;
    }
    pending.current = null;
    blocked.current = false;
    revision.current = latest.data.revision;
    saved.current = JSON.stringify(latest.data.content);
    setGraph(latest.data.content);
    setStatus({ kind: 'saved', at: latest.data.updatedAt });
  }, [projectId, decision.id]);

  const retry = useCallback(() => void flush(), [flush]);

  // Unsaved work: warn before the tab closes, and save when leaving the page in Studio
  // (on unmount only, never on a re-render).
  const latestFlush = useRef(flush);
  useEffect(() => {
    latestFlush.current = flush;
  }, [flush]);
  useEffect(() => {
    const warn = (event: BeforeUnloadEvent) => {
      if (pending.current !== null || inFlight.current) event.preventDefault();
    };
    window.addEventListener('beforeunload', warn);
    return () => {
      window.removeEventListener('beforeunload', warn);
      void latestFlush.current();
    };
  }, []);

  return { graph, status, change, keepMine, loadTheirs, retry };
}
