//! Admission control covers blocking work and the complete lifetime of streamed responses.
use futures::{stream::BoxStream, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::{timeout_at, Instant};
use tonic::Status;

/// Server-controlled limits; tickets cannot increase these values.
#[derive(Debug, Clone)]
pub struct ServiceLimits {
    /// Simultaneous requests, including responses not yet consumed (default 4).
    pub concurrent_requests: usize,
    /// Maximum command/ticket/criteria payload (default 64 KiB).
    pub request_bytes: usize,
    /// Output columns per request (default 1024); repeated names are rejected.
    pub output_columns: usize,
    /// Conservative output buffer budget per batch (default 8 MiB).
    pub batch_bytes: usize,
    /// Encoded metadata response budget, including all listed files/endpoints (default 8 MiB).
    pub response_bytes: usize,
    /// Maximum duration including response consumption (default 30 seconds).
    pub request_timeout: Duration,
    /// Maximum directory entries visited by a listing (default 10000).
    pub listing_entries: usize,
    /// Maximum files returned by a listing (default 1000).
    pub listing_files: usize,
}

impl Default for ServiceLimits {
    fn default() -> Self {
        Self {
            concurrent_requests: 4,
            request_bytes: 64 * 1024,
            output_columns: 1024,
            batch_bytes: 8 * 1024 * 1024,
            response_bytes: 8 * 1024 * 1024,
            request_timeout: Duration::from_secs(30),
            listing_entries: 10_000,
            listing_files: 1000,
        }
    }
}

impl ServiceLimits {
    pub(crate) fn validate(&self) -> std::io::Result<()> {
        if self.concurrent_requests == 0
            || self.concurrent_requests > Semaphore::MAX_PERMITS
            || self.request_bytes == 0
            || self.output_columns == 0
            || self.batch_bytes == 0
            || self.response_bytes == 0
            || self.listing_entries == 0
            || self.listing_files == 0
            || self.request_timeout.is_zero()
            || self.request_timeout > Duration::from_secs(3600)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid Flight resource limits (timeout must be <= 1 hour)",
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct RequestBudget {
    permit: Arc<OwnedSemaphorePermit>,
    pub(crate) deadline: Instant,
}

impl RequestBudget {
    pub(crate) fn acquire(slots: &Arc<Semaphore>, timeout: Duration) -> Result<Self, Status> {
        let permit = Arc::clone(slots)
            .try_acquire_owned()
            .map_err(|_| Status::resource_exhausted("trop de requêtes simultanées"))?;
        Ok(Self {
            permit: Arc::new(permit),
            deadline: Instant::now() + timeout,
        })
    }

    pub(crate) async fn blocking<T, F>(&self, f: F) -> Result<T, Status>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, Status> + Send + 'static,
    {
        let permit = Arc::clone(&self.permit);
        // A timed-out spawn_blocking task cannot be forcibly stopped. It retains its slot until
        // it actually exits, so timeouts cannot be used to create unlimited background work.
        timeout_at(
            self.deadline,
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                f()
            }),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("durée maximale dépassée"))?
        .map_err(super::internal)?
    }

    pub(crate) fn stream<T: Send + 'static>(
        self,
        stream: BoxStream<'static, Result<T, Status>>,
    ) -> BoxStream<'static, Result<T, Status>> {
        futures::stream::unfold(Some((stream, self)), |state| async move {
            let (mut stream, budget) = state?;
            match timeout_at(budget.deadline, stream.next()).await {
                Ok(Some(item)) => {
                    if item.is_err() {
                        Some((item, None))
                    } else {
                        Some((item, Some((stream, budget))))
                    }
                }
                Ok(None) => None,
                Err(_) => Some((
                    Err(Status::deadline_exceeded("durée maximale dépassée")),
                    None,
                )),
            }
        })
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stream_keeps_its_slot_and_dropping_it_releases_admission() {
        let slots = Arc::new(Semaphore::new(1));
        let budget = RequestBudget::acquire(&slots, Duration::from_secs(1)).unwrap();
        let stream = budget.stream(futures::stream::pending::<Result<(), Status>>().boxed());
        assert_eq!(
            RequestBudget::acquire(&slots, Duration::from_secs(1))
                .err()
                .unwrap()
                .code(),
            tonic::Code::ResourceExhausted
        );
        drop(stream);
        assert!(RequestBudget::acquire(&slots, Duration::from_secs(1)).is_ok());
    }

    #[tokio::test]
    async fn stalled_stream_expires_and_releases_its_slot() {
        let slots = Arc::new(Semaphore::new(1));
        let budget = RequestBudget::acquire(&slots, Duration::from_millis(10)).unwrap();
        let mut stream = budget.stream(futures::stream::pending::<Result<(), Status>>().boxed());
        let error = stream.next().await.unwrap().unwrap_err();
        assert_eq!(error.code(), tonic::Code::DeadlineExceeded);
        assert_eq!(slots.available_permits(), 1);
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn timed_out_blocking_work_retains_its_slot_until_it_really_stops() {
        let slots = Arc::new(Semaphore::new(1));
        let budget = RequestBudget::acquire(&slots, Duration::from_millis(10)).unwrap();
        let (release, wait) = std::sync::mpsc::channel();
        let error = budget
            .blocking(move || {
                let _ = wait.recv();
                Ok(())
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), tonic::Code::DeadlineExceeded);
        drop(budget);
        assert_eq!(slots.available_permits(), 0);
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while slots.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
}
