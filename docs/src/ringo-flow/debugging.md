# Debugging

When a scenario misbehaves, two flags give you visibility into what the SIP
backend is doing. Both are off by default and write nowhere unless you ask.

## The backend log

`--log [<file>]` writes the backend's log — registration, call state,
module output — to stderr, or to a file if you pass a path:

```sh
ringo-flow run scenario.rhai --log            # → stderr
ringo-flow run scenario.rhai --log run.log    # → file
```

## SIP tracing

`--sip-trace [<file>]` traces every SIP request and response (sent and
received), to its own destination — **separate** from `--log`, so you can keep
the protocol trace clean:

```sh
ringo-flow run scenario.rhai --sip-trace            # → stderr (text)
ringo-flow run scenario.rhai --sip-trace sip.txt    # → text file
```

Each message is printed as a timestamped block with its direction (`TX →` /
`RX ←`) and transport.

## Tracing into a pcap

Give `--sip-trace` a path ending in `.pcap` and you get a
[libpcap](https://wiki.wireshark.org/Development/LibpcapFileFormat) capture
instead of text — readable by
[sngrep](https://github.com/irontec/sngrep) and
[Wireshark](https://www.wireshark.org/), including Wireshark's *Telephony →
VoIP Calls* flow graph:

```sh
ringo-flow run scenario.rhai --sip-trace flow.pcap
sngrep -I flow.pcap        # or: wireshark flow.pcap
```

This is the only way to inspect the SIP when the transport is **TLS**: a live
sniffer on the wire sees only the encrypted bytes, but ringo-flow taps the trace
*inside* the stack, so the capture holds the plaintext SIP. Each message is framed
as one Ethernet/IP/UDP datagram (the original transport is irrelevant for parsing),
so the tools render a clean ladder regardless of UDP/TCP/TLS.
