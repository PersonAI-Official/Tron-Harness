use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;

use serde_json::Value;

use crate::{
    domain::agent::{
        AgentAguiEvent, AgentAguiEventSnapshot, AgentAguiProtocolEvent, AgentAguiStreamState,
    },
    shared::event_bus::AguiEventSink,
};

use super::agui_translate::{translate_agui_event, translate_protocol_events};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentAguiStreamError {
    StreamKeyRequired,
    ReplayReadFailed(String),
    ReplayWriteFailed(String),
    ReplayIncomplete(String),
}

pub trait AgentAguiReplayStore: Send + Sync {
    fn append_protocol_event(
        &self,
        stream_key: &str,
        event: AgentAguiProtocolEvent,
    ) -> Result<(), String>;

    /// Nonblocking delivery diagnostics; no database I/O on the bus publication path.
    fn record_delivery_failure(&self, _stream_key: &str, _detail: &str) {}

    fn replay_failure(&self, _stream_key: &str) -> Option<String> {
        None
    }

    fn replay_agui_events(&self, stream_key: &str) -> Result<Vec<AgentAguiEvent>, String>;

    fn replay_protocol_events(
        &self,
        stream_key: &str,
    ) -> Result<Vec<AgentAguiProtocolEvent>, String>;
}

/// One recorded AG-UI protocol event pushed to live subscribers.
#[derive(Clone, Debug)]
pub struct AgentAguiFanoutEvent {
    pub stream_key: String,
    pub event: AgentAguiProtocolEvent,
}

#[derive(Clone)]
pub struct AgentAguiStreamService {
    sink: Arc<AguiEventSink>,
    replay_store: Option<Arc<dyn AgentAguiReplayStore>>,
    transient_protocol: bool,
    protocol_buffer: Arc<Mutex<ProtocolBuffer>>,
    fanout: broadcast::Sender<AgentAguiFanoutEvent>,
}
struct ProtocolBuffer {
    frames: VecDeque<ProtocolFrame>,
    bytes: usize,
    next_sequence: u64,
}
impl Default for ProtocolBuffer {
    fn default() -> Self {
        // Sequences start at 1 so `cursor.unwrap_or(0)` reads the full tail.
        Self {
            frames: VecDeque::new(),
            bytes: 0,
            next_sequence: 1,
        }
    }
}
/// One recorded frame with a monotonically increasing sequence, so polling
/// readers can take only the frames appended since their last read.
struct ProtocolFrame {
    key: String,
    sequence: u64,
    event: AgentAguiProtocolEvent,
    bytes: usize,
}
const MAX_PROTOCOL_BUFFER_BYTES: usize = 8 * 1024 * 1024;
const MAX_PROTOCOL_BUFFER_FRAMES: usize = 8192;

pub(crate) struct AgentAguiStreamWatch {
    service: AgentAguiStreamService,
    stream_key: String,
}

impl Drop for AgentAguiStreamWatch {
    fn drop(&mut self) {
        let _ = self.service.deactivate(&self.stream_key);
    }
}

impl AgentAguiStreamService {
    pub fn new(sink: Arc<AguiEventSink>) -> Self {
        Self {
            sink,
            replay_store: None,
            transient_protocol: true,
            protocol_buffer: Arc::default(),
            fanout: broadcast::channel(1024).0,
        }
    }

    pub fn with_replay_store(
        sink: Arc<AguiEventSink>,
        replay_store: Arc<dyn AgentAguiReplayStore>,
    ) -> Self {
        Self {
            sink,
            replay_store: Some(replay_store),
            transient_protocol: false,
            protocol_buffer: Arc::default(),
            fanout: broadcast::channel(1024).0,
        }
    }
    /// Ordinary chat uses bounded in-memory reconnect evidence. Permanent
    /// protocol replay is an explicit debugging mode, separate from history.
    pub fn with_transient_protocol(mut self) -> Self {
        self.transient_protocol = true;
        self
    }
    pub(crate) fn records_transient_protocol(&self) -> bool {
        self.transient_protocol
    }

    /// Live AG-UI protocol events for remote subscribers. Replay remains the
    /// source of truth for cursor catch-up.
    pub fn subscribe(&self) -> broadcast::Receiver<AgentAguiFanoutEvent> {
        self.fanout.subscribe()
    }

    pub(crate) fn watch(&self, stream_key: &str) -> Option<AgentAguiStreamWatch> {
        self.activate(stream_key).ok()?;
        Some(AgentAguiStreamWatch {
            service: self.clone(),
            stream_key: stream_key.to_owned(),
        })
    }

    pub fn activate(&self, stream_key: &str) -> Result<AgentAguiStreamState, AgentAguiStreamError> {
        let stream_key = normalized_stream_key(stream_key)?;
        self.sink.activate_stream(stream_key.clone());
        Ok(self.state(stream_key))
    }

    pub fn deactivate(
        &self,
        stream_key: &str,
    ) -> Result<AgentAguiStreamState, AgentAguiStreamError> {
        let stream_key = normalized_stream_key(stream_key)?;
        self.sink.deactivate_stream(&stream_key);
        Ok(self.state(stream_key))
    }

    pub fn events(&self, stream_key: &str) -> Result<AgentAguiEventSnapshot, AgentAguiStreamError> {
        let stream_key = normalized_stream_key(stream_key)?;
        let snapshot = self.sink.snapshot();
        let memory_events = self
            .sink
            .replay(&stream_key)
            .into_iter()
            .filter_map(|event| translate_agui_event(&stream_key, &event))
            .collect::<Vec<_>>();
        let stored_events = match &self.replay_store {
            Some(store) => store
                .replay_agui_events(&stream_key)
                .map_err(AgentAguiStreamError::ReplayReadFailed)?,
            None => Vec::new(),
        };
        let events = merge_agui_events(stored_events, memory_events);
        let replay_incomplete = self.ensure_complete(&stream_key).is_err();
        Ok(AgentAguiEventSnapshot {
            replay_incomplete,
            stream_key,
            active_streams: snapshot.active_streams,
            total_streams: snapshot.total_streams,
            events,
        })
    }

    pub fn protocol_events(
        &self,
        stream_key: &str,
    ) -> Result<Vec<AgentAguiProtocolEvent>, AgentAguiStreamError> {
        let stream_key = normalized_stream_key(stream_key)?;
        if self.transient_protocol {
            let buffered = self
                .protocol_buffer
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .frames
                .iter()
                .filter(|frame| frame.key == stream_key)
                .map(|frame| frame.event.clone())
                .collect::<Vec<_>>();
            // Product diagnostics (MCP warnings, context notices) travel on the
            // event bus, not the native AG-UI stream. Their in-memory replay is
            // merged so transient reads stay SQLite-free yet complete.
            let memory_events = self
                .sink
                .replay(&stream_key)
                .into_iter()
                .flat_map(|event| translate_protocol_events(&stream_key, &event))
                .collect::<Vec<_>>();
            let mut events = merge_protocol_events(buffered, memory_events);
            for event in &mut events {
                project_product_visibility(&mut event.data);
            }
            return Ok(events);
        }
        let memory_events = self
            .sink
            .replay(&stream_key)
            .into_iter()
            .flat_map(|event| translate_protocol_events(&stream_key, &event))
            .collect::<Vec<_>>();
        let stored_events = match &self.replay_store {
            Some(store) => {
                // Delivery diagnostics describe history, not read availability.
                // A past dropped frame must not block later runs in this session.
                store
                    .replay_protocol_events(&stream_key)
                    .map_err(AgentAguiStreamError::ReplayReadFailed)?
            }
            None => Vec::new(),
        };
        let mut events = merge_protocol_events(stored_events, memory_events);
        // Older replay rows retain their original bytes; the public read boundary
        // applies the same visibility projection as new native output writes.
        for event in &mut events {
            project_product_visibility(&mut event.data);
        }
        Ok(events)
    }

    /// Incremental read for polling loops: only the frames recorded after
    /// `cursor` are cloned, plus the new cursor to pass to the next call.
    /// `None` means "read the retained history from the start". Durable-mode
    /// services return a full snapshot and no cursor.
    pub fn protocol_events_since(
        &self,
        stream_key: &str,
        cursor: Option<u64>,
    ) -> Result<(Vec<AgentAguiProtocolEvent>, Option<u64>), AgentAguiStreamError> {
        let stream_key = normalized_stream_key(stream_key)?;
        if !self.transient_protocol {
            let events = self.protocol_events(&stream_key)?;
            return Ok((events, None));
        }
        let after = cursor.unwrap_or(0);
        let buffer = self
            .protocol_buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut events = Vec::new();
        let mut latest = cursor;
        for frame in buffer
            .frames
            .iter()
            .filter(|frame| frame.key == stream_key && frame.sequence > after)
        {
            events.push(frame.event.clone());
            latest = Some(frame.sequence);
        }
        // Bus diagnostics (MCP warnings, context notices) are re-read from the
        // in-memory event replay each poll; the caller dedups them by stable
        // event identity, so repetition costs only the clone, not SQLite.
        let memory_events = self
            .sink
            .replay(&stream_key)
            .into_iter()
            .flat_map(|event| translate_protocol_events(&stream_key, &event))
            .collect::<Vec<_>>();
        let mut events = merge_protocol_events(events, memory_events);
        for event in &mut events {
            project_product_visibility(&mut event.data);
        }
        Ok((events, latest))
    }

    pub fn record_protocol_event(
        &self,
        stream_key: &str,
        event: AgentAguiProtocolEvent,
    ) -> Result<(), AgentAguiStreamError> {
        let stream_key = normalized_stream_key(stream_key)?;
        if self.transient_protocol {
            let bytes = serde_json::to_vec(&event)
                .map(|v| v.len() + stream_key.len())
                .unwrap_or(usize::MAX);
            if bytes > MAX_PROTOCOL_BUFFER_BYTES {
                return Ok(());
            }
            let mut buffer = self
                .protocol_buffer
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            while buffer.bytes.saturating_add(bytes) > MAX_PROTOCOL_BUFFER_BYTES
                || buffer.frames.len() >= MAX_PROTOCOL_BUFFER_FRAMES
            {
                if let Some(frame) = buffer.frames.pop_front() {
                    buffer.bytes = buffer.bytes.saturating_sub(frame.bytes);
                } else {
                    break;
                }
            }
            buffer.bytes += bytes;
            let sequence = buffer.next_sequence;
            buffer.next_sequence += 1;
            let _ = self.fanout.send(AgentAguiFanoutEvent {
                stream_key: stream_key.clone(),
                event: event.clone(),
            });
            buffer.frames.push_back(ProtocolFrame {
                key: stream_key,
                sequence,
                event,
                bytes,
            });
            return Ok(());
        }
        if let Some(store) = self.replay_store.as_ref() {
            if let Err(error) = store.append_protocol_event(&stream_key, event.clone()) {
                store.record_delivery_failure(&stream_key, &error);
                return Err(AgentAguiStreamError::ReplayWriteFailed(error));
            }
        }
        let _ = self.fanout.send(AgentAguiFanoutEvent { stream_key, event });
        Ok(())
    }

    pub fn mark_incomplete(&self, stream_key: &str, detail: &str) {
        if let Some(store) = &self.replay_store {
            store.record_delivery_failure(stream_key, detail);
        }
    }

    pub fn ensure_complete(&self, stream_key: &str) -> Result<(), AgentAguiStreamError> {
        match self
            .replay_store
            .as_ref()
            .and_then(|store| store.replay_failure(stream_key))
        {
            Some(error) => Err(AgentAguiStreamError::ReplayIncomplete(error)),
            None => Ok(()),
        }
    }

    fn state(&self, stream_key: String) -> AgentAguiStreamState {
        let snapshot = self.sink.snapshot();
        AgentAguiStreamState {
            stream_key,
            active_streams: snapshot.active_streams,
            total_streams: snapshot.total_streams,
            total_replay_bytes: snapshot.total_replay_bytes,
            replay_evicted: snapshot.replay_evicted,
        }
    }
}

fn normalized_stream_key(stream_key: &str) -> Result<String, AgentAguiStreamError> {
    let stream_key = stream_key.trim();
    if stream_key.is_empty() {
        return Err(AgentAguiStreamError::StreamKeyRequired);
    }
    Ok(stream_key.to_string())
}

fn merge_agui_events(
    stored_events: Vec<AgentAguiEvent>,
    memory_events: Vec<AgentAguiEvent>,
) -> Vec<AgentAguiEvent> {
    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();
    for event in stored_events.into_iter().chain(memory_events) {
        if seen.insert(event.id.clone()) {
            merged.push(event);
        }
    }
    merged
}

fn merge_protocol_events(
    stored_events: Vec<AgentAguiProtocolEvent>,
    memory_events: Vec<AgentAguiProtocolEvent>,
) -> Vec<AgentAguiProtocolEvent> {
    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();
    for event in stored_events.into_iter().chain(memory_events) {
        // Repeated content is valid. Only a durable event identity is a dedup key.
        if let Some(key) = protocol_event_identity(&event) {
            if !seen.insert(key) {
                continue;
            }
        }
        merged.push(event);
    }
    merged
}

pub fn protocol_event_identity(event: &AgentAguiProtocolEvent) -> Option<String> {
    event
        .data
        .pointer("/metadata/tron/eventId")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

// The lossless native/history codec deliberately retains private product reasoning.
// Its public transport projection follows the existing product visibility policy,
// at explicit SDK Content paths only; arbitrary user JSON is never traversed.
pub(crate) fn project_product_visibility(data: &mut Value) {
    let path = match data.get("type").and_then(Value::as_str) {
        Some("CUSTOM") if data.get("name").and_then(Value::as_str) == Some("framework.message") => {
            "/value/content"
        }
        Some("RUN_FINISHED") => "/result/content",
        _ => return,
    };
    if let Some(content) = data.pointer_mut(path).and_then(Value::as_array_mut) {
        content.retain_mut(|block| {
            if block.get("type").and_then(Value::as_str) != Some("json") {
                return true;
            }
            match block
                .pointer("/value/tronMessagePart/type")
                .and_then(Value::as_str)
            {
                Some(
                    "context" | "reasoning" | "compactionCheckpoint" | "toolCall" | "toolResult",
                ) => false,
                Some("reasoningSummary") => {
                    let Some(text) = block
                        .pointer("/value/tronMessagePart/text")
                        .and_then(Value::as_str)
                    else {
                        return false;
                    };
                    *block = serde_json::json!({"type":"text", "text":text});
                    true
                }
                _ => true,
            }
        });
    }
}
