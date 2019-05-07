# Roadmap

## Library - clique

- [ ] Transport
  - [ ] UDP Framed
  - [ ] gRPC TCP/h2
- [ ] SWIM
  - [ ] Messages
	  - [ ] Ping
	  - [ ] Ack
	  - [ ] PingReq
	  - [ ] PingReqAck (better name for this?)
  - [ ] Broadcasts
	- [ ] Join
	- [ ] Leave
	- [ ] Alive
	- [ ] Suspect
	- [ ] Dead
  - [ ] Piggybacking UDP Gossip messages
  - [ ] Gossip
  	- [ ] Gossip on configured interval
	- [ ] Pick random node to probe
	- [ ] Detect failure and select `k` nodes to `PingReq`
  - [ ] RPC
	- [ ] Join Push/Pull
	- [ ] Occasional Push/Pull requests
	- [ ] Client RPC
		- [ ] Join
		- [ ] Members
		- [ ] Leave
		- [ ] Research more
- [ ] Public API
  - [ ] Join
  - [ ] Serve UDP/TCP
  - [ ] Event channel
  - [ ] Peers snapshot
- [ ] Custom Event Dissemination
- [ ] Node metadata
  - [ ] Binary blob data `Vec<u8>`
  - [ ] Tags

## CLI - clique-agent

- [ ] Clap for CLI
- [ ] Commands
  - [ ] Run
  - [ ] Members/Peers list
  - [ ] Leave
  - [ ] Join
- [ ] RPC
  - [ ] Join
  - [ ] Members
  - [ ] Leave
