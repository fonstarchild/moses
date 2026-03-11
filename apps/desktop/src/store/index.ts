import { create } from 'zustand';

export interface AgentEvent {
  type: string;
  [key: string]: any;
}

// Streaming tokens get collapsed into a single "stream" bubble in the chat.
// We track a current streaming buffer separately so the chat doesn't create
// one DOM node per token.
interface AgentStore {
  events: AgentEvent[];
  streamBuffer: string;       // accumulates StreamToken content
  pendingPatch: string | null;
  isRunning: boolean;
  workspace: string;
  indexedChunks: number;
  isIndexing: boolean;
  addEvent: (event: AgentEvent) => void;
  appendToken: (token: string) => void;
  flushStream: () => void;
  setPendingPatch: (patch: string | null) => void;
  setRunning: (v: boolean) => void;
  setWorkspace: (w: string) => void;
  setIndexedChunks: (n: number | ((prev: number) => number)) => void;
  setIndexing: (v: boolean) => void;
  clearEvents: () => void;
}

export const useAgentStore = create<AgentStore>((set, get) => ({
  events: [],
  streamBuffer: '',
  pendingPatch: null,
  isRunning: false,
  workspace: '',
  indexedChunks: 0,
  isIndexing: false,

  addEvent: (event) => set((s) => ({ events: [...s.events, event] })),

  appendToken: (token) => set((s) => ({ streamBuffer: s.streamBuffer + token })),

  flushStream: () => {
    const { streamBuffer } = get();
    if (!streamBuffer.trim()) {
      set({ streamBuffer: '' });
      return;
    }
    set((s) => ({
      events: [...s.events, { type: 'Stream', content: s.streamBuffer }],
      streamBuffer: '',
    }));
  },

  setPendingPatch: (patch) => set({ pendingPatch: patch }),
  setRunning: (v) => set({ isRunning: v }),
  setWorkspace: (w) => set({ workspace: w }),
  setIndexedChunks: (n) => set((s) => ({
    indexedChunks: typeof n === 'function' ? n(s.indexedChunks) : n,
  })),
  setIndexing: (v) => set({ isIndexing: v }),
  clearEvents: () => set({ events: [], streamBuffer: '' }),
}));
