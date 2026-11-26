import { copyFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const source = path.join(webRoot, 'node_modules/@scalar/api-reference/dist/browser/standalone.js');
const destination = path.join(webRoot, 'public/scalar-api-reference.js');

await mkdir(path.dirname(destination), { recursive: true });
await copyFile(source, destination);
