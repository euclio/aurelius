use std::iter;

use aurelius::Server;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench(c: &mut Criterion) {
    let mut server = Server::bind("localhost:0").unwrap();

    c.bench_function("send", |b| {
        let input: String = iter::repeat("a ").take(30_000_000).collect();
        b.iter_with_large_drop(|| server.send(black_box(&input)));
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
