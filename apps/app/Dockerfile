FROM rust:1.94 AS builder
WORKDIR /src
# Release version reported by GET /api/v1/version; compiled into the binary.
ARG SERVICE_VERSION=unknown
ENV SERVICE_VERSION=$SERVICE_VERSION
COPY . .
RUN cargo build --release -p donka-app

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /src/target/release/donka-app /usr/local/bin/donka-app
EXPOSE 8080
ENV DONKA_LISTEN=0.0.0.0:8080
CMD ["donka-app"]
