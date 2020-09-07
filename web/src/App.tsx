import React from "react";
import "./App.css";
import Client from "Client";
import Server from "Server";
import RaftServer from "RaftServer";

function useStuff(initialServer: string) {
  let client = React.useRef<null | Client>(null);
  let [state, setState] = React.useState<Server[] | null>(null);

  React.useEffect(() => {
    client.current = new Client(initialServer, client => {
      console.log(client);
      setState(Object.values(client.servers).map(x => x.clone()));
    });
  }, [setState, initialServer]);

  return state;
}

function App() {
  let initialServer = "http://localhost:8000/";
  let state = useStuff(initialServer);

  if (state) {
    return (
      <div className="App">
        <div className="d-flex">
          {state.map(s => (
            <RaftServer server={s} key={s.id} />
          ))}
        </div>
      </div>
    );
  } else {
    return (
      <div className="App">
        <div className="card m-1">
          <div className="card-body">
            <h5 className="card-title">
              server {initialServer} not responding
            </h5>
          </div>
        </div>
      </div>
    );
  }
}

export default App;
