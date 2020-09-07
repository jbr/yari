import React from "react";
import Server from "Server";
import { LogEntry } from "RaftState";

let colorClasses = [
  "bg-primary",
  "bg-secondary",
  "bg-success",
  "bg-danger",
  "bg-warning",
  "bg-info",
  "bg-dark",
  "bg-white"
];

function colorForTerm(term: number) {
  return colorClasses[term % colorClasses.length];
}

function colorForLog(logEntry: LogEntry) {
  return colorForTerm(logEntry.term);
}

function LogEntryComponent({ logEntry }: { logEntry: LogEntry }) {
  let message = logEntry.message;
  switch (message.type) {
    case "ServerConfigChange":
      return (
        <li className={`list-group-item ${colorForLog(logEntry)}`}>
          {logEntry.index}{" "}
          {(message.current || []).map(m => (
            <span className="badge badge-light mx-1" key={m}>
              {m}
            </span>
          ))}
          &rarr;
          {(message.new || []).map(m => (
            <span className="badge badge-light mx-1" key={m}>
              {m}
            </span>
          ))}
        </li>
      );
    case "Blank":
      return null;
    case "StateMachineMessage":
      return (
        <li className={`list-group-item ${colorForLog(logEntry)}`}>
          {logEntry.index}
        </li>
      );
  }
}

function Log({ server }: { server: Server }) {
  let raftState = server.stateHistory.slice(-1)[0];
  return (
    <ol className="list-group">
      {raftState.log.entries.map(entry => (
        <LogEntryComponent
          logEntry={entry}
          key={`${entry.term} ${entry.index}`}
        />
      ))}
    </ol>
  );
}

export default function RaftServer({ server }: { server: Server }) {
  return (
    <div className="card m-1" key={server.id}>
      <div className="card-body">
        <h5 className="card-title">{server.id}</h5>
        <Log server={server} />
        <ol className="list-group log">
          {server.eventLog.map((l, i) => (
            <li
              className={`list-group-item ${
                l.message.startsWith("waiting") ? "waiting" : ""
                }`}
              style={{
                ["--time" as any]: l.message.replace(/^waiting /, "")
              }}
              key={i}
            >
              <span className="badge">{l.index}</span>
              {l.message}
            </li>
          ))}
        </ol>
      </div>
    </div>
  );
}
