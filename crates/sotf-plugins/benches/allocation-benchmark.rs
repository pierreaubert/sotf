use criterion::{criterion_group, criterion_main};

criterion_group!(benches, benchmark_zero_allocation);
criterion_main!(benches);

#[path = "allocation-benchmark/consts.rs"]
mod consts;
#[path = "allocation-benchmark/counting_alloc.rs"]
mod counting_alloc;
#[path = "allocation-benchmark/test.rs"]
mod test;

use test::benchmark_zero_allocation;
