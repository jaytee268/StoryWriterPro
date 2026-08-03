const editorTags = /<(?:p|div|br|strong|b|em|i|u|s|ul|ol|li|blockquote|a|table|tbody|tr|td|img)\b/i;

export function editorContentToHtml(content: string): string {
  if (!content) return '<p><br></p>';
  if (editorTags.test(content)) return content;
  return content
    .split(/\n{2,}/)
    .map((paragraph) => `<p>${escapeHtml(paragraph).replace(/\n/g, '<br>')}</p>`)
    .join('');
}

export function editorContentToPlainText(content: string): string {
  if (!editorTags.test(content)) return content;
  if (typeof document === 'undefined') return content.replace(/<br\s*\/?>/gi, '\n').replace(/<\/(?:p|div|li|blockquote)>/gi, '\n').replace(/<[^>]+>/g, '').replace(/\n+$/g, '');
  const container = document.createElement('div');
  container.innerHTML = content;
  const output: string[] = [];
  const visit = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) { output.push(node.textContent ?? ''); return; }
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    const element = node as HTMLElement;
    if (element.tagName === 'BR') { output.push('\n'); return; }
    for (const child of Array.from(element.childNodes)) visit(child);
    if (/^(P|DIV|LI|BLOCKQUOTE)$/.test(element.tagName)) output.push('\n');
  };
  for (const child of Array.from(container.childNodes)) visit(child);
  return output.join('').replace(/\u00a0/g, ' ').replace(/\n{3,}/g, '\n\n').replace(/\n+$/g, '');
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character] ?? character);
}
