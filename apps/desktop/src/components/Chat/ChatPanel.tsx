import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AgentEvent, useAgentStore } from '../../store';
import mosesLogo from '../../assets/moses.png';

interface Props {
  events: AgentEvent[];
  onSend: (text: string) => void;
  isRunning: boolean;
}

export function ChatPanel({ events, onSend, isRunning }: Props) {
  const { streamBuffer, clearEvents } = useAgentStore();
  const [input, setInput] = useState('');
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [events, streamBuffer]);

  const handleSend = () => {
    const text = input.trim();
    if (!text || isRunning) return;
    setInput('');
    onSend(text);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) handleSend();
  };

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <div className="flex-1 overflow-y-auto p-3 space-y-2 text-sm font-mono">
        {events.length === 0 && !streamBuffer && (
          <div className="text-gray-600 text-xs text-center mt-8 leading-relaxed space-y-1">
            <img src={mosesLogo} alt="Moses" className="w-14 h-14 rounded-xl mx-auto mb-3 opacity-90" />
            <div className="text-gray-400 font-medium">Moses is ready</div>
            <div className="text-gray-600">Open a workspace, select a file, then ask.</div>
            <div className="text-gray-700 mt-2">
              "create a test file for this"<br />
              "add error handling"<br />
              "explain what this does"<br />
              "build the auth module"
            </div>
          </div>
        )}

        {events.map((event, i) => (
          <EventBubble key={i} event={event} />
        ))}

        {streamBuffer && (
          <div className="text-gray-200 text-sm bg-gray-900/80 rounded-lg p-3 whitespace-pre-wrap border-l-2 border-blue-600">
            {streamBuffer}
            <span className="animate-pulse text-blue-400">▋</span>
          </div>
        )}

        {isRunning && !streamBuffer && (
          <div className="flex items-center gap-2 text-blue-400 text-xs">
            <span className="animate-pulse">●</span>
            <span>Working…</span>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      <div className="p-2 border-t border-gray-800 space-y-1.5">
        <textarea
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={isRunning}
          placeholder={isRunning ? 'Moses is working…' : 'Ask Moses anything — ⌘Enter to send'}
          className="w-full bg-gray-900 border border-gray-700 rounded-lg p-2.5 text-sm resize-none h-20 focus:outline-none focus:border-blue-600 disabled:opacity-40 text-gray-100 placeholder-gray-600 transition-colors"
        />
        <div className="flex gap-1.5">
          <button
            onClick={handleSend}
            disabled={isRunning || !input.trim()}
            className="flex-1 bg-blue-700 hover:bg-blue-600 disabled:bg-gray-800 disabled:text-gray-600 text-white rounded-lg py-1.5 text-xs font-medium transition-colors"
          >
            {isRunning ? 'Working…' : 'Send  ⌘↵'}
          </button>
          <button
            onClick={clearEvents}
            title="Clear"
            className="px-2.5 py-1.5 bg-gray-800 hover:bg-gray-700 text-gray-500 hover:text-gray-300 rounded-lg text-xs transition-colors"
          >
            ✕
          </button>
        </div>
      </div>
    </div>
  );
}

function EventBubble({ event }: { event: AgentEvent }) {
  switch (event.type) {
    case 'Thinking':
      return (
        <div className="text-gray-500 text-xs flex items-center gap-1.5">
          <span className="animate-spin inline-block">⟳</span>
          {event.content}
        </div>
      );

    case 'ConfirmWrite':
      return <ConfirmWriteBubble id={event.id} path={event.path} preview={event.preview} />;

    case 'FileWritten':
      return (
        <div className="text-green-400 text-xs bg-green-950/20 rounded-lg px-2.5 py-1.5">
          ✓ Saved: <span className="text-green-300 font-mono">{event.path}</span>
        </div>
      );

    case 'Stream':
      return (
        <div className="text-gray-200 text-sm bg-gray-900/60 rounded-lg p-3 whitespace-pre-wrap">
          {event.content}
        </div>
      );

    case 'Done':
      return event.summary
        ? <div className="text-gray-400 text-xs italic">{event.summary}</div>
        : null;

    case 'Error':
      return (
        <div className="text-red-400 text-xs bg-red-950/30 rounded-lg px-2.5 py-1.5">
          ✕ {event.message}
        </div>
      );

    default:
      return null;
  }
}

function ConfirmWriteBubble({ id, path, preview }: { id: string; path: string; preview: string }) {
  const [state, setState] = useState<'pending' | 'approved' | 'denied'>('pending');

  const respond = async (approved: boolean) => {
    setState(approved ? 'approved' : 'denied');
    await invoke('confirm_action', { id, approved });
  };

  if (state === 'approved') {
    return (
      <div className="text-green-500 text-xs bg-green-950/20 rounded-lg px-2.5 py-1.5">
        ✓ Writing: <span className="font-mono">{path}</span>
      </div>
    );
  }
  if (state === 'denied') {
    return (
      <div className="text-gray-600 text-xs bg-gray-900/40 rounded-lg px-2.5 py-1.5">
        ✕ Skipped: <span className="font-mono">{path}</span>
      </div>
    );
  }

  return (
    <div className="text-yellow-300 text-xs bg-yellow-950/30 border border-yellow-800/50 rounded-lg p-3 space-y-2">
      <div className="font-medium text-yellow-200">Moses wants to write:</div>
      <div className="font-mono text-yellow-400 text-xs">{path}</div>
      {preview && (
        <pre className="text-gray-400 text-xs bg-gray-900/60 rounded p-2 max-h-32 overflow-y-auto whitespace-pre-wrap">
          {preview}
        </pre>
      )}
      <div className="flex gap-2 pt-1">
        <button
          onClick={() => respond(true)}
          className="px-3 py-1 bg-yellow-700 hover:bg-yellow-600 text-white rounded text-xs font-medium transition-colors"
        >
          Allow
        </button>
        <button
          onClick={() => respond(false)}
          className="px-3 py-1 bg-gray-700 hover:bg-gray-600 text-gray-200 rounded text-xs font-medium transition-colors"
        >
          Deny
        </button>
      </div>
    </div>
  );
}
