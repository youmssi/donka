# Web app: static export (apps/web/out), served by the backend on the same origin.
FROM node:22-slim AS web
WORKDIR /src/apps/web
RUN corepack enable
COPY apps/web/package.json apps/web/pnpm-lock.yaml apps/web/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY apps/web/ ./
RUN pnpm build

FROM rust:1.94 AS builder
WORKDIR /src
# Release version reported by GET /api/v1/version; compiled into the binary.
ARG SERVICE_VERSION=unknown
ENV SERVICE_VERSION=$SERVICE_VERSION
COPY . .
RUN cargo build --release -p donka-app

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /src/target/release/donka-app /usr/local/bin/donka-app
COPY --from=web /src/apps/web/out /srv/web
EXPOSE 8080
ENV DONKA_LISTEN=0.0.0.0:8080
ENV DONKA_WEB_DIR=/srv/web
CMD ["donka-app"]
