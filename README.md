# simple-speed-test

本项目已完成 Rust 化迁移并进入归档维护状态。

## 当前范围

- Rust 后端：axum + tokio
- 桌面托盘模式：tray-icon + winit
- Web 界面：public 目录下的静态文件
- 容器化部署：Dockerfile + docker-compose

## 运行方式

- 服务端模式：cargo run --bin speedtest-server
- 桌面端模式：cargo run --bin speedtest-desktop

## Docker 使用

- 运行最新镜像：

```bash
docker run --rm -p 3000:3000 ghcr.io/fangfuzha/simple-speed-test:latest
```

- 使用 Compose（请在项目根目录、即包含 `docker-compose.yml` 的目录下运行）：

```bash
docker compose up
```

- 本仓库 Docker 镜像默认运行的是服务端版本 `speedtest-server`，并监听 `[::]:3000`；在支持 dual-stack 的操作系统上可同时接受 IPv4 和 IPv6 访问。
- 如果需要自定义参数，参考 `docker-compose.yml` 中的 `SPEEDTEST_*` 环境变量，例如：

```bash
docker run --rm -p 3000:3000 \
  -e SPEEDTEST_MODE=server \
  -e SPEEDTEST_OPEN_BROWSER=false \
  ghcr.io/fangfuzha/simple-speed-test:latest
```

- 镜像构建支持多架构平台：`linux/amd64` 和 `linux/arm64`。

## 说明

- 设计说明见 DESIGN.md。
