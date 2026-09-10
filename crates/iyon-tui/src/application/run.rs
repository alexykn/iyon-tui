use std::{future::pending, time::Instant};

pub(crate) async fn wait_for_deadline(deadline: Option<Instant>) {
    let Some(deadline) = deadline else {
        pending::<()>().await;
        return;
    };
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
}
