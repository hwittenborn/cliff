use std::future::Future;

/// A trait for awaiting a future in a non-async context. This is automatically implemented for anything implementing [`Future`].
pub trait BlockAwait<F: Future> {
    /// The output of running the future.
    type Output;

    /// Block until the future completes by passing the future to [`block_on`].
    fn block_await(self) -> Self::Output;
}

impl<F: Future> BlockAwait<F> for F {
    type Output = F::Output;

    fn block_await(self) -> Self::Output {
        futures::executor::block_on(self)
    }
}
