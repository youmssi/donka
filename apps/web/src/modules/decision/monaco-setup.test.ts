import { loader } from '@monaco-editor/react';
import * as monaco from 'monaco-editor';

vi.mock('@monaco-editor/react', () => ({ loader: { config: vi.fn() } }));
vi.mock('monaco-editor', () => ({ editor: {} }));

beforeEach(async () => {
  vi.resetModules();
  await import('./monaco-setup');
});

it('configures the local editor bundle instead of a public CDN', () => {
  expect(loader.config).toHaveBeenCalledWith({ monaco });
});

it.each([
  ['json', 'language/json/json.worker.js'],
  ['typescript', 'language/typescript/ts.worker.js'],
  ['javascript', 'language/typescript/ts.worker.js'],
  ['plaintext', 'editor/editor.worker.js'],
])('uses a bundled worker for %s', (label, suffix) => {
  const WorkerMock = vi.fn(function Worker(url: URL) {
    return { url };
  });
  vi.stubGlobal('Worker', WorkerMock);
  try {
    const environment = self.MonacoEnvironment;
    if (!environment?.getWorker) throw new Error('Local worker factory was not installed');
    environment.getWorker('', label);
    const url: unknown = WorkerMock.mock.calls[0]?.[0];
    expect(url).toBeInstanceOf(URL);
    expect(String(url)).toContain(`monaco-editor/esm/vs/${suffix}`);
    expect(new URL(String(url)).origin).toBe(window.location.origin);
  } finally {
    vi.unstubAllGlobals();
  }
});
