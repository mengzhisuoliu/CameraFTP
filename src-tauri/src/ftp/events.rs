// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::ftp::types::{DomainEvent, ServerStateSnapshot, ServerStats};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::broadcast;
use tracing::{trace, warn};
use tauri::Emitter;

#[derive(Debug, Clone, Default, PartialEq)]
struct EventBusReplayState {
    bind_addr: Option<String>,
    stats: Option<ServerStats>,
    deferred_stats: Option<ServerStats>,
    queued_events: VecDeque<DomainEvent>,
}

#[derive(Debug)]
struct EventBusInner {
    tx: broadcast::Sender<DomainEvent>,
    replay_state: EventBusReplayState,
}

/// 事件总线 - 中心化的领域事件分发
#[derive(Debug, Clone)]
pub struct EventBus {
    inner: Arc<Mutex<EventBusInner>>,
}

impl EventBus {
    /// 创建新的事件总线
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            inner: Arc::new(Mutex::new(EventBusInner {
                tx,
                replay_state: EventBusReplayState::default(),
            })),
        }
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.inner
            .lock()
            .expect("event bus mutex poisoned")
            .tx
            .subscribe()
    }

    /// 发布事件
    pub fn emit(&self, event: DomainEvent) {
        let send_result = self
            .inner
            .lock()
            .expect("event bus mutex poisoned")
            .tx
            .send(event);

        if let Err(broadcast::error::SendError(_)) = send_result {
            warn!("Event dropped: no active subscribers");
        }
    }

    /// 发布服务器启动事件
    pub async fn emit_server_started(&self, bind_addr: impl Into<String>) {
        let bind_addr = bind_addr.into();
        let send_result = {
            let mut inner = self.inner.lock().expect("event bus mutex poisoned");
            inner.replay_state.bind_addr = Some(bind_addr.clone());
            if inner.replay_state.stats.is_none() {
                inner.replay_state.stats = inner.replay_state.deferred_stats.take();
            }
            inner.tx.send(DomainEvent::ServerStarted { bind_addr })
        };

        if let Err(broadcast::error::SendError(_)) = send_result {
            warn!("Event dropped: no active subscribers");
        }
    }

    /// 发布服务器停止事件
    pub async fn emit_server_stopped(&self) {
        let send_result = {
            let mut inner = self.inner.lock().expect("event bus mutex poisoned");
            inner.replay_state = EventBusReplayState::default();
            inner.tx.send(DomainEvent::ServerStopped)
        };

        if let Err(broadcast::error::SendError(_)) = send_result {
            warn!("Event dropped: no active subscribers");
        }
    }

    /// 发布文件上传事件
    pub fn emit_file_uploaded(&self, path: impl Into<String>, size: u64) {
        let event = DomainEvent::FileUploaded {
            path: path.into(),
            size,
        };
        self.emit_non_state_event(event);
    }

    /// 发布文件索引变化事件
    pub fn emit_file_index_changed(&self, count: usize, latest_filename: Option<String>) {
        let event = DomainEvent::FileIndexChanged {
            count,
            latest_filename,
        };
        self.emit_non_state_event(event);
    }

    /// 发布统计更新（带增量检查）
    pub async fn emit_stats_updated(&self, stats: ServerStats) {
        let send_result = {
            let mut inner = self.inner.lock().expect("event bus mutex poisoned");
            if inner.replay_state.bind_addr.is_none() {
                if inner.tx.receiver_count() == 0 {
                    inner.replay_state.deferred_stats = Some(stats);
                }
                return;
            }

            match inner.replay_state.stats.as_ref() {
                None => {
                    inner.replay_state.stats = Some(stats.clone());
                    Some(inner.tx.send(DomainEvent::StatsUpdated(stats)))
                }
                Some(last_stats) if last_stats.has_changed(&stats) => {
                    inner.replay_state.stats = Some(stats.clone());
                    Some(inner.tx.send(DomainEvent::StatsUpdated(stats)))
                }
                Some(_) => None,
            }
        };

        if let Some(Err(broadcast::error::SendError(_))) = send_result {
            warn!("Event dropped: no active subscribers");
        } else if send_result.is_some() {
            trace!("Stats updated event emitted");
        } else {
            trace!("Stats unchanged, skipping event");
        }
    }

    fn emit_non_state_event(&self, event: DomainEvent) {
        let mut inner = self.inner.lock().expect("event bus mutex poisoned");
        if inner.tx.receiver_count() == 0 {
            inner.replay_state.queued_events.push_back(event.clone());
        }
        if let Err(broadcast::error::SendError(_)) = inner.tx.send(event) {
            warn!("Event dropped: no active subscribers");
        }
    }

}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// 事件处理器trait
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    /// 处理事件
    async fn handle(&mut self, event: &DomainEvent);

    /// 获取感兴趣的事件类型（None表示所有事件）
    fn interested_types(&self) -> Option<Vec<&'static str>> {
        None
    }
}

/// 事件处理器管理器
pub struct EventProcessor {
    bus_inner: Weak<Mutex<EventBusInner>>,
    caught_up: bool,
    pending_events: VecDeque<DomainEvent>,
    rx: broadcast::Receiver<DomainEvent>,
    handlers: Vec<Box<dyn EventHandler>>,
}

impl EventProcessor {
    /// 创建事件处理器
    pub fn new(bus: &EventBus) -> Self {
        let rx = bus.subscribe();

        Self {
            bus_inner: Arc::downgrade(&bus.inner),
            caught_up: false,
            pending_events: VecDeque::new(),
            rx,
            handlers: Vec::new(),
        }
    }

    /// 注册处理器
    pub fn register<H: EventHandler + 'static>(
        mut self,
        handler: H,
    ) -> Self {
        self.handlers.push(Box::new(handler));
        self
    }

    pub async fn catch_up(&mut self) {
        if self.caught_up {
            return;
        }
        let Some(bus_inner) = self.bus_inner.upgrade() else {
            self.caught_up = true;
            return;
        };
        let mut replay_state = {
            let mut inner = bus_inner.lock().expect("event bus mutex poisoned");
            let replay_state = inner.replay_state.clone();
            inner.replay_state.queued_events.clear();
            replay_state
        };
        let replayed_non_state_events = replay_state.queued_events.clone();

        while let Ok(event) = self.rx.try_recv() {
            apply_replay_event(&mut replay_state, &event);
            self.pending_events.push_back(event);
        }

        self.replay_state_to_handlers(replay_state).await;
        self.flush_pending_events_after_catch_up(replayed_non_state_events)
            .await;
        self.caught_up = true;
    }

    /// 运行处理器循环
    pub async fn run(mut self) {
        if !self.caught_up {
            self.catch_up().await;
        }
        self.run_loop().await;
    }

    async fn run_loop(&mut self) {
        loop {
            match self.rx.recv().await {
                Ok(event) => {
                    self.dispatch_event(&event).await;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(dropped = n, "Event processor lagged, some events dropped");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    }

    async fn replay_state_to_handlers(&mut self, replay_state: EventBusReplayState) {
        if let Some(bind_addr) = replay_state.bind_addr {
            let event = DomainEvent::ServerStarted { bind_addr };
            for handler in self.handlers.iter_mut() {
                if should_handle(handler.as_ref(), &event) {
                    handler.handle(&event).await;
                }
            }
        }

        if let Some(stats) = replay_state.stats {
            let event = DomainEvent::StatsUpdated(stats);
            for handler in self.handlers.iter_mut() {
                if should_handle(handler.as_ref(), &event) {
                    handler.handle(&event).await;
                }
            }
        }
    }

    async fn flush_pending_events_after_catch_up(
        &mut self,
        mut replayed_non_state_events: VecDeque<DomainEvent>,
    ) {
        while let Some(event) = replayed_non_state_events.pop_front() {
            self.dispatch_event(&event).await;
        }

        let should_emit_server_stopped = self
            .pending_events
            .iter()
            .rev()
            .find_map(|event| match event {
                DomainEvent::ServerStopped => Some(true),
                DomainEvent::ServerStarted { .. } => Some(false),
                _ => None,
            })
            .unwrap_or(false);

        while let Some(event) = self.pending_events.pop_front() {
            match event {
                DomainEvent::FileUploaded { .. } | DomainEvent::FileIndexChanged { .. } => {
                    self.dispatch_event(&event).await;
                }
                DomainEvent::ServerStopped if should_emit_server_stopped => {
                    self.dispatch_event(&event).await;
                }
                DomainEvent::ServerStarted { .. }
                | DomainEvent::ServerStopped
                | DomainEvent::StatsUpdated { .. } => {}
            }
        }
    }

    async fn dispatch_event(&mut self, event: &DomainEvent) {
        for handler in self.handlers.iter_mut() {
            if should_handle(handler.as_ref(), event) {
                handler.handle(event).await;
            }
        }
    }
}

fn apply_replay_event(replay_state: &mut EventBusReplayState, event: &DomainEvent) {
    match event {
        DomainEvent::ServerStarted { bind_addr } => {
            replay_state.bind_addr = Some(bind_addr.clone());
            if replay_state.stats.is_none() {
                replay_state.stats = replay_state.deferred_stats.take();
            }
        }
        DomainEvent::ServerStopped => {
            *replay_state = EventBusReplayState::default();
        }
        DomainEvent::StatsUpdated(stats) => {
            if replay_state.bind_addr.is_some() {
                replay_state.stats = Some(stats.clone());
            } else {
                replay_state.deferred_stats = Some(stats.clone());
            }
        }
        DomainEvent::FileUploaded { .. } | DomainEvent::FileIndexChanged { .. } => {}
    }
}

/// 检查处理器是否应该处理该事件
fn should_handle(handler: &dyn EventHandler, event: &DomainEvent) -> bool {
    match handler.interested_types() {
        None => true,
        Some(types) => types.contains(&event_type_name(event)),
    }
}

/// 获取事件类型名称
fn event_type_name(event: &DomainEvent) -> &'static str {
    match event {
        DomainEvent::ServerStarted { .. } => "ServerStarted",
        DomainEvent::ServerStopped { .. } => "ServerStopped",
        DomainEvent::FileUploaded { .. } => "FileUploaded",
        DomainEvent::StatsUpdated { .. } => "StatsUpdated",
        DomainEvent::FileIndexChanged { .. } => "FileIndexChanged",
    }
}

/// 统计事件处理器 - 将事件转换为前端推送
/// 注意：EventBus.emit_stats_updated() 已做增量检查，这里直接推送即可
pub struct StatsEventHandler {
    app_handle: tauri::AppHandle,
}

impl StatsEventHandler {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    /// 向前端发送事件，失败时记录警告日志
    fn emit_to_frontend<T: Serialize + Clone>(&self, event_name: &str, payload: T) {
        if let Err(e) = self.app_handle.emit(event_name, payload) {
            warn!(event = event_name, error = %e, "Failed to emit frontend event");
        }
    }

    fn sync_android_service_state(&self, snapshot: &ServerStateSnapshot) {
        crate::platform::get_platform().sync_android_service_state(&self.app_handle, snapshot);
    }

    fn emit_frontend_json(&self, event_name: &str, payload: serde_json::Value) {
        self.emit_to_frontend(event_name, payload);
    }
}

trait ServerEventFanout {
    fn emit_frontend_json(&mut self, event_name: &str, payload: serde_json::Value);
    fn sync_android_service_state(&mut self, snapshot: &ServerStateSnapshot);
}

impl ServerEventFanout for StatsEventHandler {
    fn emit_frontend_json(&mut self, event_name: &str, payload: serde_json::Value) {
        StatsEventHandler::emit_frontend_json(self, event_name, payload);
    }

    fn sync_android_service_state(&mut self, snapshot: &ServerStateSnapshot) {
        StatsEventHandler::sync_android_service_state(self, snapshot);
    }
}

fn fan_out_server_event(target: &mut dyn ServerEventFanout, event: &DomainEvent) {
    match event {
        DomainEvent::StatsUpdated(stats) => {
            let snapshot = ServerStateSnapshot::from(stats);
            target.emit_frontend_json(
                "stats-update",
                serde_json::to_value(snapshot.clone())
                    .expect("server snapshot should serialize for frontend events"),
            );
            target.sync_android_service_state(&snapshot);
        }
        DomainEvent::ServerStarted { bind_addr } => {
            let (ip, port) = parse_bind_addr(bind_addr);
            target.emit_frontend_json(
                "server-started",
                serde_json::json!({
                    "ip": ip,
                    "port": port
                }),
            );
            target.sync_android_service_state(&ServerStateSnapshot {
                is_running: true,
                ..ServerStateSnapshot::default()
            });
        }
        DomainEvent::ServerStopped => {
            target.emit_frontend_json("server-stopped", serde_json::Value::Null);
            target.sync_android_service_state(&ServerStateSnapshot::default());
        }
        DomainEvent::FileUploaded { path, size } => {
            target.emit_frontend_json(
                "file-uploaded",
                serde_json::json!({ "path": path, "size": size }),
            );
        }
        DomainEvent::FileIndexChanged { count, latest_filename } => {
            target.emit_frontend_json(
                "file-index-changed",
                serde_json::json!({
                    "count": count,
                    "latestFilename": latest_filename
                }),
            );
        }
    }
}

#[async_trait::async_trait]
impl EventHandler for StatsEventHandler {
    async fn handle(&mut self, event: &DomainEvent) {
        fan_out_server_event(self, event);
    }

    fn interested_types(&self) -> Option<Vec<&'static str>> {
        Some(vec![
            "StatsUpdated",
            "ServerStarted",
            "ServerStopped",
            "FileUploaded",
            "FileIndexChanged",
        ])
    }
}

/// 解析 bind_addr (格式: "ip:port") 返回 (ip, port)
fn parse_bind_addr(bind_addr: &str) -> (String, u16) {
    let parts: Vec<&str> = bind_addr.split(':').collect();
    if parts.len() == 2 {
        let ip = parts[0].to_string();
        let port = parts[1].parse().unwrap_or(2121);
        (ip, port)
    } else {
        ("0.0.0.0".to_string(), 2121)
    }
}

/// 托盘状态更新处理器 - 监听统计更新并更新托盘图标
/// 替代原有的轮询机制，使用事件驱动更新
pub struct TrayUpdateHandler {
    app_handle: tauri::AppHandle,
    last_client_count: std::sync::atomic::AtomicU32,
}

impl TrayUpdateHandler {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            app_handle,
            last_client_count: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

#[async_trait::async_trait]
impl EventHandler for TrayUpdateHandler {
    async fn handle(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::ServerStarted { .. } => {
                // 服务器启动时更新托盘为运行状态
                crate::platform::get_platform().on_server_started(&self.app_handle);
            }
            DomainEvent::ServerStopped => {
                // 服务器停止时更新托盘为停止状态
                crate::platform::get_platform().on_server_stopped(&self.app_handle);
                self.last_client_count.store(0, std::sync::atomic::Ordering::Relaxed);
            }
            DomainEvent::StatsUpdated(stats) => {
                let client_count = stats.active_connections as u32;
                let last_count = self.last_client_count.load(std::sync::atomic::Ordering::Relaxed);

                // 仅在客户端数量变化时更新托盘图标状态
                if client_count != last_count {
                    crate::platform::get_platform()
                        .update_server_state(&self.app_handle, client_count);
                    self.last_client_count.store(client_count, std::sync::atomic::Ordering::Relaxed);
                }
            }
            // 文件索引变化事件不需要处理托盘状态
            DomainEvent::FileIndexChanged { .. } => {}
            // 文件上传事件不需要处理托盘状态
            DomainEvent::FileUploaded { .. } => {}
        }
    }

    fn interested_types(&self) -> Option<Vec<&'static str>> {
        Some(vec!["ServerStarted", "ServerStopped", "StatsUpdated"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Clone, Default)]
    struct RecordingHandler {
        events: Arc<Mutex<Vec<DomainEvent>>>,
    }

    #[derive(Default)]
    struct RecordingFanout {
        frontend_events: Vec<(String, serde_json::Value)>,
        android_snapshots: Vec<ServerStateSnapshot>,
    }

    impl ServerEventFanout for RecordingFanout {
        fn emit_frontend_json(&mut self, event_name: &str, payload: serde_json::Value) {
            self.frontend_events.push((event_name.to_string(), payload));
        }

        fn sync_android_service_state(&mut self, snapshot: &ServerStateSnapshot) {
            self.android_snapshots.push(snapshot.clone());
        }
    }

    #[async_trait::async_trait]
    impl EventHandler for RecordingHandler {
        async fn handle(&mut self, event: &DomainEvent) {
            self.events.lock().await.push(event.clone());
        }

        fn interested_types(&self) -> Option<Vec<&'static str>> {
            Some(vec![
                "ServerStarted",
                "ServerStopped",
                "StatsUpdated",
                "FileUploaded",
                "FileIndexChanged",
            ])
        }
    }

    #[test]
    fn tray_update_handler_calls_platform_update_server_state_for_active_clients() {
        let source = include_str!("events.rs");

        assert!(source.contains("update_server_state(&self.app_handle, client_count);"));
    }

    #[tokio::test]
    async fn late_event_processor_replays_running_state_to_handlers() {
        let bus = EventBus::new();
        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_stats_updated(ServerStats {
            active_connections: 2,
            total_uploads: 4,
            total_bytes_received: 1024,
            last_uploaded_file: Some("latest.jpg".to_string()),
        })
        .await;

        let handler = RecordingHandler::default();
        let events = handler.events.clone();
        let processor = EventProcessor::new(&bus).register(handler);

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        let recorded = events.lock().await.clone();
        assert_eq!(
            recorded,
            vec![
                DomainEvent::ServerStarted {
                    bind_addr: "127.0.0.1:2121".to_string(),
                },
                DomainEvent::StatsUpdated(ServerStats {
                    active_connections: 2,
                    total_uploads: 4,
                    total_bytes_received: 1024,
                    last_uploaded_file: Some("latest.jpg".to_string()),
                }),
            ]
        );
    }

    #[tokio::test]
    async fn late_event_processor_does_not_replay_after_server_stops() {
        let bus = EventBus::new();
        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_server_stopped().await;

        let handler = RecordingHandler::default();
        let events = handler.events.clone();
        let processor = EventProcessor::new(&bus).register(handler);

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert!(events.lock().await.is_empty());
    }

    #[tokio::test]
    async fn stop_before_run_prevents_stale_replay() {
        let bus = EventBus::new();
        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_stats_updated(ServerStats {
            active_connections: 2,
            total_uploads: 1,
            total_bytes_received: 32,
            last_uploaded_file: Some("before-stop.jpg".to_string()),
        })
        .await;

        let handler = RecordingHandler::default();
        let events = handler.events.clone();
        let processor = EventProcessor::new(&bus).register(handler);

        bus.emit_server_stopped().await;

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert!(events.lock().await.is_empty());
    }

    #[tokio::test]
    async fn queued_start_then_stop_before_run_does_not_replay_stale_state() {
        let bus = EventBus::new();
        let handler = RecordingHandler::default();
        let events = handler.events.clone();
        let processor = EventProcessor::new(&bus).register(handler);

        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_stats_updated(ServerStats {
            active_connections: 4,
            total_uploads: 2,
            total_bytes_received: 128,
            last_uploaded_file: Some("queued-before-stop.jpg".to_string()),
        })
        .await;
        bus.emit_server_stopped().await;

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(events.lock().await.as_slice(), &[DomainEvent::ServerStopped]);
    }

    #[tokio::test]
    async fn catch_up_preserves_queued_non_state_events() {
        let bus = EventBus::new();
        let handler = RecordingHandler::default();
        let events = handler.events.clone();
        let processor = EventProcessor::new(&bus).register(handler);

        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_file_uploaded("queued.jpg", 42);
        bus.emit_file_index_changed(3, Some("queued.jpg".to_string()));

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(
            events.lock().await.as_slice(),
            &[
                DomainEvent::ServerStarted {
                    bind_addr: "127.0.0.1:2121".to_string(),
                },
                DomainEvent::FileUploaded {
                    path: "queued.jpg".to_string(),
                    size: 42,
                },
                DomainEvent::FileIndexChanged {
                    count: 3,
                    latest_filename: Some("queued.jpg".to_string()),
                },
            ]
        );
    }

    #[tokio::test]
    async fn non_state_events_emitted_before_processor_creation_are_replayed() {
        let bus = EventBus::new();
        bus.emit_file_uploaded("early-upload.jpg", 7);
        bus.emit_file_index_changed(1, Some("early-upload.jpg".to_string()));

        let handler = RecordingHandler::default();
        let events = handler.events.clone();
        let processor = EventProcessor::new(&bus).register(handler);

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(
            events.lock().await.as_slice(),
            &[
                DomainEvent::FileUploaded {
                    path: "early-upload.jpg".to_string(),
                    size: 7,
                },
                DomainEvent::FileIndexChanged {
                    count: 1,
                    latest_filename: Some("early-upload.jpg".to_string()),
                },
            ]
        );
    }

    #[tokio::test]
    async fn stats_updated_before_server_started_is_replayed_after_startup_handoff() {
        let bus = EventBus::new();
        let handler = RecordingHandler::default();
        let events = handler.events.clone();
        let processor = EventProcessor::new(&bus).register(handler);

        bus.emit_stats_updated(ServerStats {
            active_connections: 2,
            total_uploads: 5,
            total_bytes_received: 512,
            last_uploaded_file: Some("handoff.jpg".to_string()),
        })
        .await;
        bus.emit_server_started("127.0.0.1:2121").await;

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(
            events.lock().await.as_slice(),
            &[
                DomainEvent::ServerStarted {
                    bind_addr: "127.0.0.1:2121".to_string(),
                },
                DomainEvent::StatsUpdated(ServerStats {
                    active_connections: 2,
                    total_uploads: 5,
                    total_bytes_received: 512,
                    last_uploaded_file: Some("handoff.jpg".to_string()),
                }),
            ]
        );
    }

    #[tokio::test]
    async fn server_stop_clears_replayed_stats_state() {
        let bus = EventBus::new();
        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_stats_updated(ServerStats {
            active_connections: 1,
            total_uploads: 2,
            total_bytes_received: 128,
            last_uploaded_file: Some("stale.jpg".to_string()),
        })
        .await;
        bus.emit_server_stopped().await;

        let replay_state = bus
            .inner
            .lock()
            .expect("event bus mutex poisoned")
            .replay_state
            .clone();

        assert_eq!(replay_state, EventBusReplayState::default());
    }

    #[tokio::test]
    async fn stats_updates_after_stop_do_not_restore_replay_running_state() {
        let bus = EventBus::new();
        bus.emit_server_started("192.168.1.10:2121").await;
        bus.emit_server_stopped().await;
        bus.emit_stats_updated(ServerStats {
            active_connections: 1,
            total_uploads: 1,
            total_bytes_received: 64,
            last_uploaded_file: Some("ignored-after-stop.jpg".to_string()),
        })
        .await;

        let replay_state = bus
            .inner
            .lock()
            .expect("event bus mutex poisoned")
            .replay_state
            .clone();

        assert_eq!(replay_state, EventBusReplayState::default());
    }

    #[tokio::test]
    async fn event_processor_does_not_keep_channel_alive_after_bus_drop() {
        let bus = EventBus::new();
        let processor = EventProcessor::new(&bus);
        drop(bus);

        tokio::time::timeout(std::time::Duration::from_millis(100), processor.run())
            .await
            .expect("event processor should exit after bus drop");
    }

    #[test]
    fn fan_out_server_events_drive_ui_and_android_sync_from_same_source() {
        let mut fanout = RecordingFanout::default();
        let stats = ServerStats {
            active_connections: 3,
            total_uploads: 7,
            total_bytes_received: 2048,
            last_uploaded_file: Some("dual.jpg".to_string()),
        };

        fan_out_server_event(
            &mut fanout,
            &DomainEvent::ServerStarted {
                bind_addr: "127.0.0.1:2121".to_string(),
            },
        );
        fan_out_server_event(&mut fanout, &DomainEvent::StatsUpdated(stats.clone()));
        fan_out_server_event(&mut fanout, &DomainEvent::ServerStopped);

        assert_eq!(
            fanout.frontend_events,
            vec![
                (
                    "server-started".to_string(),
                    serde_json::json!({ "ip": "127.0.0.1", "port": 2121 }),
                ),
                (
                    "stats-update".to_string(),
                    serde_json::to_value(ServerStateSnapshot::from(&stats))
                        .expect("server snapshot should serialize"),
                ),
                ("server-stopped".to_string(), serde_json::Value::Null),
            ]
        );
        assert_eq!(
            fanout.android_snapshots,
            vec![
                ServerStateSnapshot {
                    is_running: true,
                    ..ServerStateSnapshot::default()
                },
                ServerStateSnapshot::from(&stats),
                ServerStateSnapshot::default(),
            ]
        );
    }

}
