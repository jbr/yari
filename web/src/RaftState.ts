export interface LogEntry {
  index: number;
  term: number;
  message: ServerConfigChangeMessage | BlankMessage | StateMachineMessage;
}

export interface StateMachineMessage {
  type: "StateMachineMessage";
}

export interface ServerConfigChangeMessage {
  type: "ServerConfigChange";
  current: string[];
  new: string[];
}

export interface BlankMessage {
  type: "Blank";
}

export interface RaftState {
  id: string;
  log: {
    entries: LogEntry[];
  };
  current_term: number;
  voted_for: null | string;

  follower_state: {
    [id: string]: {
      identifier: string;
      next_inde: number;
      match_index: number;
    };
  };
  servers: {
    set: string[];
    new_config: null;
  };
}
export default RaftState;
