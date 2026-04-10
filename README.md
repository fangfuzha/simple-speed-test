# simple-speed-test (Archived)

本项目已完成 Rust 化迁移并进入归档维护状态。

## Current Scope

- Rust backend: axum + tokio
- Desktop tray mode: tray-icon + winit
- Web UI: static files in `public/`
- Container deployment: Dockerfile + docker-compose

## Run

- Server mode: `cargo run --bin speedtest-server`
- Desktop mode: `cargo run --bin speedtest-desktop`

## Notes

- 本仓库已移除 Node.js 运行时残留（`server.js`、`package*.json`、`node_modules`）。
- 设计说明见 `DESIGN.md`。
