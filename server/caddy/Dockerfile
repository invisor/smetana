# The official Caddy image, run as the same uid as the relay (10001, `smetana` in ../Dockerfile).
# The relay reads Caddy's certificate for its own QUIC listener, and Caddy writes its keys 0600, so
# the two processes have to be one user; nothing else in the image changes. The official binary
# carries cap_net_bind_service, which is what lets a non-root Caddy bind 80 and 443.
FROM caddy:2.10-alpine
USER root
RUN chown -R 10001:10001 /data /config
USER 10001
