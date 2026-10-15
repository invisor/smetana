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

**Nothing is stored.** There is no database and no log of requests. The push gateway's log lines
hold a status and a platform and nothing else: no token, no blob, no address, and no text from a
provider's error (which could contain the token). A test in `push-gateway/tests/log.rs` fails if
that changes. The rate limiter is a table in memory that is gone when the process stops.
The one file that survives a restart is the relay's own Let's Encrypt certificate and account,
in a named volume, when you ask the relay to obtain a certificate itself.

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
| `RELAY_CERT_MODE` | `letsencrypt` | `letsencrypt` or `manual` |
| `RELAY_HOSTNAME` | | name on the certificate (required for `letsencrypt`) |
| `RELAY_CONTACT` | | email for Let's Encrypt (required for `letsencrypt`) |
| `RELAY_LETSENCRYPT_PROD` | `true` | `false` uses Let's Encrypt staging |
| `RELAY_HTTP_PORT` | `3340` in compose, `80` in `.env.example` | HTTP port |
| `RELAY_HTTPS_PORT` | `8443` in compose, `443` in `.env.example` | HTTPS port |
| `RELAY_QUIC_PORT` | `7842` | QUIC address discovery (UDP) |

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
| 400 | unknown platform, malformed token, blob not base64, bad JSON, or body over 4 KiB |
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

The gateway speaks plain HTTP; put TLS in front of it (a reverse proxy) when you expose it.
A platform with no credentials answers 503, and the process still starts.

## Build and test

The server is built separately from the desktop app: `src-tauri/` and the front end's tooling
never see this directory.

```sh
cd server/push-gateway && cargo test     # providers are stand-ins; no network
docker build -t smetana-server server/
docker compose -f server/docker-compose.yml up
```
