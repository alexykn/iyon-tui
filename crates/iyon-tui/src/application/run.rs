use std::{future::pending, time::Instant};

use crate::terminal::PresentReceipt;

pub(crate) fn wait_for_present_blocking(
    pending: &mut Option<PresentReceipt>,
) -> anyhow::Result<()> {
    let Some(receipt) = pending.take() else {
        return Ok(());
    };
    receipt
        .blocking_recv()
        .map_err(|error| anyhow::anyhow!("terminal presentation reply lost: {error}"))?
}

pub(crate) async fn wait_for_deadline(deadline: Option<Instant>) {
    let Some(deadline) = deadline else {
        pending::<()>().await;
        return;
    };
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
}
