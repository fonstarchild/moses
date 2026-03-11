import { useEffect, useRef } from 'react';
import mosesLogo from '../../assets/moses.png';

interface SetupProgress {
  step: string;
  detail: string;
  done: boolean;
  error: string | null;
}

interface Props {
  progress: SetupProgress[];
  current: SetupProgress | null;
}

function parsePercent(detail: string): number | null {
  const m = detail.match(/(\d+)%/);
  return m ? parseInt(m[1], 10) : null;
}

function isDownloadStep(step: string) {
  return step.toLowerCase().includes('download');
}

// Format a log line from a progress entry
function toLogLine(p: SetupProgress): string {
  if (p.error) return `✗ ${p.step}: ${p.error}`;
  if (p.detail) return `› ${p.step}: ${p.detail}`;
  return `› ${p.step}`;
}

export function SetupScreen({ progress, current }: Props) {
  const consoleRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (consoleRef.current) {
      consoleRef.current.scrollTop = consoleRef.current.scrollHeight;
    }
  }, [progress, current]);

  const hasError = current?.error != null;
  const pct = current && isDownloadStep(current.step) ? parsePercent(current.detail) : null;

  // All console lines: completed steps + current (if any)
  const logLines = [
    ...progress.map(p => ({ text: `✓ ${p.step}${p.detail ? `: ${p.detail}` : ''}`, done: true, error: false })),
    ...(current ? [{ text: toLogLine(current), done: false, error: hasError }] : []),
  ];

  return (
    <div className="h-screen w-screen bg-gray-950 flex flex-col items-center justify-center p-8">
      {/* Logo */}
      <div className="mb-6 text-center">
        <img src={mosesLogo} alt="Moses" className="w-20 h-20 rounded-2xl mx-auto mb-3 shadow-lg" />
        <h1 className="text-2xl font-bold text-gray-100 tracking-tight">Moses</h1>
        <p className="text-gray-500 text-sm mt-1">Local AI coding assistant</p>
      </div>

      {/* Progress card */}
      <div className="w-full max-w-lg bg-gray-900 rounded-xl border border-gray-800 overflow-hidden">

        {/* Current step header */}
        <div className={`px-5 py-4 border-b border-gray-800 ${hasError ? 'bg-red-950/30' : ''}`}>
          {hasError ? (
            <div>
              <div className="text-red-400 font-medium">{current?.step}</div>
              <div className="text-red-500/70 text-sm mt-1">{current?.error}</div>
              <div className="text-gray-500 text-xs mt-3">
                Try running <code className="text-gray-400 bg-gray-800 px-1 rounded">ollama serve</code> manually, then restart Moses.
              </div>
            </div>
          ) : current ? (
            <div>
              <div className="flex items-center gap-3">
                <div className="w-4 h-4 border-2 border-blue-500 border-t-transparent rounded-full animate-spin flex-shrink-0" />
                <div className="flex-1 min-w-0">
                  <div className="text-gray-200 text-sm font-medium">{current.step}</div>
                  {current.detail && pct == null && (
                    <div className="text-gray-500 text-xs mt-0.5 truncate">{current.detail}</div>
                  )}
                </div>
                {pct != null && (
                  <span className="text-blue-400 text-sm font-mono tabular-nums flex-shrink-0">{pct}%</span>
                )}
              </div>
              {/* Download progress bar */}
              {pct != null && (
                <div className="mt-3">
                  <div className="w-full bg-gray-800 rounded-full h-1.5 overflow-hidden">
                    <div
                      className="bg-blue-500 h-1.5 rounded-full transition-all duration-300"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                  <div className="text-gray-600 text-xs mt-1.5 truncate">{current.detail}</div>
                </div>
              )}
            </div>
          ) : (
            <div className="flex items-center gap-3 text-gray-500 text-sm">
              <div className="w-4 h-4 border-2 border-gray-700 border-t-transparent rounded-full animate-spin flex-shrink-0" />
              Initializing…
            </div>
          )}
        </div>

        {/* Mini console */}
        <div
          ref={consoleRef}
          className="px-4 py-3 h-36 overflow-y-auto font-mono text-xs bg-gray-950/60 space-y-0.5"
        >
          {logLines.length === 0 ? (
            <div className="text-gray-700">Waiting for setup to start…</div>
          ) : (
            logLines.map((line, i) => (
              <div
                key={i}
                className={
                  line.error ? 'text-red-400' :
                  line.done  ? 'text-green-700' :
                  'text-gray-400'
                }
              >
                {line.text}
              </div>
            ))
          )}
          {/* blinking cursor on last line */}
          {!hasError && current && (
            <span className="inline-block w-1.5 h-3 bg-gray-500 animate-pulse ml-0.5 align-middle" />
          )}
        </div>
      </div>

      {/* Footer */}
      <p className="text-gray-700 text-xs mt-5 text-center max-w-sm">
        First launch downloads Ollama and a DeepSeek model (~4 GB).<br />
        This only happens once.
      </p>
    </div>
  );
}
