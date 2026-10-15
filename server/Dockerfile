# One image, two stateless processes: `relay` (iroh's own iroh-relay, unmodified) and
# `push-gateway`. The container's command picks which one runs.
#
#   docker build -t smetana-server server/

FROM rust:1.97-bookworm AS build
WORKDIR /src

# The relay is the published crate at an exact version, from the lockfile its authors ship.
ARG IROH_RELAY_VERSION=1.3.0
RUN cargo install iroh-relay --version "=${IROH_RELAY_VERSION}" --locked \
        --features server --root /out

# The gateway is this repository's own code, built from its own lockfile.
COPY push-gateway/Cargo.toml push-gateway/Cargo.lock push-gateway/
COPY push-gateway/src push-gateway/src
RUN cd push-gateway && cargo build --release --locked \
    && cp target/release/push-gateway /out/bin/push-gateway

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home smetana \
    && mkdir -p /var/lib/relay-certs && chown smetana /var/lib/relay-certs
COPY --from=build /out/bin/iroh-relay /out/bin/push-gateway /usr/local/bin/
COPY relay/entrypoint.sh /usr/local/bin/relay-entrypoint
RUN chmod 0755 /usr/local/bin/relay-entrypoint
USER smetana
# With Let's Encrypt the relay keeps its ACME account and issued certificate in RELAY_CERT_DIR,
# the one thing that survives a restart. It is the relay's own, not anybody's content; see the
# README.
ENV RELAY_CERT_DIR=/var/lib/relay-certs
CMD ["relay-entrypoint"]
