import type { ProvisionalEntity } from '../types/domain';

export function provisionalEntityId(jobId: string, temporaryId: string): string { return `provisional-${jobId}-${temporaryId}`; }

export function matchPriorProvisionalEntity(name: string, aliases: string[], candidates: ProvisionalEntity[]): ProvisionalEntity | undefined {
  const normalized = new Set([name, ...aliases].map((value) => value.trim().toLocaleLowerCase()).filter(Boolean));
  return candidates.find((candidate) => [candidate.canonicalName, ...candidate.aliases].some((value) => normalized.has(value.trim().toLocaleLowerCase())));
}
