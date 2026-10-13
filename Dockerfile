# The API and the command-line tools in one image.
#   docker build -t music-rec .
#   docker run -p 3000:3000 -e MUSIC_LENIENT=true music-rec
# On the first start the entrypoint loads sample/sample.csv (or MUSIC_CSV)
# into data/music.db; mount a volume on /app/data to keep the database.

FROM rust:1-slim-bookworm AS build
# the build script of utoipa-swagger-ui downloads the Swagger UI files with curl
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked \
    && cp target/release/music-recommendation-system /usr/local/bin/

FROM debian:bookworm-slim
RUN useradd --create-home --uid 10001 music
WORKDIR /app
COPY --from=build /usr/local/bin/music-recommendation-system /usr/local/bin/
COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
COPY data/sample.csv sample/sample.csv
RUN chmod +x /usr/local/bin/entrypoint.sh \
    && mkdir -p data logs \
    && chown -R music:music /app
USER music

# relative paths only: the SQLite URL cannot take an absolute Windows-style path
ENV MUSIC_ADDR=0.0.0.0:3000 \
    MUSIC_DB=data/music.db \
    MUSIC_CSV=sample/sample.csv
EXPOSE 3000
VOLUME ["/app/data"]
ENTRYPOINT ["entrypoint.sh"]
CMD ["serve"]
