use bitgrid::{BitBoard, MortonLayout, RowMajorLayout};
use criterion::{Criterion, black_box, criterion_group, criterion_main};

const W: usize = 256;
const H: usize = 256;

fn bench_morphology(c: &mut Criterion) {
    let mut group = c.benchmark_group("Morphology");

    // Pre-populate boards with some shapes
    let mut rm_board = BitBoard::<W, H, RowMajorLayout>::new();
    let mut mo_board = BitBoard::<W, H, MortonLayout>::new();

    // Create a square pattern
    for y in 100..150 {
        for x in 100..150 {
            rm_board.set(x, y, true);
            mo_board.set(x, y, true);
        }
    }

    // Benchmark small and large dilations
    group.bench_function("RowMajor: Dilate r=1", |b| {
        b.iter(|| black_box(rm_board.dilate(1)))
    });
    group.bench_function("Morton: Dilate r=1", |b| {
        b.iter(|| black_box(mo_board.dilate(1)))
    });

    group.bench_function("RowMajor: Dilate r=5", |b| {
        b.iter(|| black_box(rm_board.dilate(5)))
    });
    group.bench_function("Morton: Dilate r=5", |b| {
        b.iter(|| black_box(mo_board.dilate(5)))
    });

    group.bench_function("RowMajor: Dilate r=10", |b| {
        b.iter(|| black_box(rm_board.dilate(10)))
    });
    group.bench_function("Morton: Dilate r=10", |b| {
        b.iter(|| black_box(mo_board.dilate(10)))
    });

    // Benchmark small and large erosions
    group.bench_function("RowMajor: Erode r=1", |b| {
        b.iter(|| black_box(rm_board.erode(1)))
    });
    group.bench_function("Morton: Erode r=1", |b| {
        b.iter(|| black_box(mo_board.erode(1)))
    });

    group.bench_function("RowMajor: Erode r=5", |b| {
        b.iter(|| black_box(rm_board.erode(5)))
    });
    group.bench_function("Morton: Erode r=5", |b| {
        b.iter(|| black_box(mo_board.erode(5)))
    });

    group.bench_function("RowMajor: Erode r=10", |b| {
        b.iter(|| black_box(rm_board.erode(10)))
    });
    group.bench_function("Morton: Erode r=10", |b| {
        b.iter(|| black_box(mo_board.erode(10)))
    });

    group.finish();
}

criterion_group!(benches, bench_morphology);
criterion_main!(benches);
