import { useAgentStore } from '../../store';

export function StatusBar() {
  const { workspace, indexedChunks, isIndexing, isRunning } = useAgentStore();

  return (
    <div className="h-6 bg-gray-900 border-t border-gray-800 flex items-center px-3 gap-4 text-xs text-gray-600 flex-shrink-0">
      {/* Left: workspace */}
      <span className="truncate max-w-48">
        {workspace ? `📁 ${workspace}` : 'No workspace'}
      </span>

      <span className="flex-1" />

      {/* Index status */}
      {isIndexing && (
        <span className="text-yellow-600 animate-pulse">⟳ Indexing…</span>
      )}
      {!isIndexing && indexedChunks > 0 && (
        <span className="text-gray-600">{indexedChunks.toLocaleString()} chunks indexed</span>
      )}

      {/* Agent status */}
      {isRunning && (
        <span className="text-blue-500 animate-pulse">● Working…</span>
      )}

      {/* Version */}
      <span className="text-gray-800">Moses v0.1</span>
    </div>
  );
}
