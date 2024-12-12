# Enhancing Rust Streams with `Ext` Traits and Futures

While building a Rust server to serve Astro sites, I ran into something called `StreamTrait.` I had no idea what this was, or how to work with it. I hacked together a few examples and got it working, but I wanted to understand _how_ it worked.

Like most developers, I talked with a colleague at work, Googled my way forward and eventually fell into the world of `Ext` traits and `Future`s. At first, it felt like over-engineering—more tools on top of tools—but then it clicked.

In this post, I’ll share what I learned, focusing on how to extend Rust’s streams using `Ext` traits. We’ll implement a `next` method that integrates neatly with the `Future` trait, and I’ll include a clear, practical example. If you’ve ever looked at a trait and thought, _I know this is important, but I don’t know why,_ this might help. Let’s dig in.

## What Are `Ext` Traits?

An _Ext_ trait in Rust is a pattern used to extend the functionality of an existing type or trait. By defining methods in an _Ext_ trait, you can add custom behavior to types without modifying their original implementation. This is particularly useful for adding utility methods to widely used traits like _Stream_.

The _Ext_ pattern is often used because working directly with core traits like _Stream_ can be difficult. The _Stream_ trait itself defines a minimal set of low-level methods, such as `poll_next`, that are essential for implementing a stream but are not user-friendly for day-to-day use. Extension traits like _StreamExt_ provide higher-level utility methods—such as `next`, `filter`, and `map`—built on top of these low-level primitives. This makes streams much easier to work with.

In this tutorial, I'll build a custom _Ext_ trait and add `next`, `map`, and `take` methods to _Stream. These methods will return a _Future_ that resolves to the next item in the stream.

## Building the `StreamExt` Trait

Let's start by defining the `StreamExt` trait. It extends the `Stream` trait with a `next`, `map` and `take` methods:

```rust
use futures_core::stream::Stream;

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
```

Here’s what’s happening:

1. The `StreamExt` trait defines a `next` method that returns a `Next` struct.
2. It also defines a `map` method, which takes a closure and applies it to each item in the stream.
3. The `take` method limits the number of items produced by a stream.
4. We implement `StreamExt` for all types that implement `Stream`, ensuring broad compatibility.

## Introducing the `Next` Future

The `next` method returns a `Next` struct, which implements the `Future` trait. This is where the magic happens:

```rust
use futures_core::task::{Context, Poll};
use std::pin::Pin;

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
```

Key points:

1. `Next` holds a mutable reference to the stream.
2. It implements the `Future` trait, with `Output` being `Option<S::Item>`.
3. The `poll` method drives the stream’s `poll_next` method to retrieve the next item.

## Introducing the `Map` Adapter

The `map` method transforms each item in the stream using a provided closure. Here’s how it works:

```rust
// Stream adapter for `map`
pub struct Map<S, F> {
    stream: S,
    f: F,
}

impl<S, F, T> Stream for Map<S, F>
where
    S: Stream + std::marker::Unpin,
    F: FnMut(S::Item) -> T + std::marker::Unpin,
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
```

Key points:

1. `Map` wraps a stream and a closure.
2. It implements `Stream`, transforming each item from the inner stream using the closure.
3. The `poll_next` method applies the closure to each item as it is polled.

## Introducing the `Take` Adapter

The `take` method limits the number of items a stream produces. Here’s how it works:

```rust
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
```

Key points:

1. `Take` tracks the number of remaining items to produce.
2. It stops producing items when the count reaches zero.
3. It passes through items from the inner stream until the limit is reached.

## Using the `next`, `map`, and `take` Methods

Now that we have our `StreamExt` trait with `next`, `map`, and `take`, let's see them in action:

```rust
use futures::stream;

#[tokio::main]
async fn main() {
    let mut stream = stream::repeat_with(|| 42).take(10).map(|x| x * 2);

    // Using the `next` method from our custom `StreamExt`
    while let Some(item) = stream.next().await {
        println!("{}", item);
    }
}
```

Here’s what’s happening:

1. We create a stream that repeatedly yields the number `42`.
2. We use the `take` method to limit the stream to 10 items, ensuring it terminates.
3. We use the `map` method to multiply each item by 2.
4. We use the `next` method to retrieve items from the transformed stream one at a time.
5. The `.await` keyword drives the futures to completion, printing each transformed item.

## Why Use `.await`?

Finally, you might wonder why `.await` is necessary. The answer lies in the asynchronous nature of streams:

1. The `next` method returns a `Next` struct, which implements `Future`.
2. Calling `.await` drives this future to completion, executing its `poll` method.
3. The `poll` method, in turn, invokes `poll_next` on the underlying stream, retrieving the next item asynchronously.

## Recap and Takeaways

By combining `Ext` traits with futures, we can create ergonomic and powerful abstractions for working with streams in Rust. I've pushed this example to GitHub if you want to try it out yourself.
