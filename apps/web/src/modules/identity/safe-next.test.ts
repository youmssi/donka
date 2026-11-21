import { safeNext } from './safe-next';

describe('safeNext', () => {
  it('keeps paths inside Studio', () => {
    expect(safeNext('/projects?id=42')).toBe('/projects?id=42');
  });

  it.each([null, '', 'projects', '//evil.example', 'https://evil.example', '/\\evil.example'])(
    'sends %s to the home page',
    (next) => {
      expect(safeNext(next)).toBe('/');
    },
  );
});
