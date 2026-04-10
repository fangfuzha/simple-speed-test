# -------- Stage 1: build binary --------
# 使用官方 Rust 镜像在构建阶段编译服务端可执行文件。
FROM rust:1.94-slim AS builder
WORKDIR /app

# 先复制依赖声明并预下载 crates，避免源码变更时重复拉取依赖。
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
RUN cargo fetch --locked

# 再复制源码和静态前端资源，源码变更只会影响最终编译层。
COPY src src
COPY public public

# 仅构建服务端二进制，产出 release 版本用于生产镜像。
RUN cargo build --locked --release --bin speedtest-server

# -------- Stage 2: runtime image --------
# 运行阶段使用更小的 Debian slim，减小镜像体积并降低攻击面。
FROM debian:bookworm-slim
WORKDIR /app

# 仅安装运行所需的 CA 证书，用于 HTTPS 请求等场景。
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# 从构建阶段复制最终可执行文件到运行镜像。
COPY --from=builder /app/target/release/speedtest-server /usr/local/bin/speedtest-server

# 对外暴露服务端口。
EXPOSE 3000

# 提供默认运行参数，可通过 docker run / compose 环境变量覆盖。
ENV SPEEDTEST_BIND=0.0.0.0:3000 \
    SPEEDTEST_MODE=server \
    SPEEDTEST_OPEN_BROWSER=false \
    SPEEDTEST_AUTOSTART=false

# 容器启动后默认运行测速服务端。
CMD ["speedtest-server"]
