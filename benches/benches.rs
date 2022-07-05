use criterion::BenchmarkId;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tokio::runtime;

use aurelius::Server;

fn from_elem(c: &mut Criterion) {
    let size: usize = 1024;

    c.bench_with_input(BenchmarkId::new("small send", size), &size, |b, _| {
        let runtime = runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        let addr = "127.0.0.1:0".parse().unwrap();

        let server = runtime.block_on(async { Server::bind(&addr).await.unwrap() });

        b.to_async(&runtime).iter(|| async {
            server.send(black_box("Hello, world!")).await.unwrap();
        });
    });

    c.bench_with_input(BenchmarkId::new("large send", size), &size, |b, _| {
        let runtime = runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        let addr = "127.0.0.1:0".parse().unwrap();

        let server = runtime.block_on(async { Server::bind(&addr).await.unwrap() });

        let markdown = "a ".repeat(100_000);

        b.to_async(&runtime).iter(|| async {
            server.send(black_box(&markdown)).await.unwrap();
        });
    });
}

criterion_group!(benches, from_elem);
criterion_main!(benches);
