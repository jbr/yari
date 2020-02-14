# Yari &mdash; Yet Another Raft Implementation

> Yari (槍) is the term for one of the traditionally made Japanese
> blades (nihonto) in the form of a spear, or more specifically, the
> straight-headed spear. The martial art of wielding the yari is
> called sōjutsu.

This project is very much a learning experience, and one of my
self-imposed constraints is not looking at any RAFT reference except
the [paper](https://raft.github.io/raft.pdf). This is also the largest
chunk of rust I've written, and I'm learning rust through this process.

Known issues and plan:
1. Because of the current approach to concurrency, there's no way for
   the client http response to wait for commit. Currently it reports
   back a stale value.
2. ~~The server list is hardcoded in a config.toml, which means the only
   way to change membership is to restart all of the servers.~~
3. ~~The state machine is hardcoded, and should be configurable at
   compile time. Most of the code should become generic over something
   that implements the state machine trait and has an associated
   Message type. The code to create a new binary with a custom state
   machine should be as simple as passing it to RaftState::new.~~
4. ~~Save files aren't versioned.~~
5. Snapshots aren't implemented yet.
6. Save files are JSON instead of
   [bincode](https://docs.rs/bincode/1.2.1/bincode/), and I need to
   think through whether it's safe to save them asynchronously.
7. Error handling should improve.
8. ~~Currently server ids are SocketAddrs but there's no reason other
   urls wouldn't work~~
9. Currently the assumption is that this only will exist on a closed
   network. At the very least, https could be supported.
10. Look into using http2 in order to save on reconnection overhead.
