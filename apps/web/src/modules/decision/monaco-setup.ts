// Monaco (the editor's code panels) from Studio's own bundle. By default it is
// downloaded from a public CDN, which a self-hosted installation must never do:
// a bank's network blocks it, or it should.
import { loader } from '@monaco-editor/react';
import * as monaco from 'monaco-editor';

self.MonacoEnvironment = {
  getWorker(_id: string, label: string) {
    switch (label) {
      case 'json':
        return new Worker(new URL('monaco-editor/esm/vs/language/json/json.worker.js', import.meta.url));
      case 'typescript':
      case 'javascript':
        return new Worker(new URL('monaco-editor/esm/vs/language/typescript/ts.worker.js', import.meta.url));
      default:
        return new Worker(new URL('monaco-editor/esm/vs/editor/editor.worker.js', import.meta.url));
    }
  },
};

loader.config({ monaco });
