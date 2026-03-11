import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface FileNode {
  name: string;
  path: string;
  is_dir: boolean;
  children?: FileNode[];
}

interface Props {
  workspace: string;
  onFileSelect: (path: string) => void;
  activeFile: string | null;
  refreshKey?: number;
}

const FILE_ICONS: Record<string, string> = {
  rs: '🦀', ts: '🔷', tsx: '🔷', js: '🟨', jsx: '🟨',
  py: '🐍', go: '🐹', md: '📝', json: '{}', toml: '⚙',
  yaml: '⚙', yml: '⚙', sh: '📜', css: '🎨', html: '🌐',
};

function fileIcon(name: string): string {
  const ext = name.split('.').pop() ?? '';
  return FILE_ICONS[ext] ?? '·';
}

export function FileExplorer({ workspace, onFileSelect, activeFile, refreshKey }: Props) {
  const [tree, setTree] = useState<FileNode[]>([]);

  useEffect(() => {
    if (!workspace) { setTree([]); return; }
    invoke<FileNode[]>('list_workspace_files', { root: workspace })
      .then(setTree)
      .catch(console.error);
  }, [workspace, refreshKey]);

  if (!workspace) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-gray-700 text-xs p-4 text-center gap-2">
        <span className="text-2xl">📂</span>
        <span>Open a workspace<br />to see files</span>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto py-1">
      {tree.map(node => (
        <FileNodeView
          key={node.path}
          node={node}
          depth={0}
          onSelect={onFileSelect}
          activeFile={activeFile}
        />
      ))}
    </div>
  );
}

function FileNodeView({
  node, depth, onSelect, activeFile
}: {
  node: FileNode;
  depth: number;
  onSelect: (path: string) => void;
  activeFile: string | null;
}) {
  const [expanded, setExpanded] = useState(depth < 1);
  const isActive = activeFile === node.path;

  return (
    <div>
      <div
        onClick={() => node.is_dir ? setExpanded(!expanded) : onSelect(node.path)}
        className={`flex items-center gap-1 py-0.5 rounded cursor-pointer text-xs truncate transition-colors ${
          isActive
            ? 'bg-blue-900/40 text-blue-300'
            : 'hover:bg-gray-800/60 text-gray-400 hover:text-gray-200'
        }`}
        style={{ paddingLeft: `${depth * 10 + 6}px`, paddingRight: '6px' }}
      >
        <span className="flex-shrink-0 w-3 text-center text-gray-600">
          {node.is_dir ? (expanded ? '▾' : '▸') : ''}
        </span>
        <span className="flex-shrink-0 text-xs">
          {node.is_dir ? (expanded ? '📂' : '📁') : fileIcon(node.name)}
        </span>
        <span className="truncate">{node.name}</span>
      </div>
      {node.is_dir && expanded && node.children?.map(child => (
        <FileNodeView
          key={child.path}
          node={child}
          depth={depth + 1}
          onSelect={onSelect}
          activeFile={activeFile}
        />
      ))}
    </div>
  );
}
