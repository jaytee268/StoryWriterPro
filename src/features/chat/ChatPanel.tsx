import { useState } from 'react';
import { AtSign, Send, WandSparkles } from 'lucide-react';
import type { ChatMessage, ContextRequest, StorySourceReference } from '../../types/domain';
import type { ProjectContextBuilder } from '../../services/contextBuilder';
import { answerFromProjectContext } from '../../services/providerBridge';

interface ChatPanelProps {
  messages: ChatMessage[];
  onMessagesChange: (messages: ChatMessage[]) => void;
  contextBuilder: ProjectContextBuilder;
  contextRequest: Omit<ContextRequest, 'userQuestion'>;
  onOpenSourceReference: (reference: StorySourceReference) => void;
}

export function ChatPanel({ messages, onMessagesChange, contextBuilder, contextRequest, onOpenSourceReference }: ChatPanelProps) {
  const [input, setInput] = useState('');
  const [busy, setBusy] = useState(false);
  const send = async (text = input) => {
    if (!text.trim() || busy) return;
    const question = text.trim();
    const user: ChatMessage = { id: crypto.randomUUID(), role: 'user', content: question, time: 'Jetzt' };
    const withUser = [...messages, user];
    onMessagesChange(withUser);
    setInput('');
    setBusy(true);
    try {
      const context = await contextBuilder.build({ ...contextRequest, userQuestion: question });
      const answer = answerFromProjectContext(question, context);
      const assistant: ChatMessage = { id: crypto.randomUUID(), role: 'assistant', content: answer.text, sources: answer.sources, time: 'Jetzt' };
      onMessagesChange([...withUser, assistant]);
    } catch (error) {
      onMessagesChange([...withUser, { id: crypto.randomUUID(), role: 'assistant', content: error instanceof Error ? `Ich konnte den Projektkontext nicht laden: ${error.message}` : 'Der Projektkontext konnte nicht geladen werden.', time: 'Jetzt' }]);
    } finally { setBusy(false); }
  };
  return <aside className="chat-panel simple-chat"><div className="simple-chat-head"><span className="assistant-avatar"><WandSparkles size={15} /></span><div><strong>Projektassistent</strong><small>Antwortet nur aus deinem lokalen Projektkontext.</small></div></div><div className="chat-messages">{messages.map((message) => <div className={`message ${message.role}`} key={message.id}><div className="message-label">{message.role === 'assistant' ? 'Assistent' : 'Du'}<span>{message.time}</span></div><p>{message.content}</p>{message.sources?.length ? <div className="source-chips">{message.sources.map((source) => <button key={source.id} className="source-chip-button" onClick={() => source.sceneId && onOpenSourceReference({ id: source.id, projectId: contextRequest.projectId, chapterId: source.chapterId ?? '', sceneId: source.sceneId, entityId: source.entityId, excerpt: source.excerpt ?? '', startOffset: source.startOffset, endOffset: source.endOffset, createdAt: '' })}><AtSign size={11} />{source.label}</button>)}</div> : null}</div>)}</div><div className="simple-chat-suggestions"><button onClick={() => void send('Welche Figuren kommen in der aktuellen Szene vor?')}>Figuren der Szene</button><button onClick={() => void send('Welche offenen Handlungsstränge betreffen diese Szene?')}>Offene Handlungsstränge</button><button onClick={() => void send('Welche Vermutungen sind noch unbestätigt?')}>Unbestätigte Vermutungen</button></div><div className="chat-compose"><textarea value={input} onChange={(event) => setInput(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); void send(); } }} placeholder="Eine Frage zu deinem Buch …" rows={3} disabled={busy} /><button className="send-button large-send" onClick={() => void send()} aria-label="Nachricht senden" disabled={busy}><Send size={17} /></button></div></aside>;
}
