#!/bin/sh
# Renders iroh-relay's config file from environment variables and starts it. The relay binary
# itself is iroh's, unmodified; it only reads a TOML file, and this is the glue between that and
# a container's environment.
set -eu

HTTP_PORT="${RELAY_HTTP_PORT:-80}"
HTTPS_PORT="${RELAY_HTTPS_PORT:-443}"
QUIC_PORT="${RELAY_QUIC_PORT:-7842}"
CERT_MODE="${RELAY_CERT_MODE:-letsencrypt}"
CERT_DIR="${RELAY_CERT_DIR:-/var/lib/relay-certs}"
CONFIG="${RELAY_CONFIG_PATH:-/tmp/relay.toml}"

mkdir -p "$CERT_DIR"

{
  echo "http_bind_addr = \"[::]:${HTTP_PORT}\""
  echo "enable_quic_addr_discovery = true"
  echo "enable_metrics = false"
  echo ""
  echo "[tls]"
  echo "https_bind_addr = \"[::]:${HTTPS_PORT}\""
  echo "quic_bind_addr = \"[::]:${QUIC_PORT}\""
  echo "cert_dir = \"${CERT_DIR}\""
  case "$CERT_MODE" in
    letsencrypt)
      : "${RELAY_HOSTNAME:?RELAY_HOSTNAME is required for RELAY_CERT_MODE=letsencrypt}"
      : "${RELAY_CONTACT:?RELAY_CONTACT (an email address) is required for RELAY_CERT_MODE=letsencrypt}"
      echo "cert_mode = \"LetsEncrypt\""
      echo "hostname = \"${RELAY_HOSTNAME}\""
      echo "contact = \"${RELAY_CONTACT}\""
      echo "prod_tls = ${RELAY_LETSENCRYPT_PROD:-true}"
      ;;
    manual)
      : "${RELAY_CERT_FILE:?RELAY_CERT_FILE is required for RELAY_CERT_MODE=manual}"
      : "${RELAY_KEY_FILE:?RELAY_KEY_FILE is required for RELAY_CERT_MODE=manual}"
      echo "cert_mode = \"Manual\""
      echo "hostname = \"${RELAY_HOSTNAME:-localhost}\""
      echo "manual_cert_path = \"${RELAY_CERT_FILE}\""
      echo "manual_key_path = \"${RELAY_KEY_FILE}\""
      ;;
    proxy)
      # Behind a TLS-terminating proxy (docker-compose.proxy.yml): the relay speaks plain HTTP on
      # RELAY_HTTP_PORT and the proxy forwards to it. QUIC address discovery cannot go through an
      # HTTP proxy, so the relay still serves it on RELAY_QUIC_PORT itself, with the proxy's own
      # certificate for the same name, which `Reloading` re-reads as the proxy renews it.
      # `dangerous_http_only` is what iroh-relay's own `--dev` sets to drop HTTPS and keep QUIC;
      # its documentation says not to set it by hand, so it is pinned with IROH_RELAY_VERSION.
      : "${RELAY_CERT_FILE:?RELAY_CERT_FILE is required for RELAY_CERT_MODE=proxy}"
      : "${RELAY_KEY_FILE:?RELAY_KEY_FILE is required for RELAY_CERT_MODE=proxy}"
      echo "cert_mode = \"Reloading\""
      echo "dangerous_http_only = true"
      echo "hostname = \"${RELAY_HOSTNAME:-localhost}\""
      echo "manual_cert_path = \"${RELAY_CERT_FILE}\""
      echo "manual_key_path = \"${RELAY_KEY_FILE}\""
      ;;
    *)
      echo "RELAY_CERT_MODE must be 'letsencrypt', 'manual' or 'proxy', got '${RELAY_CERT_MODE}'" >&2
      exit 2
      ;;
  esac
} > "$CONFIG"

# The proxy obtains the certificate only once it is up, and the relay cannot start without one, so
# on a first start the relay waits for it rather than failing into a restart loop.
if [ "$CERT_MODE" = proxy ]; then
  waited=0
  until [ -r "$RELAY_CERT_FILE" ] && [ -r "$RELAY_KEY_FILE" ]; do
    [ "$waited" = 0 ] && echo "waiting for the proxy's certificate at ${RELAY_CERT_FILE}" >&2
    waited=1
    sleep 5
  done
fi

exec iroh-relay --config-path "$CONFIG"
