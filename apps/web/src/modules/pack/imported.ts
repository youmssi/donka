import type { Imported } from './schema';

/** How the scenarios went on a new project's first versions: all passed, or how many did not. */
export function testOutcome(imported: Imported): { passed: number; failing: number } {
  const { passed, failed, errors } = imported.tests;
  return { passed, failing: failed + errors };
}
