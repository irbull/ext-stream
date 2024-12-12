use futures::stream;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use std::pin::Pin;

// Extension trait
pub trait StreamExt: Stream {
    fn next(&mut self) -> Next<'_, Self>
    where
        Self: Unpin,
    {
        Next { stream: self }
    }

    fn map<F, T>(self, f: F) -> Map<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> T,
    {
        Map { stream: self, f }
    }

    fn take(self, n: usize) -> Take<Self>
    where
        Self: Sized,
    {
        Take {
            stream: self,
            remaining: n,
        }
    }
}

// Implement the extension trait for all types that implement Stream
impl<T: ?Sized> StreamExt for T where T: Stream {}

// Future returned by the `next` method
pub struct Next<'a, S: ?Sized> {
    stream: &'a mut S,
}

impl<S: Stream + Unpin + ?Sized> std::future::Future for Next<'_, S> {
    type Output = Option<S::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let stream = Pin::new(&mut *self.get_mut().stream);
        stream.poll_next(cx)
    }
}

// Stream adapter for `map`
pub struct Map<S, F> {
    stream: S,
    f: F,
}

impl<S, F, T> Stream for Map<S, F>
where
    S: Stream + Unpin,
    F: FnMut(S::Item) -> T + Unpin,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Pin::new(&mut this.stream).poll_next(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some((this.f)(item))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

// Stream adapter for `take`
pub struct Take<S> {
    stream: S,
    remaining: usize,
}

impl<S: Stream + Unpin> Stream for Take<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.remaining == 0 {
            Poll::Ready(None)
        } else {
            match Pin::new(&mut this.stream).poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    this.remaining -= 1;
                    Poll::Ready(Some(item))
                }
                other => other,
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let mut counter: u32 = 0;

    let stream = stream::repeat_with(|| {
        counter += 1;
        counter
    }).take(10).map(|x| x * 2);

    tokio::pin!(stream);

    // Using the `next` method from our custom `StreamExt`
    while let Some(item) = stream.next().await {
        println!("{item}");
    }
}
