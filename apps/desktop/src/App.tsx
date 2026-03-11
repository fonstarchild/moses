import { useState, useEffect } from 'react';
import { listen, emit } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/tauri';
import { open } from '@tauri-apps/api/dialog';
import Editor from '@monaco-editor/react';
import { useAgentStore } from './store';
import { ChatPanel } from './components/Chat/ChatPanel';
import { FileExplorer } from './components/FileExplorer/FileExplorer';
import { ModelControl } from './components/ModelControl/ModelControl';
import { DiffBar } from './components/DiffViewer/DiffBar';
import { StatusBar } from './components/StatusBar/StatusBar';
import { SetupScreen } from './components/Setup/SetupScreen';

const MODEL = 'deepseek-coder:6.7b';

interface SetupProgress {
  step: string;
  detail: string;
  done: boolean;
  error: string | null;
}

export default function App() {
  const {
    events, addEvent, appendToken, flushStream,
    pendingPatch, setPendingPatch,
    isRunning, setRunning,
    workspace, setWorkspace,
    setIndexedChunks, setIndexing,
  } = useAgentStore();

  const [setupDone, setSetupDone] = useState(false);
  const [explorerRefreshKey, setExplorerRefreshKey] = useState(0);
  const [setupLog, setSetupLog] = useState<SetupProgress[]>([]);
  const [setupCurrent, setSetupCurrent] = useState<SetupProgress | null>(null);

  // Listen for setup progress — signal Rust once listener is ready
  useEffect(() => {
    const unlisten = listen<SetupProgress>('setup-progress', (event) => {
      const p = event.payload;
      if (p.done) { setSetupDone(true); return; }
      if (p.error) { setSetupCurrent(p); return; }
      setSetupCurrent(prev => {
        if (prev && !prev.error) setSetupLog(log => [...log, prev]);
        return p;
      });
    });
    unlisten.then(() => emit('setup-ready'));
    return () => { unlisten.then(f => f()); };
  }, []);

  const [activeFile, setActiveFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState('');

  // Restore last workspace on launch
  useEffect(() => {
    if (!setupDone) return;
    invoke<{ workspace?: string }>('load_settings').then((s) => {
      if (s.workspace) {
        setWorkspace(s.workspace);
        invoke('set_workspace', { path: s.workspace }).catch(() => {});
        triggerIndex(s.workspace);
      }
    }).catch(() => {});
    // Lock model to deepseek
    invoke('set_model', { model: MODEL }).catch(() => {});
  }, [setupDone]);

  // File watcher → update index count + reload active file
  useEffect(() => {
    const unlisten = listen<{ kind: string; path: string; chunks: number }>('file-changed', (event) => {
      const { kind, path, chunks } = event.payload;
      if (kind !== 'deleted' && chunks > 0) setIndexedChunks(prev => Math.max(0, prev + chunks));
      if (kind === 'modified' && activeFile && activeFile.endsWith(path)) handleFileSelect(activeFile);
    });
    return () => { unlisten.then(f => f()); };
  }, [activeFile]);

  // Agent events
  useEffect(() => {
    const unlisten = listen<any>('agent-event', (event) => {
      const p = event.payload;
      if (p.type === 'StreamToken') { appendToken(p.token); return; }
      if (p.type === 'Done' || p.type === 'Error') {
        flushStream();
        addEvent(p);
        setRunning(false);
        return;
      }
      if (p.type === 'FileWritten') {
        setExplorerRefreshKey(k => k + 1);
        if (activeFile && p.path === activeFile) handleFileSelect(activeFile);
      }
      addEvent(p);
    });
    return () => { unlisten.then(f => f()); };
  }, [activeFile]);

  const handleSelectWorkspace = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== 'string') return;
    setWorkspace(selected);
    await invoke('set_workspace', { path: selected });
    invoke('save_workspace_setting', { path: selected }).catch(() => {});
    triggerIndex(selected);
  };

  const triggerIndex = async (root: string) => {
    setIndexing(true);
    try {
      const count = await invoke<number>('index_workspace', { root });
      setIndexedChunks(count);
    } catch { /* best-effort */ }
    finally { setIndexing(false); }
  };

  const handleFileSelect = async (path: string) => {
    setActiveFile(path);
    try {
      const content = await invoke<string>('read_file', { path });
      setFileContent(content);
    } catch (e) {
      setFileContent(`// Error reading file: ${e}`);
    }
  };

  const handleSend = async (text: string) => {
    if (!workspace) {
      addEvent({ type: 'Error', message: 'Open a workspace first — click the folder button top-left.' });
      return;
    }
    setRunning(true);
    try {
      await invoke('run_agent', {
        task: {
          prompt: text,
          workspace_root: workspace,
          open_files: activeFile ? [activeFile] : [],
          mode: 'Edit',
        },
      });
    } catch (e) {
      flushStream();
      addEvent({ type: 'Error', message: `${e}` });
      setRunning(false);
    }
  };

  const handleAcceptPatch = async () => {
    if (!pendingPatch) return;
    try {
      await invoke('apply_patch_cmd', { diff: pendingPatch, workspaceRoot: workspace });
      setPendingPatch(null);
      if (activeFile) handleFileSelect(activeFile);
    } catch (e) {
      addEvent({ type: 'Error', message: `Failed to apply patch: ${e}` });
    }
  };

  if (!setupDone) {
    return <SetupScreen progress={setupLog} current={setupCurrent} />;
  }

  return (
    <div className="flex flex-col h-screen bg-gray-950 text-gray-100 overflow-hidden">
      <div className="flex flex-1 min-h-0">

        {/* Left: file explorer */}
        <div className="w-52 border-r border-gray-800 flex flex-col flex-shrink-0">
          <div className="p-2 border-b border-gray-800">
            <button
              onClick={handleSelectWorkspace}
              className="w-full text-left text-xs text-gray-400 hover:text-gray-200 truncate px-2 py-1.5 rounded hover:bg-gray-800 transition-colors"
            >
              {workspace ? `📁 ${workspace.split('/').pop()}` : '📂 Open Workspace'}
            </button>
          </div>
          <FileExplorer workspace={workspace} onFileSelect={handleFileSelect} activeFile={activeFile} refreshKey={explorerRefreshKey} />
        </div>

        {/* Center: editor */}
        <div className="flex-1 flex flex-col min-w-0">
          {activeFile && (
            <div className="px-3 py-1 bg-gray-900 border-b border-gray-800 text-xs text-gray-500 truncate">
              {activeFile}
            </div>
          )}
          <div className="flex-1">
            <Editor
              path={activeFile ?? 'untitled'}
              value={fileContent}
              language={getLanguage(activeFile)}
              onChange={(v) => setFileContent(v ?? '')}
              theme="vs-dark"
              options={{
                fontSize: 13,
                minimap: { enabled: false },
                wordWrap: 'on',
                scrollBeyondLastLine: false,
                renderLineHighlight: 'line',
                fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
              }}
            />
          </div>
          {pendingPatch && (
            <DiffBar
              diff={pendingPatch}
              onAccept={handleAcceptPatch}
              onReject={() => setPendingPatch(null)}
            />
          )}
        </div>

        {/* Right: status + chat */}
        <div className="w-88 border-l border-gray-800 flex flex-col flex-shrink-0" style={{ width: '22rem' }}>
          <ModelControl onReindex={() => workspace && triggerIndex(workspace)} />
          <ChatPanel events={events} onSend={handleSend} isRunning={isRunning} />
        </div>
      </div>
      <StatusBar />
    </div>
  );
}

function getLanguage(file: string | null): string {
  if (!file) return 'plaintext';
  const ext = file.split('.').pop() ?? '';
  const map: Record<string, string> = {
    ts: 'typescript', tsx: 'typescript',
    js: 'javascript', jsx: 'javascript',
    rs: 'rust', py: 'python', go: 'go',
    cpp: 'cpp', c: 'c', cs: 'csharp',
    json: 'json', md: 'markdown', toml: 'toml',
    yaml: 'yaml', yml: 'yaml', sh: 'shell',
  };
  return map[ext] ?? 'plaintext';
}
