import { editorContentToPlainText } from './editorContent';
import { unicodeSlice } from './aiText';

function nodeLength(node: Node): number {
  if (node.nodeType === Node.TEXT_NODE) return Array.from(node.nodeValue ?? '').length;
  if (node.nodeType !== Node.ELEMENT_NODE) return 0;
  const element = node as HTMLElement;
  if (element.tagName === 'BR') return 1;
  let length = Array.from(element.childNodes).reduce((sum, child) => sum + nodeLength(child), 0);
  if (/^(P|DIV|LI|BLOCKQUOTE)$/.test(element.tagName)) length += 1;
  return length;
}

function pointOffset(root: HTMLElement, node: Node, offset: number): number {
  if (node === root) return Array.from(root.childNodes).slice(0, offset).reduce((sum, child) => sum + nodeLength(child), 0);
  let position = 0;
  let current: Node | null = node;
  while (current && current !== root) {
    const parent: Node | null = current.parentNode;
    if (!parent) break;
    const siblings = Array.from(parent.childNodes) as Node[];
    const index = siblings.indexOf(current);
    position += Array.from(parent.childNodes).slice(0, index).reduce<number>((sum, child) => sum + nodeLength(child), 0);
    current = parent;
  }
  if (node.nodeType === Node.TEXT_NODE) return position + Array.from((node.nodeValue ?? '').slice(0, offset)).length;
  return position + Array.from(node.childNodes).slice(0, Math.min(offset, node.childNodes.length)).reduce((sum, child) => sum + nodeLength(child), 0);
}

export interface EditorSelectionSnapshot { excerpt: string; startOffset: number; endOffset: number; }

export function selectionToUnicodeOffsets(root: HTMLElement, range: Range): EditorSelectionSnapshot | undefined {
  if (range.collapsed || !root.contains(range.startContainer) || !root.contains(range.endContainer)) return undefined;
  const plainText = editorContentToPlainText(root.innerHTML);
  const max = Array.from(plainText).length;
  const startOffset = Math.min(max, Math.max(0, pointOffset(root, range.startContainer, range.startOffset)));
  const endOffset = Math.min(max, Math.max(startOffset, pointOffset(root, range.endContainer, range.endOffset)));
  const excerpt = unicodeSlice(plainText, startOffset, endOffset);
  return excerpt ? { excerpt, startOffset, endOffset } : undefined;
}
