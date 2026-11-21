// Fails when a locale misses a key of the reference locale (English) or has extra ones,
// or when a message uses different {placeholders} than its English original.
import { readFileSync } from 'node:fs';

const LOCALES = ['en', 'fr'];
const REFERENCE = 'en';

function flatten(node, prefix = '', out = new Map()) {
  for (const [key, value] of Object.entries(node)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value !== null && typeof value === 'object') flatten(value, path, out);
    else out.set(path, String(value));
  }
  return out;
}

const placeholders = (message) =>
  [...message.matchAll(/\{(\w+)/g)]
    .map((m) => m[1])
    .sort()
    .join(',');
const load = (locale) => flatten(JSON.parse(readFileSync(new URL(`../src/messages/${locale}.json`, import.meta.url))));

const reference = load(REFERENCE);
const problems = [];
for (const locale of LOCALES.filter((l) => l !== REFERENCE)) {
  const messages = load(locale);
  for (const [key, message] of reference) {
    if (!messages.has(key)) problems.push(`${locale}: missing ${key}`);
    else if (placeholders(messages.get(key)) !== placeholders(message))
      problems.push(
        `${locale}: ${key} uses {${placeholders(messages.get(key))}} instead of {${placeholders(message)}}`,
      );
    else if (!messages.get(key).trim()) problems.push(`${locale}: empty ${key}`);
  }
  for (const key of messages.keys()) if (!reference.has(key)) problems.push(`${locale}: extra ${key}`);
}

if (problems.length) {
  console.error(problems.join('\n'));
  process.exit(1);
}
console.log(`i18n: ${reference.size} keys, ${LOCALES.join(' and ')} match`);
