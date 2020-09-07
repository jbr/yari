import RaftState from "RaftState";
import Server from "Server";

export default class Client {
  servers: { [id: string]: Server } = {};
  changed: () => void;

  add(id: string) {
    if (!(id in this.servers)) {
      this.servers[id] = new Server(id, this);
    }
  }

  callback(r: RaftState) {
    r.servers.set.forEach(id => {
      this.add(id);
    });
    this.changed();
  }

  currentStates(): RaftState[] {
    return Object.values(this.servers)
      .map(s => s.currentState())
      .filter((x): x is RaftState => !!x);
  }

  constructor(id: string, changed: (client: Client) => void) {
    this.changed = () => changed(this);
    this.add(id);
  }
}
