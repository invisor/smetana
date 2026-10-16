# Smetana server

Two small processes for the mobile companion, in one Docker image:

| process | what it is | who runs it |
|---|---|---|
| `relay` | iroh's own `iroh-relay`, unmodified, plus a generated config file | anybody |
| `push-gateway` | one route, `POST /v1/push`, that forwards an encrypted blob to APNs or FCM | the app's owner only |

Neither stores anything. Everything that passes through either is encrypted end to end between
the desktop and the phone; the servers only move bytes they cannot read.

## What each server can see

Stated plainly, because silence about it is what would be held against us.

**Nobody on the server side can see any content**: not the board, code, agent output,
conversations or the text of a push. The desktop and the phone encrypt to each other's keys;
the servers hold none of them.

**The relay sees:**

- IP addresses of the desktop and the phone that connect to it;
- the public keys of those devices (their iroh endpoint ids);
- the time of each connection, and the volume of traffic.

**The push gateway sees:**

- the device token the phone registered with APNs or FCM;
- the time of each push;
- the size of the encrypted blob.

It passes the blob to Apple or Google exactly as received. It has no key that could open it.
(The connecting IP is used in memory for rate limiting and never written anywhere.)

**Apple and Google see** that a notification was delivered to the device, as they do for every
push.

**Nothing is stored.** There is no database, and neither process writes request data anywhere.
Said precisely, because "no logs" is a claim that has to be true to be made:

- **The relay logs nothing, by configuration.** `iroh-relay` tags its log lines with the peer's
  address (`conn{peer=IP:port}`) and prints one at its default level for every failed handshake,
  which Docker would keep on the host's disk. `docker-compose.yml` therefore sets
  `RUST_LOG=off` for it (`RELAY_LOG` overrides this for debugging; if you turn it on, those
  lines hold client addresses and you should clear the container's logs afterwards). Any
  container output is also capped at 1 MB by the logging options in the compose file.
- **The push gateway logs a status and a platform per request** (`push status=200
  platform="ios"`) and one line at startup saying which platforms have credentials. It never
  logs a token, a blob, an address, or the text of a provider's error. A test in
  `push-gateway/tests/log.rs` fails if that changes.
- **The rate limiter is a table in memory**, keyed by client address (IPv6 by its /64) and
  bounded in size; it is gone when the process stops.
- **The one file that survives a restart** is the relay's own Let's Encrypt account and
  certificate, in a named volume, when you ask the relay to obtain a certificate itself.

## Run your own relay

You need a machine with a public address, a DNS name pointing at it, and Docker. A bare IP is
not supported: the phone and the desktop address a relay by an `https://` URL with a real
certificate.

```sh
cd server
cp .env.example .env
$EDITOR .env        # RELAY_HOSTNAME, RELAY_CONTACT
docker compose up -d relay
```

Open TCP 80 and 443 and UDP 7842 (QUIC address discovery) on the machine. With the default
`RELAY_CERT_MODE=letsencrypt` the relay obtains and renews its own certificate for
`RELAY_HOSTNAME`.

If you already have a certificate, put it in `server/certs/` as `relay.crt` and `relay.key`
and set `RELAY_CERT_MODE=manual`.

Then, in Smetana's settings on the desktop, set the relay URL to `https://<RELAY_HOSTNAME>`.
The URL travels in the pairing code, so a phone paired afterwards uses the same relay.

| variable | default | meaning |
|---|---|---|
| `RELAY_CERT_MODE` | `letsencrypt` | `letsencrypt`, `manual`, or `proxy` (see "Our deployment") |
| `RELAY_HOSTNAME` | | name on the certificate (required for `letsencrypt`) |
| `RELAY_CONTACT` | | email for Let's Encrypt (required for `letsencrypt`) |
| `RELAY_LETSENCRYPT_PROD` | `true` | `false` uses Let's Encrypt staging |
| `RELAY_HTTP_PORT` | `80` | HTTP port |
| `RELAY_HTTPS_PORT` | `443` | HTTPS port |
| `RELAY_QUIC_PORT` | `7842` | QUIC address discovery (UDP) |
| `RELAY_LOG` | `off` | the relay's log filter; see above before changing it |

The relay is the unmodified `iroh-relay` crate at an exact version (`IROH_RELAY_VERSION` in the
`Dockerfile`), installed with `--locked`. The container's entrypoint
(`relay/entrypoint.sh`) only turns the variables above into the TOML file that binary reads.

## The push gateway

`POST /v1/push` with a JSON body of at most 4 KiB:

```json
{ "platform": "ios", "token": "<apns device token, hex>", "blob": "<base64>" }
```

`platform` is `ios` or `android`. The blob goes into the notification's data payload as it is.
For iOS the notification also carries `mutable-content: 1` and a neutral alert ("Smetana"), so
the phone's Notification Service Extension can decrypt the blob and replace the text.

| status | meaning |
|---|---|
| 200 | handed to the provider |
| 400 | unknown platform, malformed token, blob not base64 or over 3500 characters, bad JSON, or body over 4 KiB |
| 408 | the body did not arrive within 5 seconds |
| 410 | the provider says the token is no longer valid |
| 429 | too many requests from this address |
| 502 | the provider could not be reached or refused |
| 503 | no credentials configured for that platform |

Only the owner of the app's APNs key and FCM service account can wake the app, which is a rule
of Apple's and Google's, so this part cannot be self-hosted usefully. The same arrangement is
used by Matrix and ntfy.

| variable | meaning |
|---|---|
| `PUSH_BIND` | listen address, `0.0.0.0:8080` |
| `PUSH_RATE_LIMIT_PER_MINUTE` | per client address, default 30 |
| `PUSH_TRUST_FORWARDED_FOR` | `1` only behind a reverse proxy that appends the real client to `X-Forwarded-For`; otherwise the header is ignored |
| `APNS_KEY_PATH`, `APNS_KEY_ID`, `APNS_TEAM_ID`, `APNS_BUNDLE_ID`, `APNS_SANDBOX` | APNs token authentication (`.p8` key) |
| `FCM_SERVICE_ACCOUNT_PATH` | FCM HTTP v1 service account JSON |

The gateway speaks plain HTTP and compose publishes it on `127.0.0.1` only; put a TLS-terminating
reverse proxy in front of it when you expose it, and set `PUSH_TRUST_FORWARDED_FOR=1` so the rate
limit sees real clients.
A platform with no credentials answers 503, and the process still starts.

## Our deployment

`relay.smetana.io` and `push.smetana.io` — the desktop's built-in defaults — run from this
directory on one machine, with Caddy in front of both. `docker-compose.proxy.yml` is laid over
`docker-compose.yml` for that:

| service | listens | does |
|---|---|---|
| `caddy` | TCP 80, 443 | TLS for every name, certificates from Let's Encrypt, renewed by Caddy itself |
| `relay` | UDP 7842 only | `RELAY_CERT_MODE=proxy`: plain HTTP to Caddy on the compose network |
| `push-gateway` | 127.0.0.1:8080 | behind Caddy, with `PUSH_TRUST_FORWARDED_FOR=1` |

`SITE_HOSTNAME` (`smetana.io`) is reserved for the project's site; until there is one, Caddy
answers it with a single line.

**The `proxy` mode.** The relay drops its own HTTPS and serves plain HTTP on port 3340 inside the
compose network, and Caddy forwards `RELAY_HOSTNAME` to it, upgrades included. QUIC address
discovery cannot pass through an HTTP proxy, so the relay still answers it on UDP 7842 itself,
using Caddy's certificate for its own name: Caddy's data volume is mounted into the relay
read-only, the relay re-reads the certificate as Caddy renews it, and on a first start it waits
until Caddy has obtained one. That is why Caddy runs as the relay's uid (`caddy/Dockerfile`):
Caddy writes its keys readable by their owner only. Nothing about the invariants above changes:
Caddy keeps no access log, and its data volume holds certificates and the ACME account, nothing
from a client.

Bringing it up, on Ubuntu with Docker from Docker's own repository:

```sh
# on the machine, in a copy of this directory
cp .env.example .env
$EDITOR .env        # RELAY_HOSTNAME, and the commented proxy block
docker compose up -d --build
```

With `COMPOSE_FILE` set in `.env`, plain `docker compose` uses both files. Open TCP 80 and 443 and
UDP 7842; every service has `restart: unless-stopped`, so the stack comes back after a reboot.

**Uploading the gateway's keys** is a script run from the workstation, never by hand:

```sh
server/upload-secrets.sh ~/Desktop/Projects/smetana-secrets smetana-vps [remote-dir]
```

It reads `apns.p8`, `apns.txt` (Key ID on the first line, Team ID on the second) and
`fcm-service-account.json` from the folder — any platform whose files are missing is skipped —
copies the keys into `secrets/` on the machine through ssh's stdin, owned by the gateway's uid and
readable by it alone, sets `APNS_KEY_PATH`, `APNS_KEY_ID`, `APNS_TEAM_ID`, `APNS_BUNDLE_ID` and
`FCM_SERVICE_ACCOUNT_PATH` in the machine's `.env`, and recreates the gateway so it reads them.
It prints the names of what it set and never a value. `remote-dir` is this directory on the
machine, `smetana/server` under the login's home by default.

## Build and test

The server is built separately from the desktop app: `src-tauri/` and the front end's tooling
never see this directory.

```sh
cd server/push-gateway && cargo test     # providers are stand-ins; no network
docker build -t smetana-server server/
docker compose -f server/docker-compose.yml up
```

To smoke-test the stack without a domain, put a self-signed `relay.crt` and `relay.key` in
`server/certs/` and run with `RELAY_CERT_MODE=manual RELAY_HOSTNAME=localhost RELAY_HTTP_PORT=3340
RELAY_HTTPS_PORT=8443 docker compose up`. `curl -k https://localhost:8443/` answers from the relay.
