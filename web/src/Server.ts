import RaftState from "RaftState";
import Client from "Client";
import lodash from "lodash";

export const LOG_RETAIN = 50;

function JSONSame(x: any, y: any): boolean {
  return JSON.stringify(x) === JSON.stringify(y);
}

export default class Server {
  stateHistory: RaftState[] = [];
  eventSource: EventSource;
  client: Client;
  eventLog: { index: number; message: string }[] = [];
  id: string;
  logIndex = 0;

  clone(): Server {
    let s = new Server(this.id, this.client, this.eventSource);
    s.stateHistory = lodash.cloneDeep(this.stateHistory);
    s.eventLog = lodash.cloneDeep(this.eventLog);
    s.logIndex = this.logIndex;
    return s;
  }

  addToHistory(r: RaftState): boolean {
    if (JSONSame(this.currentState(), r)) {
      return false;
    } else {
      this.stateHistory.push(r);
      return true;
    }
  }

  buildEventSource(url: string) {
    let es = new window.EventSource(url);

    es.addEventListener("state", event => {
      let raftState = JSON.parse(((event as unknown) as MessageEvent).data);

      if (this.addToHistory(raftState)) {
        this.client.callback(raftState);
      }
    });

    es.addEventListener("log", event => {
      let data = ((event as unknown) as MessageEvent).data as string;
      this.eventLog.unshift({ message: data, index: ++this.logIndex });
      while (this.eventLog.length > LOG_RETAIN) this.eventLog.pop();
      this.client.changed();
    });

    return es;
  }

  constructor(id: string, client: Client, eventSource?: EventSource) {
    this.id = id;
    this.client = client;
    this.eventSource =
      eventSource ?? this.buildEventSource(`${id.replace(/\/$/, "")}/sse`);
  }

  currentState(): RaftState | undefined {
    return this.stateHistory[this.stateHistory.length - 1];
  }
}
