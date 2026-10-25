//! One selected window, one renderer subscription, independent per-pane targets. Payload remains charged until renderer ack.
use crate::{Error, Result, model::Size};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, sync::Mutex};
use tokio::sync::Notify;
pub const MAX_OUTPUT: usize = 262144;
pub const MAX_BATCH: usize = 16384;
pub const MAX_EVENTS: usize = 256;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generation {
    pub connection: u64,
    pub pane: u64,
    pub size: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneTarget {
    pub connection: u64,
    pub pane_id: String,
    pub generation: Generation,
}
impl PaneTarget {
    /// A viewport change does not change where queued user input belongs.
    pub fn same_input_destination(&self, other: &Self) -> bool {
        self.connection == other.connection
            && self.pane_id == other.pane_id
            && self.generation.connection == other.generation.connection
            && self.generation.pane == other.generation.pane
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputKind {
    Resize,
    ResetBegin,
    ResetChunk,
    ResetEnd,
    Live,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct OutputEvent {
    pub subscription: u64,
    pub sequence: u64,
    pub target: PaneTarget,
    pub kind: OutputKind,
    pub size: Size,
    pub bytes: Vec<u8>,
}
#[derive(Default)]
struct Queue {
    subscription: u64,
    sequence: u64,
    delivered: u64,
    acked: u64,
    bytes: usize,
    pending: VecDeque<OutputEvent>,
    inflight: VecDeque<(u64, usize)>,
    closed: bool,
    active: bool,
}
#[derive(Default)]
pub struct OutputQueue {
    inner: Mutex<Queue>,
    space: Notify,
}
impl OutputQueue {
    pub fn subscribe(&self) -> Result<u64> {
        let mut q = self.inner.lock().unwrap();
        if q.closed {
            return Err(Error::Closed);
        }
        q.subscription = q.subscription.checked_add(1).ok_or(Error::Closed)?;
        q.pending.clear();
        q.inflight.clear();
        q.bytes = 0;
        q.delivered = 0;
        q.acked = 0;
        q.sequence = 0;
        q.active = true;
        let id = q.subscription;
        drop(q);
        self.space.notify_waiters();
        Ok(id)
    }
    pub fn unsubscribe(&self, id: u64) {
        let mut q = self.inner.lock().unwrap();
        if q.subscription == id && q.active {
            q.active = false;
            q.pending.clear();
            q.inflight.clear();
            q.bytes = 0;
            q.subscription = q
                .subscription
                .checked_add(1)
                .expect("subscription exhausted");
        }
        drop(q);
        self.space.notify_waiters();
    }
    pub fn subscription(&self) -> u64 {
        let q = self.inner.lock().unwrap();
        if q.active { q.subscription } else { 0 }
    }
    pub fn try_push(
        &self,
        subscription: u64,
        target: PaneTarget,
        kind: OutputKind,
        size: Size,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let mut q = self.inner.lock().unwrap();
        if q.closed {
            return Err(Error::Closed);
        }
        if q.subscription != subscription || !q.active {
            return Err(Error::StaleGeneration);
        }
        if bytes.len() > MAX_BATCH {
            return Err(Error::InvalidArgument);
        }
        if q.bytes + bytes.len() > MAX_OUTPUT || q.pending.len() + q.inflight.len() >= MAX_EVENTS {
            return Err(Error::QueueFull);
        }
        q.sequence = q.sequence.checked_add(1).ok_or(Error::Closed)?;
        let sequence = q.sequence;
        q.bytes += bytes.len();
        q.pending.push_back(OutputEvent {
            subscription,
            sequence,
            target,
            kind,
            size,
            bytes,
        });
        Ok(())
    }
    pub async fn push(
        &self,
        subscription: u64,
        target: &PaneTarget,
        kind: OutputKind,
        size: Size,
        bytes: &[u8],
    ) -> Result<()> {
        // The caller holds at most one parser-sized read; no secondary unbounded delivery queue.
        for chunk in bytes.chunks(MAX_BATCH).chain(if bytes.is_empty() {
            Some(&[][..])
        } else {
            None
        }) {
            loop {
                let notified = self.space.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                match self.try_push(
                    subscription,
                    target.clone(),
                    kind.clone(),
                    size,
                    chunk.to_vec(),
                ) {
                    Ok(()) => break,
                    Err(Error::QueueFull) => notified.await,
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }
    pub fn poll(&self, id: u64) -> Result<Option<OutputEvent>> {
        let mut q = self.inner.lock().unwrap();
        if q.closed {
            return Err(Error::Closed);
        }
        if q.subscription != id || !q.active {
            return Err(Error::StaleGeneration);
        }
        let e = q.pending.pop_front();
        if let Some(ref e) = e {
            q.delivered = e.sequence;
            q.inflight.push_back((e.sequence, e.bytes.len()));
        }
        Ok(e)
    }
    pub fn ack(&self, id: u64, sequence: u64) -> Result<()> {
        let mut q = self.inner.lock().unwrap();
        if q.closed {
            return Err(Error::Closed);
        }
        if id != q.subscription || !q.active {
            return Err(Error::StaleGeneration);
        }
        if sequence > q.delivered {
            return Err(Error::InvalidArgument);
        }
        if sequence <= q.acked {
            return Ok(());
        }
        while q.inflight.front().is_some_and(|(s, _)| *s <= sequence) {
            let (_, n) = q.inflight.pop_front().unwrap();
            q.bytes -= n;
        }
        q.acked = sequence;
        drop(q);
        self.space.notify_waiters();
        Ok(())
    }
    pub fn metrics(&self) -> (usize, usize) {
        let q = self.inner.lock().unwrap();
        (q.bytes, q.pending.len() + q.inflight.len())
    }
    pub fn close(&self) {
        let mut q = self.inner.lock().unwrap();
        q.closed = true;
        q.active = false;
        q.pending.clear();
        q.inflight.clear();
        q.bytes = 0;
        drop(q);
        self.space.notify_waiters();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn target() -> PaneTarget {
        PaneTarget {
            connection: 1,
            pane_id: "%1".into(),
            generation: Generation {
                connection: 1,
                pane: 1,
                size: 1,
            },
        }
    }
    #[test]
    fn input_survives_size_changes_but_never_focus_or_connection_changes() {
        let original = target();
        let mut resized = original.clone();
        resized.generation.size += 1;
        assert!(original.same_input_destination(&resized));
        resized.generation.pane += 1;
        assert!(!original.same_input_destination(&resized));
        let mut reconnected = original.clone();
        reconnected.generation.connection += 1;
        assert!(!original.same_input_destination(&reconnected));
    }
    #[tokio::test]
    async fn full_queue_ack_and_close() {
        let q = std::sync::Arc::new(OutputQueue::default());
        let id = q.subscribe().unwrap();
        for _ in 0..16 {
            q.try_push(
                id,
                target(),
                OutputKind::Live,
                Size::default(),
                vec![0; MAX_BATCH],
            )
            .unwrap();
        }
        assert_eq!(q.metrics().0, MAX_OUTPUT);
        let e = q.poll(id).unwrap().unwrap();
        assert_eq!(q.metrics().0, MAX_OUTPUT);
        assert_eq!(q.ack(id, e.sequence + 1), Err(Error::InvalidArgument));
        q.ack(id, e.sequence).unwrap();
        q.ack(id, e.sequence).unwrap();
        q.try_push(
            id,
            target(),
            OutputKind::Live,
            Size::default(),
            vec![0; MAX_BATCH],
        )
        .unwrap();
        let other = q.clone();
        let task = tokio::spawn(async move {
            other
                .push(id, &target(), OutputKind::Live, Size::default(), b"x")
                .await
        });
        tokio::task::yield_now().await;
        q.close();
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap(),
            Err(Error::Closed)
        );
        assert_eq!(q.metrics(), (0, 0));
    }
    #[test]
    fn unsubscribed_queue_has_no_consumer() {
        let q = OutputQueue::default();
        let id = q.subscribe().unwrap();
        q.unsubscribe(id);
        assert_eq!(q.subscription(), 0);
        assert_eq!(
            q.try_push(id, target(), OutputKind::Live, Size::default(), vec![1]),
            Err(Error::StaleGeneration)
        );
        assert_eq!(q.metrics(), (0, 0));
        assert!(q.subscribe().unwrap() > id);
    }
    #[test]
    fn replaced_subscription_rejects_old_buffers() {
        let q = OutputQueue::default();
        let old = q.subscribe().unwrap();
        q.try_push(old, target(), OutputKind::Live, Size::default(), vec![1])
            .unwrap();
        q.subscribe().unwrap();
        assert_eq!(q.poll(old).unwrap_err(), Error::StaleGeneration);
        assert_eq!(q.metrics(), (0, 0));
    }
}
