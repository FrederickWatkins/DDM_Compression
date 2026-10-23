use std::{collections::VecDeque, sync::mpsc};

use tracing::trace;

/// A bounded standard-library channel used to connect pipeline stages.
pub struct PipelineChannel<T> {
    /// Sends items to the receiving pipeline stage.
    pub sender: mpsc::SyncSender<T>,
    /// Receives items from the sending pipeline stage.
    pub receiver: mpsc::Receiver<T>,
}

impl<T> PipelineChannel<T> {
    /// Create a bounded channel with the given capacity.
    pub fn bounded(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        Self { sender, receiver }
    }
}

pub struct PipelineInputIter<'a, I> {
    input: &'a mut I,
}

impl<'a, I> PipelineInputIter<'a, I> {
    pub fn new(input: &'a mut I) -> Self {
        Self { input }
    }
}

impl<I: PipelineInput> Iterator for PipelineInputIter<'_, I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.input.recv()
    }
}

pub trait PipelineInput {
    type Item;

    fn recv(&mut self) -> Option<Self::Item>;
}

pub trait PipelineOutput {
    type Item;

    fn send(&mut self, item: Self::Item) -> anyhow::Result<()>;
}

/// Fan a single stream into multiple. All consumers receive every item, and the stream will wait on
/// a slow consumer
///
/// # Errors
/// Passes on any errors from outputs
#[tracing::instrument(skip_all, level = "trace")]
pub fn fanout<T: Clone, I: PipelineInput<Item = T>, O: PipelineOutput<Item = T>>(
    input: &mut I,
    outputs: &mut [O],
) -> anyhow::Result<()> {
    trace!(
        thread_id = ?thread_id::get(),
        "Started fanout to {} outputs",
        outputs.len()
    );
    let mut total_packets = 0_usize;
    while let Some(item) = input.recv() {
        total_packets = total_packets.saturating_add(1);
        let Some((last, preceding)) = outputs.split_last_mut() else {
            continue;
        };
        for output in preceding {
            output.send(item.clone())?;
        }
        last.send(item)?;
    }
    trace!(
        thread_id = ?thread_id::get(),
        "Finished fanout of {total_packets} to {} outputs",
        outputs.len()
    );
    Ok(())
}

impl<T> PipelineOutput for Vec<T> {
    type Item = T;

    fn send(&mut self, item: Self::Item) -> anyhow::Result<()> {
        self.push(item);
        Ok(())
    }
}

impl<T> PipelineInput for &mut VecDeque<T> {
    type Item = T;

    fn recv(&mut self) -> Option<T> {
        self.pop_front()
    }
}

impl<T> PipelineOutput for &mut VecDeque<T> {
    type Item = T;

    fn send(&mut self, item: Self::Item) -> anyhow::Result<()> {
        self.push_back(item);
        Ok(())
    }
}

impl<T> PipelineInput for mpsc::Receiver<T> {
    type Item = T;

    fn recv(&mut self) -> Option<Self::Item> {
        #[expect(clippy::use_self, reason = "False positive")]
        mpsc::Receiver::recv(self).ok()
    }
}

impl<T: Send + Sync + 'static> PipelineOutput for mpsc::Sender<T> {
    type Item = T;

    fn send(&mut self, item: Self::Item) -> anyhow::Result<()> {
        #[expect(clippy::use_self, reason = "False positive")]
        mpsc::Sender::send(self, item).map_err(Into::into)
    }
}

impl<T: Send + Sync + 'static> PipelineOutput for mpsc::SyncSender<T> {
    type Item = T;

    fn send(&mut self, item: Self::Item) -> anyhow::Result<()> {
        #[expect(clippy::use_self, reason = "False positive")]
        mpsc::SyncSender::send(self, item).map_err(Into::into)
    }
}

#[cfg(feature = "tokio")]
impl<T> PipelineInput for tokio::sync::mpsc::Receiver<T> {
    type Item = T;

    fn recv(&mut self) -> Option<T> {
        self.blocking_recv()
    }
}

#[cfg(feature = "tokio")]
impl<T: Send + Sync + 'static> PipelineOutput for tokio::sync::mpsc::Sender<T> {
    type Item = T;

    fn send(&mut self, item: Self::Item) -> anyhow::Result<()> {
        self.blocking_send(item)?;
        Ok(())
    }
}

#[cfg(feature = "crossbeam")]
impl<T> PipelineInput for crossbeam_channel::Receiver<T> {
    type Item = T;

    fn recv(&mut self) -> Option<Self::Item> {
        #[expect(clippy::use_self, reason = "False positive")]
        crossbeam_channel::Receiver::recv(self).ok()
    }
}

#[cfg(feature = "crossbeam")]
impl<T: Send + Sync + 'static> PipelineOutput for crossbeam_channel::Sender<T> {
    type Item = T;

    fn send(&mut self, item: Self::Item) -> anyhow::Result<()> {
        #[expect(clippy::use_self, reason = "False positive")]
        crossbeam_channel::Sender::send(self, item).map_err(Into::into)
    }
}

#[cfg(feature = "flume")]
impl<T> PipelineInput for flume::Receiver<T> {
    type Item = T;

    fn recv(&mut self) -> Option<Self::Item> {
        #[expect(clippy::use_self, reason = "False positive")]
        flume::Receiver::recv(self).ok()
    }
}

#[cfg(feature = "flume")]
impl<T: Send + Sync + 'static> PipelineOutput for flume::Sender<T> {
    type Item = T;

    fn send(&mut self, item: Self::Item) -> anyhow::Result<()> {
        #[expect(clippy::use_self, reason = "False positive")]
        flume::Sender::send(self, item)
            .map_err(|_| anyhow::anyhow!("Failed to send item: all receivers have been dropped"))
    }
}
