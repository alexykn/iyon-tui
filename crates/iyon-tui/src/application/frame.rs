//! Exact native presentation state for one host.
//!
//! The state is deliberately separate from desired occurrence data. A
//! prepared frame owns the captured stamps, derived scene products and
//! completion plans; receipt completion may therefore publish an older frame
//! without clearing newer desired work.

use crate::application::environment::ReceiptWake;
use crate::scene::PreparedSceneFrame;

use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

use super::content::PreparedContentCommit;

/// A receipt is the single owner of the native oneshot receiver. Polling it
/// installs the queue-only waker; the sender's completion then re-admits the
/// qualified host without a waiter thread or a duplicate result slot.
pub(crate) struct PresentReceipt {
    receiver: tokio::sync::oneshot::Receiver<anyhow::Result<()>>,
    wake: ReceiptWake,
}

/// Receipt for one asynchronous physical History insertion. The worker owns
/// terminal I/O; the host only polls this owned acknowledgement and applies
/// the captured logical prefix after it succeeds.
pub(crate) struct HistoryReceipt {
    receiver: tokio::sync::oneshot::Receiver<anyhow::Result<usize>>,
    wake: ReceiptWake,
}

impl HistoryReceipt {
    pub(crate) fn from_receiver(
        receiver: tokio::sync::oneshot::Receiver<anyhow::Result<usize>>,
        wake: ReceiptWake,
    ) -> Self {
        Self { receiver, wake }
    }

    pub(crate) fn poll(&mut self) -> Poll<anyhow::Result<usize>> {
        let waker = Waker::from(Arc::new(self.wake.clone()));
        let mut context = Context::from_waker(&waker);
        match Pin::new(&mut self.receiver).poll(&mut context) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(_)) => Poll::Ready(Err(anyhow::anyhow!("terminal History reply lost"))),
            Poll::Pending => Poll::Pending,
        }
    }

    pub(crate) fn blocking_recv(self) -> anyhow::Result<usize> {
        self.receiver
            .blocking_recv()
            .map_err(|_| anyhow::anyhow!("terminal History reply lost"))?
    }
}

impl PresentReceipt {
    pub(crate) fn from_receiver(
        receiver: tokio::sync::oneshot::Receiver<anyhow::Result<()>>,
        wake: ReceiptWake,
    ) -> Self {
        Self { receiver, wake }
    }

    pub(crate) fn poll(&mut self) -> Poll<anyhow::Result<()>> {
        let waker = Waker::from(Arc::new(self.wake.clone()));
        let mut context = Context::from_waker(&waker);
        match Pin::new(&mut self.receiver).poll(&mut context) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(_)) => {
                Poll::Ready(Err(anyhow::anyhow!("terminal presentation reply lost")))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    pub(crate) fn blocking_recv(self) -> anyhow::Result<()> {
        blocking_receive(self.receiver)
    }
}

pub(crate) fn blocking_receive(
    receiver: tokio::sync::oneshot::Receiver<anyhow::Result<()>>,
) -> anyhow::Result<()> {
    receiver
        .blocking_recv()
        .map_err(|_| anyhow::anyhow!("terminal presentation reply lost"))?
}

#[derive(Debug)]
pub(crate) enum PreparedFrameProduct {
    /// A scene prepared for backend submission.
    Scene {
        products: PreparedSceneProducts,
        physical_frame_id: u64,
    },
    /// A real prepared scene whose physical bytes are proven equal to the
    /// confirmed surface. This preserves the old surface comparison path
    /// without conflating it with metadata-only completion.
    NoOutput {
        products: PreparedSceneProducts,
        physical_frame_id: u64,
    },
    /// No scene was prepared. The candidate references only the exact
    /// confirmed physical frame and promotes metadata/barriers locally.
    Metadata { physical_frame_id: u64 },
}

#[derive(Debug)]
pub(crate) struct PreparedSceneProducts {
    pub(crate) scene: PreparedSceneFrame,
    pub(crate) content: PreparedContentCommit,
    pub(crate) content_dirty_epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SceneDisposition {
    Submit,
    NoOutput,
}

#[derive(Debug)]
pub(crate) struct PreparedFrame {
    pub(crate) product: PreparedFrameProduct,
    pub(crate) ui_revision: u64,
    pub(crate) work_epoch: u64,
    pub(crate) structural_revision: u64,
}

impl PreparedFrame {
    pub(crate) fn with_scene(
        products: PreparedSceneProducts,
        disposition: SceneDisposition,
        physical_frame_id: u64,
        ui_revision: u64,
        work_epoch: u64,
        structural_revision: u64,
    ) -> Self {
        Self {
            product: match disposition {
                SceneDisposition::Submit => PreparedFrameProduct::Scene {
                    products,
                    physical_frame_id,
                },
                SceneDisposition::NoOutput => PreparedFrameProduct::NoOutput {
                    products,
                    physical_frame_id,
                },
            },
            ui_revision,
            work_epoch,
            structural_revision,
        }
    }

    pub(crate) fn metadata(
        physical_frame_id: u64,
        ui_revision: u64,
        work_epoch: u64,
        structural_revision: u64,
    ) -> Self {
        Self {
            product: PreparedFrameProduct::Metadata { physical_frame_id },
            ui_revision,
            work_epoch,
            structural_revision,
        }
    }

    pub(crate) fn is_no_output(&self) -> bool {
        matches!(
            &self.product,
            PreparedFrameProduct::NoOutput { .. } | PreparedFrameProduct::Metadata { .. }
        )
    }

    pub(crate) fn physical_frame_id(&self) -> u64 {
        match &self.product {
            PreparedFrameProduct::Scene {
                physical_frame_id, ..
            }
            | PreparedFrameProduct::NoOutput {
                physical_frame_id, ..
            }
            | PreparedFrameProduct::Metadata { physical_frame_id } => *physical_frame_id,
        }
    }

    pub(crate) fn scene(&self) -> Option<&PreparedSceneFrame> {
        match &self.product {
            PreparedFrameProduct::Scene { products, .. }
            | PreparedFrameProduct::NoOutput { products, .. } => Some(&products.scene),
            PreparedFrameProduct::Metadata { .. } => None,
        }
    }

    pub(crate) fn content(&self) -> Option<&PreparedContentCommit> {
        match &self.product {
            PreparedFrameProduct::Scene { products, .. }
            | PreparedFrameProduct::NoOutput { products, .. } => Some(&products.content),
            PreparedFrameProduct::Metadata { .. } => None,
        }
    }

    pub(crate) fn content_dirty_epoch(&self) -> Option<u64> {
        match &self.product {
            PreparedFrameProduct::Scene { products, .. }
            | PreparedFrameProduct::NoOutput { products, .. } => Some(products.content_dirty_epoch),
            PreparedFrameProduct::Metadata { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrameFailure {
    pub(crate) phase: &'static str,
    pub(crate) code: &'static str,
    pub(crate) attempted_ui_revision: u64,
    pub(crate) attempted_work_epoch: u64,
    pub(crate) retryable: bool,
    pub(crate) diagnostic: String,
}

/// One typed notification from the native scheduler, content projector or
/// presentation backend. The automatic observer consumes these records
/// directly; it does not poll a status table or drive another frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiFailureNotification {
    pub phase: String,
    pub code: String,
    pub attempted_ui_revision: u64,
    pub attempted_work_epoch: u64,
    pub diagnostic: String,
    pub retryable: bool,
}

impl From<&FrameFailure> for UiFailureNotification {
    fn from(failure: &FrameFailure) -> Self {
        Self {
            phase: failure.phase.to_owned(),
            code: failure.code.to_owned(),
            attempted_ui_revision: failure.attempted_ui_revision,
            attempted_work_epoch: failure.attempted_work_epoch,
            diagnostic: failure.diagnostic.clone(),
            retryable: failure.retryable,
        }
    }
}

pub(crate) enum PresentationState {
    Idle,
    Prepared(PreparedFrame),
    InFlight {
        frame: PreparedFrame,
        receipt: PresentReceipt,
    },
    Completing {
        frame: PreparedFrame,
    },
    Failed(FrameFailure),
    Closed,
}

impl PresentationState {
    pub(crate) fn prepared(frame: PreparedFrame) -> Self {
        Self::Prepared(frame)
    }

    pub(crate) fn frame(&self) -> Option<&PreparedFrame> {
        match self {
            Self::Prepared(frame) | Self::InFlight { frame, .. } | Self::Completing { frame } => {
                Some(frame)
            }
            Self::Idle | Self::Failed(_) | Self::Closed => None,
        }
    }

    pub(crate) fn is_in_flight(&self) -> bool {
        matches!(self, Self::InFlight { .. })
    }
}
