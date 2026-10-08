# Studio image: the backend, serving the web app's static export on the same origin.
#
#   docker build -f apps/app/Dockerfile --build-arg SERVICE_VERSION=1.2.0 -t donka-studio .
#
# Rust dependencies are compiled in their own layer, keyed on the workspace's manifests and
# Cargo.lock only (cargo-chef), so a change to Studio's code rebuilds Studio, not its ~600
# dependencies.

# Web app: static export (apps/web/out).
FROM node:22-slim AS web
WORKDIR /src/apps/web
RUN corepack enable
COPY apps/web/package.json apps/web/pnpm-lock.yaml apps/web/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY apps/web/ ./
RUN pnpm build

FROM lukemathwalker/cargo-chef:0.1.77-rust-1.94 AS chef
WORKDIR /src

# The dependency recipe: what the manifests and Cargo.lock say, nothing from the code.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --locked --package donka-app --recipe-path recipe.json
COPY . .
# Release version reported by GET /api/v1/version; compiled into the binary. Declared after the
# dependency layer so a new version does not rebuild the dependencies.
ARG SERVICE_VERSION=unknown
ENV SERVICE_VERSION=$SERVICE_VERSION
RUN cargo build --release --locked --package donka-app

# No shell, no package manager; runs as the unprivileged `nonroot` user (uid 65532). Same Debian
# release as the build stage (13), so the binary finds the glibc it was linked against.
FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=builder /src/target/release/donka-app /usr/local/bin/donka-app
COPY --from=web /src/apps/web/out /srv/web
USER nonroot:nonroot
EXPOSE 8080
ENV DONKA_LISTEN=0.0.0.0:8080
ENV DONKA_WEB_DIR=/srv/web
CMD ["donka-app"]
