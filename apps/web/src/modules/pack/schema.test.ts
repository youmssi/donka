import { fileName, localized, releaseOf } from './schema';

it('reads a pack in the reader’s language, English when it has no French', () => {
  expect(localized({ en: 'Retail credit', fr: 'Crédit' }, 'fr')).toBe('Crédit');
  expect(localized({ en: 'Retail credit', fr: '' }, 'fr')).toBe('Retail credit');
});

it('names the download as the server does', () => {
  expect(fileName('attachment; filename="retail.donka-pack.zip"', 'x.zip')).toBe('retail.donka-pack.zip');
  expect(fileName(null, 'x.zip')).toBe('x.zip');
  expect(releaseOf('drafts')).toBeUndefined();
  expect(releaseOf('r-1')).toBe('r-1');
});
