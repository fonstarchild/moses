import WebSocket from 'ws';

type Handler = (msg: unknown) => void;

export class BridgeClient {
  private ws: WebSocket | null = null;
  private handlers: Handler[] = [];
  private timer?: ReturnType<typeof setTimeout>;

  constructor(private url: string) {}

  connect() {
    try {
      this.ws = new WebSocket(this.url);
      this.ws.on('open', () => {
        if (this.timer) clearTimeout(this.timer);
      });
      this.ws.on('message', (data) => {
        try {
          const msg = JSON.parse(data.toString());
          this.handlers.forEach(h => h(msg));
        } catch {}
      });
      this.ws.on('close', () => this.reconnect());
      this.ws.on('error', () => this.reconnect());
    } catch {
      this.reconnect();
    }
  }

  send(msg: object) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  onMessage(h: Handler) { this.handlers.push(h); }

  disconnect() { this.ws?.close(); }

  private reconnect() {
    this.timer = setTimeout(() => this.connect(), 3000);
  }
}
