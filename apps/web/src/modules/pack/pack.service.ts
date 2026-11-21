import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';
import type { NewProjectValues } from '@/modules/project';

import { fileName, type Imported, type Pack } from './schema';

const ZIP = { 'content-type': 'application/zip' };

export function listPacks(): Promise<ActionResult<Pack[]>> {
  return attempt(async () => {
    const response = await http.get('packs');
    return response.ok ? success((await response.json<{ items: Pack[] }>()).items) : failure(response);
  });
}

/** What a pack file holds, checked in full by the server; nothing is imported. */
export function inspectPack(file: Blob): Promise<ActionResult<Pack>> {
  return attempt(async () => {
    const response = await http.post('packs/inspect', { body: file, headers: ZIP });
    return response.ok ? success(await response.json<Pack>()) : failure(response);
  });
}

/** A new project from a pack of the catalogue (`pack` is its key), or from a pack file. */
export function importPack(pack: string | Blob, values: NewProjectValues): Promise<ActionResult<Imported>> {
  return attempt(async () => {
    const response =
      typeof pack === 'string'
        ? await http.post(`packs/${encodeURIComponent(pack)}/import`, { json: values })
        : await http.post('packs/import', { body: pack, headers: ZIP, searchParams: { ...values } });
    return response.ok ? success(await response.json<Imported>()) : failure(response);
  });
}

export function duplicateProject(
  projectId: string,
  values: NewProjectValues,
  releaseId?: string,
): Promise<ActionResult<Imported>> {
  return attempt(async () => {
    const response = await http.post(`projects/${encodeURIComponent(projectId)}/duplicate`, {
      json: { ...values, releaseId },
    });
    return response.ok ? success(await response.json<Imported>()) : failure(response);
  });
}

export interface PackFile {
  blob: Blob;
  name: string;
}

/** The project as a pack file, fetched rather than linked so a refusal shows on the page. */
export function exportProject(projectId: string, releaseId?: string): Promise<ActionResult<PackFile>> {
  return attempt(async () => {
    const response = await http.get(`projects/${encodeURIComponent(projectId)}/export`, {
      searchParams: releaseId ? { releaseId } : {},
      timeout: 60_000,
    });
    if (!response.ok) return failure(response);
    return success({
      blob: await response.blob(),
      name: fileName(response.headers.get('content-disposition'), 'project.donka-pack.zip'),
    });
  });
}
