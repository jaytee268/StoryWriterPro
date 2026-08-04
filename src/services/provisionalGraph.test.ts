import { describe, expect, it } from 'vitest';
import { matchPriorProvisionalEntity, provisionalEntityId } from './provisionalGraph';

describe('provisional manuscript graph identity', () => {
  const entity = { id: 'p-1', jobId: 'job-1', projectId: 'project-1', entityType: 'character' as const, canonicalName: 'Elena Berger', aliases: ['Lena', 'Berger'], description: '', confidence: 0.8, reviewStatus: 'proposed' as const, createdAt: '', updatedAt: '' };
  it('keeps a stable job-scoped id and matches aliases without merging silently', () => {
    expect(provisionalEntityId('job-1', 'character-1')).toBe(provisionalEntityId('job-1', 'character-1'));
    expect(matchPriorProvisionalEntity('Lena', [], [entity])).toBe(entity);
    expect(matchPriorProvisionalEntity('Nina', [], [entity])).toBeUndefined();
  });
  it('does not use later candidates when the earlier graph is empty', () => {
    expect(matchPriorProvisionalEntity('er', [], [])).toBeUndefined();
  });
});
