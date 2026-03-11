import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAgentStore } from '../../store';

interface Props {
  onReindex: () => void;
}

export function ModelControl({ onReindex }: Props) {
  const { indexedChunks, isIndexing } = useAgentStore();
  const [status, setStatus] = useState<'checking' | 'online' | 'offline'>('checking');
  const [showSettings, setShowSettings] = useState(false);

  useEffect(() => {
    checkOllama();
    const interval = setInterval(checkOllama, 8000);
    return () => clearInterval(interval);
  }, []);

  const checkOllama = async () => {
    try {
      await invoke('check_ollama');
      setStatus('online');
    } catch {
      setStatus('offline');
    }
  };

  return (
    <div className="border-b border-gray-800">
      <div className="px-3 py-1.5 flex items-center gap-2">
        {/* Ollama status dot */}
        <div
          title={`Ollama ${status}`}
          className={`w-2 h-2 rounded-full flex-shrink-0 ${
            status === 'online'  ? 'bg-green-500' :
            status === 'offline' ? 'bg-red-500 animate-pulse' :
            'bg-yellow-500 animate-pulse'
          }`}
        />
        <span className="flex-1" />
        <button
          onClick={() => setShowSettings(s => !s)}
          title="Settings"
          className={`text-xs px-1 transition-colors ${showSettings ? 'text-blue-400' : 'text-gray-600 hover:text-gray-400'}`}
        >
          ⚙
        </button>
      </div>

      {showSettings && (
        <div className="px-3 pb-3 space-y-2 bg-gray-900/50 border-t border-gray-800 text-xs">
          <div className="pt-2 text-gray-500 font-medium uppercase tracking-wider">Index</div>
          <div className="flex items-center justify-between">
            <span className="text-gray-400">
              {isIndexing ? '⟳ Indexing…' : indexedChunks > 0 ? `✓ ${indexedChunks.toLocaleString()} chunks` : '○ Not indexed'}
            </span>
            <button
              onClick={onReindex}
              disabled={isIndexing}
              className="px-2 py-0.5 bg-gray-800 hover:bg-gray-700 disabled:opacity-40 rounded text-gray-300 transition-colors"
            >
              {isIndexing ? 'Indexing…' : 'Re-index'}
            </button>
          </div>
          {status === 'offline' && (
            <div className="text-red-400">
              Ollama offline — restart Moses to reconnect.
            </div>
          )}
        </div>
      )}
    </div>
  );
}
