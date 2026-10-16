#!/bin/sh
# Copies the push gateway's credentials from a folder on this machine to the server, writes the
# matching variables into the server's .env, and restarts the gateway.
#
#   server/upload-secrets.sh <folder> <ssh-host> [remote-dir]
#
# <folder> holds any of:
#   apns.p8                    the APNs token key
#   apns.txt                   two lines: the key's Key ID, then the Team ID
#   fcm-service-account.json   the FCM HTTP v1 service account
# A platform whose files are missing is left as it is on the server. [remote-dir] is the server/
# directory on the machine, `smetana/server` under the login's home by default.
#
# Nothing read from <folder> is printed, and nothing here writes it anywhere but the server.
set -eu

usage() { echo "usage: $0 <folder> <ssh-host> [remote-dir]" >&2; exit 2; }
[ $# -ge 2 ] && [ $# -le 3 ] || usage
src=$1
host=$2
dir=${3:-smetana/server}
[ -d "$src" ] || { echo "no such folder: $src" >&2; exit 1; }

# The bundle id the iOS app is published under; the APNs topic has to match it.
bundle_id=com.invisor.smetana.companion
# The uid the gateway runs as (`smetana` in Dockerfile); it reads the keys through a read-only mount.
uid=10001

env_lines=""
files=""

if [ -f "$src/apns.p8" ] || [ -f "$src/apns.txt" ]; then
  [ -f "$src/apns.p8" ] && [ -f "$src/apns.txt" ] \
    || { echo "APNs needs both apns.p8 and apns.txt" >&2; exit 1; }
  key_id=$(sed -n 1p "$src/apns.txt" | tr -d ' \r')
  team_id=$(sed -n 2p "$src/apns.txt" | tr -d ' \r')
  # Both are ten upper-case letters and digits; checking the shape also keeps them safe to splice
  # into the remote command below.
  for value in "$key_id" "$team_id"; do
    printf '%s' "$value" | grep -Eq '^[A-Z0-9]{10}$' \
      || { echo "apns.txt: expected a Key ID and a Team ID of ten letters and digits" >&2; exit 1; }
  done
  files="$files apns.p8"
  env_lines="${env_lines}APNS_KEY_PATH=/secrets/apns.p8
APNS_KEY_ID=$key_id
APNS_TEAM_ID=$team_id
APNS_BUNDLE_ID=$bundle_id
"
fi

if [ -f "$src/fcm-service-account.json" ]; then
  files="$files fcm-service-account.json"
  env_lines="${env_lines}FCM_SERVICE_ACCOUNT_PATH=/secrets/fcm-service-account.json
"
fi

[ -n "$files" ] || { echo "nothing to upload in $src" >&2; exit 1; }

# Root on the machine, or sudo without a password: the files are handed to the gateway's uid.
sudo='if [ "$(id -u)" = 0 ]; then "$@"; else sudo -n "$@"; fi'

for name in $files; do
  # Through stdin, never a command line: the key is not visible in any process list.
  ssh "$host" "set -eu; as_root() { $sudo; }; cd '$dir'; as_root mkdir -p secrets; \
    as_root sh -c 'umask 077; cat > secrets/$name.new'; \
    as_root chown $uid:$uid secrets/$name.new; as_root mv -f secrets/$name.new secrets/$name" \
    < "$src/$name"
  echo "uploaded $name"
done

# Each variable replaces its own line in .env, or is appended when it is not there yet.
printf '%s' "$env_lines" | ssh "$host" "set -eu; cd '$dir'; touch .env; \
  while IFS= read -r line; do \
    key=\${line%%=*}; \
    grep -v \"^\$key=\" .env > .env.new || true; \
    printf '%s\n' \"\$line\" >> .env.new; \
    mv -f .env.new .env; \
  done; chmod 600 .env"
printf '%s' "$env_lines" | sed 's/=.*/ set in .env/'

# Recreated rather than restarted: a restart keeps the environment the container was created with.
ssh "$host" "cd '$dir' && docker compose up -d --force-recreate --no-deps push-gateway"
echo "push-gateway restarted"
