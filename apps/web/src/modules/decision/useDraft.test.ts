import { act, renderHook } from '@testing-library/react';

import { getDecision, saveDecision } from './decision.service';
import type { Decision } from './schema';
import { AUTOSAVE_DELAY_MS, canonical, useDraft } from './useDraft';

vi.mock('./decision.service', () => ({ saveDecision: vi.fn(), getDecision: vi.fn() }));
const saveMock = vi.mocked(saveDecision);
const getMock = vi.mocked(getDecision);

const decision: Decision = {
  id: 'd-1',
  key: 'person-score',
  revision: 3,
  updatedAt: '2026-09-29T09:00:00Z',
  updatedBy: { id: 'u-1', email: 'ada@bank.example' },
  latestVersion: 1,
  changedSinceVersion: false,
  content: { nodes: [], edges: [] },
};
const graph = (n: number) => ({ nodes: Array.from({ length: n }, (_, i) => ({ id: `n${i}` })), edges: [] });
const saved = (revision: number) =>
  ({
    ok: true,
    data: { ...decision, revision, updatedAt: '2026-09-29T09:05:00Z' },
  }) as const;

beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
});
afterEach(() => vi.useRealTimers());

async function pause() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(AUTOSAVE_DELAY_MS);
  });
}

it('saves after a pause in editing, once for a burst of changes', async () => {
  saveMock.mockResolvedValue(saved(4));
  const onSaved = vi.fn();
  const { result } = renderHook(() => useDraft('p-1', decision, true, onSaved));
  act(() => result.current.change(graph(1)));
  act(() => result.current.change(graph(2)));
  expect(result.current.status.kind).toBe('dirty');
  await pause();
  expect(saveMock).toHaveBeenCalledTimes(1);
  expect(saveMock).toHaveBeenCalledWith('p-1', 'd-1', graph(2), 3);
  expect(result.current.status).toEqual({ kind: 'saved', at: '2026-09-29T09:05:00Z' });
  expect(onSaved).toHaveBeenCalled();

  // The next save starts from the new revision.
  act(() => result.current.change(graph(3)));
  await pause();
  expect(saveMock).toHaveBeenLastCalledWith('p-1', 'd-1', graph(3), 4);
});

it('does not save what did not change', async () => {
  const { result } = renderHook(() => useDraft('p-1', decision, true, vi.fn()));
  act(() => result.current.change({ nodes: [], edges: [] }));
  await pause();
  expect(saveMock).not.toHaveBeenCalled();
  expect(result.current.status.kind).toBe('saved');
});

it('stops on a conflict and lets the person choose', async () => {
  saveMock.mockResolvedValueOnce({
    ok: false,
    error: {
      code: 'DECISION_CONFLICT',
      details: {
        revision: 5,
        updatedAt: '2026-09-29T09:04:00Z',
        updatedBy: { id: 'u-2', email: 'grace@bank.example' },
      },
    },
  });
  const { result } = renderHook(() => useDraft('p-1', decision, true, vi.fn()));
  act(() => result.current.change(graph(1)));
  await pause();
  expect(result.current.status).toMatchObject({ kind: 'conflict', conflict: { revision: 5 } });

  // Further edits wait for the choice.
  act(() => result.current.change(graph(2)));
  await pause();
  expect(saveMock).toHaveBeenCalledTimes(1);

  // Keep mine: saved over their revision, with the latest edit.
  saveMock.mockResolvedValueOnce(saved(6));
  await act(async () => {
    result.current.keepMine();
    await vi.runAllTimersAsync();
  });
  expect(saveMock).toHaveBeenLastCalledWith('p-1', 'd-1', graph(2), 5);
  expect(result.current.status.kind).toBe('saved');
});

it('loads their version and drops unsaved changes', async () => {
  saveMock.mockResolvedValueOnce({
    ok: false,
    error: { code: 'DECISION_CONFLICT', details: { revision: 5, updatedAt: '2026-09-29T09:04:00Z', updatedBy: null } },
  });
  getMock.mockResolvedValue({ ok: true, data: { ...decision, revision: 5, content: graph(7) } });
  const { result } = renderHook(() => useDraft('p-1', decision, true, vi.fn()));
  act(() => result.current.change(graph(1)));
  await pause();
  await act(async () => {
    await result.current.loadTheirs();
  });
  expect(result.current.graph).toEqual(graph(7));
  expect(result.current.status.kind).toBe('saved');
  act(() => result.current.change(graph(8)));
  saveMock.mockResolvedValueOnce(saved(6));
  await pause();
  expect(saveMock).toHaveBeenLastCalledWith('p-1', 'd-1', graph(8), 5);
});

it('keeps a failed save to retry, and never saves for a viewer', async () => {
  saveMock.mockResolvedValueOnce({ ok: false, error: { code: 'NETWORK' } });
  const { result } = renderHook(() => useDraft('p-1', decision, true, vi.fn()));
  act(() => result.current.change(graph(1)));
  await pause();
  expect(result.current.status).toEqual({ kind: 'error', error: { code: 'NETWORK' } });
  saveMock.mockResolvedValueOnce(saved(4));
  await act(async () => {
    result.current.retry();
    await vi.runAllTimersAsync();
  });
  expect(saveMock).toHaveBeenLastCalledWith('p-1', 'd-1', graph(1), 3);

  saveMock.mockClear();
  const viewer = renderHook(() => useDraft('p-1', decision, false, vi.fn()));
  expect(viewer.result.current.status.kind).toBe('readOnly');
  act(() => viewer.result.current.change(graph(1)));
  await pause();
  expect(saveMock).not.toHaveBeenCalled();
});

it('saves what is pending when the page is left, and only then', async () => {
  saveMock.mockResolvedValue(saved(4));
  const { result, rerender, unmount } = renderHook(({ cb }) => useDraft('p-1', decision, true, cb), {
    initialProps: { cb: vi.fn() },
  });
  act(() => result.current.change(graph(1)));
  // A re-render with a new callback is not leaving the page.
  rerender({ cb: vi.fn() });
  expect(saveMock).not.toHaveBeenCalled();
  unmount();
  await act(async () => {
    await vi.runAllTimersAsync();
  });
  expect(saveMock).toHaveBeenCalledWith('p-1', 'd-1', graph(1), 3);
});

it('settles: sends the pending change at once, so a version is what is on screen', async () => {
  saveMock.mockResolvedValue({ ok: true, data: { ...saved(4).data, changedSinceVersion: true } });
  const { result } = renderHook(() => useDraft('p-1', decision, true, vi.fn()));
  expect(result.current.version).toEqual({ latest: 1, changed: false });
  act(() => result.current.change(graph(1)));
  let settled = false;
  await act(async () => {
    settled = await result.current.settle();
  });
  expect(settled).toBe(true);
  expect(saveMock).toHaveBeenCalledWith('p-1', 'd-1', graph(1), 3);
  expect(result.current.currentRevision()).toBe(4);
  expect(result.current.version).toEqual({ latest: 1, changed: true });

  act(() => result.current.versionSaved(2));
  expect(result.current.version).toEqual({ latest: 2, changed: false });
});

it('does not settle while a conflict waits for a choice', async () => {
  saveMock.mockResolvedValueOnce({
    ok: false,
    error: { code: 'DECISION_CONFLICT', details: { revision: 5, updatedAt: '2026-09-29T09:04:00Z', updatedBy: null } },
  });
  const { result } = renderHook(() => useDraft('p-1', decision, true, vi.fn()));
  act(() => result.current.change(graph(1)));
  let settled = true;
  await act(async () => {
    settled = await result.current.settle();
  });
  expect(settled).toBe(false);
});

it('replaces the draft after a restore and drops what was pending', async () => {
  const { result } = renderHook(() => useDraft('p-1', decision, true, vi.fn()));
  act(() => result.current.change(graph(1)));
  act(() =>
    result.current.replace({
      ...decision,
      revision: 9,
      latestVersion: 4,
      changedSinceVersion: false,
      updatedAt: '2026-09-29T10:00:00Z',
      content: graph(5),
    }),
  );
  await pause();
  expect(saveMock).not.toHaveBeenCalled();
  expect(result.current.graph).toEqual(graph(5));
  expect(result.current.version).toEqual({ latest: 4, changed: false });
  expect(result.current.status).toEqual({ kind: 'saved', at: '2026-09-29T10:00:00Z' });
  expect(result.current.currentRevision()).toBe(9);
});

it('does not save a graph that only differs in key order', async () => {
  const { result } = renderHook(() =>
    useDraft('p-1', { ...decision, content: { nodes: [{ id: 'a', name: 'x' }], edges: [] } }, true, vi.fn()),
  );
  act(() => result.current.change({ edges: [], nodes: [{ name: 'x', id: 'a' }] }));
  await pause();
  expect(saveMock).not.toHaveBeenCalled();
  expect(canonical({ b: 1, a: [{ d: 2, c: 3 }] })).toBe('{"a":[{"c":3,"d":2}],"b":1}');
});
