FROM rust:1.70

WORKDIR /app
COPY . .

RUN cargo build --release
ENV BIND_ADDRESS=0.0.0.0
ENV BIND_PORT=443

CMD ["cargo", "run", "--release"]
EXPOSE 443
