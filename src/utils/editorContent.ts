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
  if (typeof document === 'undefined') return content.replace(/<br\s*\/?>/gi, '\n').replace(/<\/p>|<\/div>/gi, '\n').replace(/<[^>]+>/g, '');
  const container = document.createElement('div');
  container.innerHTML = content;
  return (container.innerText || container.textContent || '').replace(/\u00a0/g, ' ');
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character] ?? character);
}
