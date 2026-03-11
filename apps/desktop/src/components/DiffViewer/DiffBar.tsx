import { useState } from 'react';

interface Props {
  diff: string;
  onAccept: () => void;
  onReject: () => void;
}

export function DiffBar({ diff, onAccept, onReject }: Props) {
  const [showPreview, setShowPreview] = useState(false);
  const linesChanged = diff.split('\n').filter(l => l.startsWith('+') || l.startsWith('-')).length;

  return (
    <>
      <div className="border-t border-yellow-700 bg-yellow-950/50 px-3 py-2 flex items-center gap-3 text-sm">
        <span className="text-yellow-400">✎ Moses proposes {linesChanged} line changes</span>
        <button
          onClick={onAccept}
          className="px-3 py-1 bg-green-700 hover:bg-green-600 text-white rounded text-xs transition-colors"
        >
          Accept
        </button>
        <button
          onClick={onReject}
          className="px-3 py-1 bg-red-800 hover:bg-red-700 text-white rounded text-xs transition-colors"
        >
          Reject
        </button>
        <button
          onClick={() => setShowPreview(!showPreview)}
          className="px-3 py-1 bg-gray-700 hover:bg-gray-600 text-white rounded text-xs transition-colors"
        >
          {showPreview ? 'Hide' : 'Preview'}
        </button>
      </div>
      {showPreview && (
        <div className="border-t border-gray-800 max-h-80 overflow-y-auto bg-gray-950">
          <pre className="text-xs p-3 text-gray-300 whitespace-pre-wrap">{diff}</pre>
        </div>
      )}
    </>
  );
}
