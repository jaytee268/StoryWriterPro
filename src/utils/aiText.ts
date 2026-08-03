import type { Scene } from '../types/domain';
import { editorContentToPlainText } from './editorContent';

export function unicodeSlice(text: string, start: number, end: number): string {
  return Array.from(text).slice(start, end).join('');
}

export function unicodeIndexOf(text: string, needle: string): number {
  const source = Array.from(text);
  const target = Array.from(needle);
  if (!target.length) return 0;
  return source.findIndex((_, index) => target.every((character, offset) => source[index + offset] === character));
}

export function contentHash(text: string): string {
  let hash = 2166136261;
  for (const character of Array.from(text)) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}

export interface CanonicalAiScene {
  scene: Scene;
  text: string;
  hash: string;
}

export function canonicalizeSceneForAi(scene: Scene): CanonicalAiScene {
  const text = editorContentToPlainText(scene.content);
  return { scene: { ...scene, content: text }, text, hash: contentHash(text) };
}
