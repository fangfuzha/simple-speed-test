# simple-speed-test（已归档）

本项目已完成 Rust 化迁移并进入归档维护状态。

## 当前范围

- Rust 后端：axum + tokio
- 桌面托盘模式：tray-icon + winit
- Web 界面：public 目录下的静态文件
- 容器化部署：Dockerfile + docker-compose

## 运行方式

- 服务端模式：cargo run --bin speedtest-server
- 桌面端模式：cargo run --bin speedtest-desktop

## 说明

- 本仓库已移除 Node.js 运行时残留（`server.js`、`package*.json`、`node_modules`）。
- 设计说明见 DESIGN.md。
