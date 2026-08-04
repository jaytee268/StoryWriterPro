import { useRef, useState } from 'react';
import { Check, Merge, Scissors, Upload, X } from 'lucide-react';
import type { ManuscriptImportResult } from '../../types/domain';
import type { StoryRepository } from '../../services/storyRepository';
import { mergeImportChapters, parseManuscriptFile, splitContinuityUnits, splitImportChapter, type ManuscriptImportChapterPreview, type ManuscriptImportPreview, type ContinuityPassageUnit } from '../../services/manuscriptImport';

interface Props {
  projectId: string;
  bookId: string;
  repository: StoryRepository;
  onClose: () => void;
  onImported: (result: ManuscriptImportResult, pageMarkersFound?: number, unitsByChapter?: ContinuityPassageUnit[][]) => Promise<void>;
}

export function ManuscriptImportModal({ projectId, bookId, repository, onClose, onImported }: Props) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [preview, setPreview] = useState<ManuscriptImportPreview>();
  const [sourceFile, setSourceFile] = useState<File>();
  const [removePageMarkers, setRemovePageMarkers] = useState(true);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [splitOffset, setSplitOffset] = useState('');
  const [status, setStatus] = useState<'idle' | 'reading' | 'importing' | 'error'>('idle');
  const [error, setError] = useState('');

  const readFile = async (file?: File) => {
    if (!file) return;
    setStatus('reading'); setError(''); setSourceFile(file);
    try {
      setPreview(await parseManuscriptFile(file, { removePageMarkers }));
      setSelectedIndex(0);
    } catch (reason) {
      setStatus('error'); setError(reason instanceof Error ? reason.message : 'Die Datei konnte nicht gelesen werden.');
      return;
    }
    setStatus('idle');
  };

  const reread = async (nextRemovePageMarkers: boolean) => {
    setRemovePageMarkers(nextRemovePageMarkers);
    if (sourceFile) {
      try { setPreview(await parseManuscriptFile(sourceFile, { removePageMarkers: nextRemovePageMarkers })); }
      catch (reason) { setError(reason instanceof Error ? reason.message : 'Die Datei konnte nicht erneut gelesen werden.'); }
    }
  };

  const updateChapter = (index: number, patch: Partial<ManuscriptImportChapterPreview>) => {
    setPreview((current) => current ? { ...current, chapters: current.chapters.map((chapter, chapterIndex) => chapterIndex === index ? { ...chapter, ...patch } : chapter) } : current);
  };

  const splitSelected = () => {
    if (!preview) return;
    const chapter = preview.chapters[selectedIndex];
    if (!chapter) return;
    try {
      const offset = Number(splitOffset) || Math.floor(chapter.content.length / 2);
      const [first, second] = splitImportChapter(chapter, offset);
      const chapters = [...preview.chapters.slice(0, selectedIndex), first, second, ...preview.chapters.slice(selectedIndex + 1)].map((item, index) => ({ ...item, orderIndex: index + 1 }));
      setPreview({ ...preview, chapters, issues: [], duplicateChapterNumbers: [], missingChapterNumbers: [] });
      setSelectedIndex(selectedIndex + 1); setSplitOffset('');
    } catch (reason) { setError(reason instanceof Error ? reason.message : 'Das Kapitel konnte nicht geteilt werden.'); }
  };

  const mergeSelected = () => {
    if (!preview || selectedIndex >= preview.chapters.length - 1) return;
    const merged = mergeImportChapters(preview.chapters[selectedIndex], preview.chapters[selectedIndex + 1]);
    const chapters = [...preview.chapters.slice(0, selectedIndex), merged, ...preview.chapters.slice(selectedIndex + 2)].map((item, index) => ({ ...item, orderIndex: index + 1 }));
    setPreview({ ...preview, chapters, issues: [], duplicateChapterNumbers: [], missingChapterNumbers: [] });
  };

  const importNow = async () => {
    if (!preview || preview.chapters.length === 0) return;
    setStatus('importing'); setError('');
    try {
      const result = await repository.importManuscript({ projectId, bookId, chapters: preview.chapters.map(({ title, content }) => ({ title, content })) });
      await onImported(result, preview.pageMarkersFound, preview.chapters.map((chapter) => splitContinuityUnits(chapter.content, chapter.pageMarkers)));
    } catch (reason) {
      setStatus('error'); setError(reason instanceof Error ? reason.message : 'Der Import konnte nicht gespeichert werden.');
    }
  };

  const selected = preview?.chapters[selectedIndex];
  const blocking = (preview?.issues.some((issue) => issue.severity === 'error') ?? false) || (preview?.chapters.some((chapter) => !chapter.title.trim()) ?? false);
  return <div className="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="import-title">
    <div className="modal simple-modal manuscript-import-modal">
      <div className="modal-head"><div><span className="eyebrow">SICHERER IMPORT</span><h2 id="import-title">Manuskript importieren</h2></div><button className="icon-button" onClick={onClose} aria-label="Dialog schließen"><X size={20} /></button></div>
      <p className="modal-intro">Kapitel werden erkannt und als Kapiteltext importiert. Szenen musst du nicht vorher anlegen.</p>
      {!preview && <button className="drop-zone simple-drop" onClick={() => inputRef.current?.click()}><Upload size={28} /><strong>Datei auswählen</strong><span>TXT, Markdown oder DOCX</span><input ref={inputRef} hidden type="file" accept=".txt,.md,.markdown,.docx,text/plain,text/markdown,application/vnd.openxmlformats-officedocument.wordprocessingml.document" onChange={(event) => void readFile(event.target.files?.[0])} /></button>}
      {status === 'reading' && <p className="modal-intro">Datei wird gelesen …</p>}
      {preview && <>
        <div className="import-options"><label className="field-label">Datei<strong>{preview.fileName}</strong></label><label className="checkbox-row"><input type="checkbox" checked={removePageMarkers} onChange={(event) => void reread(event.target.checked)} /> Seitenmarker entfernen</label></div>
        <div className="import-preview-head"><strong>{preview.chapters.length} Kapitel erkannt</strong><span>{preview.chapters.reduce((sum, chapter) => sum + chapter.wordCount, 0).toLocaleString('de-DE')} Wörter</span><button className="text-button" onClick={() => inputRef.current?.click()}>Andere Datei</button><input ref={inputRef} hidden type="file" accept=".txt,.md,.markdown,.docx" onChange={(event) => void readFile(event.target.files?.[0])} /></div>
        {(preview.issues.length > 0 || error) && <div className="import-issues">{preview.issues.map((issue) => <div key={`${issue.message}-${issue.chapterIndex ?? 'all'}`} className={issue.severity === 'error' ? 'save-error' : 'provider-notice'}>{issue.message}</div>)}{error && <div className="save-error">{error}</div>}</div>}
        <div className="import-chapter-list">{preview.chapters.map((chapter, index) => <button key={chapter.id} className={`import-chapter-row ${selectedIndex === index ? 'active' : ''}`} onClick={() => setSelectedIndex(index)}><span>{String(index + 1).padStart(2, '0')}</span><strong>{chapter.title}</strong><small>{chapter.wordCount.toLocaleString('de-DE')} Wörter</small></button>)}</div>
        {selected && <div className="import-editor"><label className="field-label">Kapitelname<input value={selected.title} onChange={(event) => updateChapter(selectedIndex, { title: event.target.value })} /></label><label className="field-label">Kapiteltext<textarea value={selected.content} onChange={(event) => updateChapter(selectedIndex, { content: event.target.value, wordCount: event.target.value.trim() ? event.target.value.trim().split(/\s+/u).length : 0 })} rows={8} /></label><div className="import-edit-actions"><label className="field-label compact">Teilen bei Zeichen<input value={splitOffset} onChange={(event) => setSplitOffset(event.target.value)} placeholder={`${Math.floor(selected.content.length / 2)}`} /></label><button className="ghost-button" onClick={splitSelected}><Scissors size={15} /> Kapitel teilen</button><button className="ghost-button" disabled={selectedIndex >= preview.chapters.length - 1} onClick={mergeSelected}><Merge size={15} /> Mit nächstem verbinden</button></div></div>}
        <div className="modal-actions"><button className="ghost-button" onClick={onClose}>Abbrechen</button><button className="primary-button" disabled={blocking || status === 'importing' || status === 'reading'} onClick={() => void importNow()}>{status === 'importing' ? 'Import wird gespeichert …' : <><Check size={16} /> Kapitel importieren</>}</button></div>
      </>}
      {!preview && error && <div className="save-error">{error}</div>}
    </div>
  </div>;
}
