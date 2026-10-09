# 基于 Rust 异步 TCP 连接池的高性能网关系统

基于 **Rust + Tokio** 自研异步 TCP 连接池，并以此为核心搭建高性能 TCP 网关。
配套 HTTP 管理接口、压测客户端，以及 **Vue3 + Element Plus + ECharts** 可视化监控后台，完整覆盖设计报告中的核心机制。

## 目录结构

```
tcp-pool-gateway/
├── crates/
│   ├── tcp-pool/       # 核心：自研异步 TCP 连接池（库）
│   ├── protocol/       # 共享：长度前缀二进制帧协议（库）
│   ├── gateway/        # 网关：TCP 代理转发 + Axum HTTP 管理接口 + 前端静态托管（二进制）
│   ├── mock-server/    # 模拟后端业务服务（二进制）
│   └── demo-client/    # 压测/演示客户端（二进制）
├── frontend/           # Vue3 + Element Plus + ECharts 可视化后台
└── Cargo.toml          # workspace
```

## 架构

```
 demo-client ──TCP──▶ gateway ──连接池──▶ mock-server
                       │
                       └─HTTP─▶ 管理 API + 可视化后台(指标/趋势/动态配置/日志)
```

- **前端可视化层**：Vue3 + Element Plus + ECharts，由网关同源托管（`tower-http` ServeDir）。
- **网关服务层**：对外监听 TCP 端口接收客户端请求，对内通过连接池转发至后端；
  同时用 Axum 暴露 RESTful 管理接口。
- **连接池核心层**：纯手写，不依赖任何第三方连接池库。
- **后端业务服务层**：模拟业务（大写回显 + 可选延迟）。

## 二进制协议

```
+----------------+-----------------------+
| u32 BE 长度     |        载荷 payload    |
+----------------+-----------------------+
```

- 帧载荷上限 1 MiB。
- `mock-server` 收到载荷后将其 **ASCII 大写** 后原样长度前缀返回。

## 连接池核心机制（tcp-pool）

| 机制 | 实现 |
| --- | --- |
| 连接复用 | LIFO 栈，优先复用最新空闲连接 |
| 健康检查 | 取用时 `try_read` 非阻塞探测：`0 字节`=对端关闭→销毁，`WouldBlock`=正常，`IO 错误`→销毁 |
| 空闲淘汰 | 取用时惰性淘汰 + 后台 reaper 每 1s 扫描淘汰 |
| 最大限流 | `max_connections` 原子变量，超出则等待 |
| 获取超时 | `acquire_timeout` + `Notify` 通知唤醒，超时报错 |
| RAII 归还 | `PooledConn` 在 `Drop` 中自动归还；`mark_broken()` 的坏连接销毁不归还 |

锁粒度：仅在存取空闲连接 / 修改计数时加锁（`std::sync::Mutex`），业务数据流全程无锁。

## 构建与运行

```bash
# 0) 构建整个 workspace
cargo build --release

# 1) 启动模拟后端（另开一个终端，可选 --latency-ms 模拟慢后端）
./target/release/mock-server --listen 0.0.0.0:9000 --latency-ms 5

# 2) 启动网关（指向后端，默认 tcp=8000 http=8080，托管 frontend/dist）
./target/release/gateway --backend 127.0.0.1:9000 --tcp-listen 0.0.0.0:8000 --http-listen 0.0.0.0:8080

# 3) 压测（默认 1 万请求 / 200 并发）
./target/release/demo-client --addr 127.0.0.1:8000 --requests 10000 --concurrency 200
```

## 前端可视化后台

`frontend/`（Vue3 + Element Plus + ECharts + Vite），包含指标仪表盘、连接池趋势图、在线配置面板、运行日志。

```bash
cd frontend
npm install        # 建议国内使用 --registry https://registry.npmmirror.com
npm run dev        # 本地开发（已配置 /api 代理到 :8080）
npm run build      # 产物在 frontend/dist，由网关托管
```

浏览器打开 `http://<网关>:8080/` 即可访问后台。

## HTTP 管理接口

```bash
# 查询连接池状态（总/空闲/活跃/复用次数 + 当前配置）
curl http://127.0.0.1:8080/api/pool/stats

# 连接池历史采样（趋势图，后台每秒采样）
curl http://127.0.0.1:8080/api/pool/history

# 动态修改连接池参数
curl -X PUT http://127.0.0.1:8080/api/pool/config \
     -H 'Content-Type: application/json' \
     -d '{"max_connections": 64, "idle_timeout_ms": 60000}'

# 查看运行日志与错误统计
curl http://127.0.0.1:8080/api/logs
```

## 运行测试

```bash
cargo test            # 连接池 8 项集成测试
cargo test --release  # release 模式
```

## CI/CD 部署

推送 `main` 分支触发 GitHub Actions：在 `ubuntu-22.04` runner 上构建 Rust 后端（release）与前端，再通过 SSH 将二进制 + `dist` 部署到服务器并自动重启 `gateway` / `mock-server`。

所需仓库 Secrets：

| Secret | 说明 |
| --- | --- |
| `SERVER_HOST` | 服务器 IP / 域名 |
| `SERVER_USER` | SSH 登录用户（如 `root`） |
| `DEPLOY_SSH_KEY` | 专用部署私钥（公钥加入服务器 `~/.ssh/authorized_keys`） |
