# Installing `smetanad`

`smetanad` is the Smetana server without a window: it keeps the board, the agents, the terminals, the
runs and the git work, and every app — the desktop, the phone — is a client of it. Install it on a
machine you want the agents to live on (a VPS, a spare box), pair your desktop and phone with it, and
close the laptop: the work goes on.

The design is `docs/superpowers/specs/2026-10-25-headless-server-design.md`. Everything below is the
"Remote placement" part of it.

> This is for the **Smetana Next** desktop build. The released desktop app keeps its own server inside
> it and does not connect to this one.

## What you need

- A Linux machine (macOS and Windows archives exist too, without the service unit). A user that is not
  root: `smetanad` refuses to run as root.
- `git` on it, with whatever credentials your repositories need (an ssh key, a credential helper).
  **The server clones and fetches as its own user**; nothing is borrowed from your laptop.
- The agent CLIs you use (`claude`, `codex`, ...) installed **and logged in** as that same user.
  `smetanad status` tells you which are missing.
- Outbound HTTPS. No inbound port is opened: devices find the server through a relay and then talk to
  it directly when they can. Everything between a device and the server is encrypted end to end; the
  relay (by default `relay.smetana.io`, or your own — see `../README.md`) carries ciphertext.

## Install

Each release carries one archive per platform, `smetanad-<version>-<target>.tar.gz` (a `.zip` on
Windows), and a `.sha256` beside it. Check it, unpack it where the service unit expects it, and make
the user:

```sh
sha256sum -c smetanad-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
sudo mkdir -p /opt/smetana
sudo tar -xzf smetanad-0.1.0-x86_64-unknown-linux-gnu.tar.gz -C /opt/smetana --strip-components=1

sudo useradd --system --create-home --home-dir /var/lib/smetana --shell /bin/bash smetana
sudo install -d -o smetana -g smetana /var/lib/smetana/data
```

The archive holds `smetanad`, the `bd` it drives, the skill library (`resources/`) and the unit
(`smetanad.service`). Then:

```sh
sudo cp /opt/smetana/smetanad.service /etc/systemd/system/smetanad.service
sudo systemctl daemon-reload
sudo systemctl enable --now smetanad
sudo journalctl -u smetanad -f          # "serving /var/lib/smetana/data as <key>"
```

The unit runs `smetanad run --data-dir /var/lib/smetana/data --resource-dir /opt/smetana/resources`.
Everything the server keeps is in that data directory (`settings.json`, the device key and list, the
runs, the transcripts, the attachments): back it up, and a new machine with a copy of it is the same
server. It is never guessed — every subcommand takes `--data-dir`.

Without systemd, run the same command in the foreground or under whatever you use; SIGTERM stops it
cleanly.

Log in the agents as the service user, once:

```sh
sudo -iu smetana
claude        # sign in; likewise codex, and `ssh-keygen` / `git config` for git
```

and check the environment the *service* sees (it is not your shell's):

```sh
sudo -u smetana /opt/smetana/smetanad status --data-dir /var/lib/smetana/data
```

## Pair the desktop

On the server, as the service user:

```sh
sudo -u smetana /opt/smetana/smetanad pair --data-dir /var/lib/smetana/data --kind desktop
```

It prints a code as text and as a QR code, and waits. **The code is the way in**: it is good once, for
two minutes, and nobody can pair without it. In the desktop app (Smetana Next): Settings, then Server,
paste the code under "Use a server on another machine" and press "Use this server". The terminal that
ran `pair` asks you to confirm the device; answer `y`. A phone is paired the same way with
`--kind phone` and its camera on the QR code.

Then add a project: the folder dialog is replaced by the server's own folders (under the service
user's home), or "Clone a repository…" with an address the server can reach. The desktop shows what is
in those folders and nothing above them.

If the desktop was using the server on its own machine, that one keeps running and is not started
again while you use the remote one. If it has work in it you are asked whether to leave it or have it
stop once that work has finished.

### Who can do what

The kind a device was paired as is fixed on the server: a **desktop** may call every command; a
**phone** may read the board, follow sessions and files and answer questions, and a terminal is
read-only on it. Anything a phone claims about itself is ignored. Revoke a device and its connection
is closed at once:

```sh
sudo -u smetana /opt/smetana/smetanad devices --data-dir /var/lib/smetana/data
sudo -u smetana /opt/smetana/smetanad revoke  --data-dir /var/lib/smetana/data <id>
```

### What a desktop can read from the server

Besides the project's files through the editor, a client can ask for a file by path in exactly two
places: the attachment store (a dropped screenshot, a picture an agent shows) and each project's
`.smetana/reports/` folder (a run's report, which the desktop opens in your own browser). A path
anywhere else — a `..`, a symlink pointing out, an absolute path elsewhere — is refused. The folder
browser lists directories under the service user's home and nothing above it.

## Update

The server is a binary you replace. Stop it, unpack the new archive over `/opt/smetana`, start it:

```sh
sudo systemctl stop smetanad
sudo tar -xzf smetanad-<new>.tar.gz -C /opt/smetana --strip-components=1
sudo systemctl start smetanad
```

A stop ends every agent, run and shell the server owns, so do it when none is running — or ask the
server to restart itself once the work is done, from a machine that has the data directory:

```sh
sudo -u smetana /opt/smetana/smetanad restart --data-dir /var/lib/smetana/data --when-idle
```

A desktop whose protocol major differs from the server's says which side to update and attempts
nothing else.

## Build from source

```sh
cargo build --release --manifest-path crates/smetanad/Cargo.toml
```

The result is `crates/smetanad/target/release/smetanad`. The `bd` it drives is fetched by
`npm run fetch-bd` into `src-tauri/binaries/`; put a copy beside the executable, or pass `--bd`.
