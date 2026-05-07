use bitgrid::{BitBoard, MortonLayout, RowMajorLayout};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rand::{Rng, SeedableRng, rngs::StdRng};

const W: usize = 256;
const H: usize = 256;

fn bench_set_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("Basic Operations");

    group.bench_function("RowMajor: Random Set/Get", |b| {
        let mut rng = StdRng::seed_from_u64(42);
        let mut board = BitBoard::<W, H, RowMajorLayout>::new();
        b.iter(|| {
            let x = rng.gen_range(0..W);
            let y = rng.gen_range(0..H);
            board.set(x as i32, y as i32, true);
            black_box(board.get(black_box(x as i32), black_box(y as i32)));
        });
    });

    group.bench_function("Morton: Random Set/Get", |b| {
        let mut rng = StdRng::seed_from_u64(42);
        let mut board = BitBoard::<W, H, MortonLayout>::new();
        b.iter(|| {
            let x = rng.gen_range(0..W);
            let y = rng.gen_range(0..H);
            board.set(x as i32, y as i32, true);
            black_box(board.get(black_box(x as i32), black_box(y as i32)));
        });
    });

    group.finish();
}

fn bench_bitwise(c: &mut Criterion) {
    let mut group = c.benchmark_group("Bitwise Operations");

    // Pre-populate some boards
    let mut rng = StdRng::seed_from_u64(42);
    let mut rm_a = BitBoard::<W, H, RowMajorLayout>::new();
    let mut rm_b = BitBoard::<W, H, RowMajorLayout>::new();
    let mut mo_a = BitBoard::<W, H, MortonLayout>::new();
    let mut mo_b = BitBoard::<W, H, MortonLayout>::new();

    for _ in 0..10000 {
        let x1 = rng.gen_range(0..W);
        let y1 = rng.gen_range(0..H);
        let x2 = rng.gen_range(0..W);
        let y2 = rng.gen_range(0..H);
        rm_a.set(x1 as i32, y1 as i32, true);
        mo_a.set(x1 as i32, y1 as i32, true);
        rm_b.set(x2 as i32, y2 as i32, true);
        mo_b.set(x2 as i32, y2 as i32, true);
    }

    group.bench_function("RowMajor: AND", |b| b.iter(|| black_box(&rm_a & &rm_b)));
    group.bench_function("Morton: AND", |b| b.iter(|| black_box(&mo_a & &mo_b)));

    group.bench_function("RowMajor: OR", |b| b.iter(|| black_box(&rm_a | &rm_b)));
    group.bench_function("Morton: OR", |b| b.iter(|| black_box(&mo_a | &mo_b)));

    group.finish();
}

fn bench_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("Iteration");

    let mut rng = StdRng::seed_from_u64(42);

    // Sparse boards
    let mut sparse_rm = BitBoard::<W, H, RowMajorLayout>::new();
    let mut sparse_mo = BitBoard::<W, H, MortonLayout>::new();
    for _ in 0..100 {
        let x = rng.gen_range(0..W);
        let y = rng.gen_range(0..H);
        sparse_rm.set(x as i32, y as i32, true);
        sparse_mo.set(x as i32, y as i32, true);
    }

    // Dense boards
    let mut dense_rm = BitBoard::<W, H, RowMajorLayout>::new();
    let mut dense_mo = BitBoard::<W, H, MortonLayout>::new();
    for _ in 0..(W * H / 2) {
        let x = rng.gen_range(0..W);
        let y = rng.gen_range(0..H);
        dense_rm.set(x as i32, y as i32, true);
        dense_mo.set(x as i32, y as i32, true);
    }

    group.bench_function("RowMajor: Iter Sparse", |b| {
        b.iter(|| {
            for pos in sparse_rm.iter_set_bits() {
                black_box(pos);
            }
        })
    });

    group.bench_function("Morton: Iter Sparse", |b| {
        b.iter(|| {
            for pos in sparse_mo.iter_set_bits() {
                black_box(pos);
            }
        })
    });

    group.bench_function("RowMajor: Iter Dense", |b| {
        b.iter(|| {
            for pos in dense_rm.iter_set_bits() {
                black_box(pos);
            }
        })
    });

    group.bench_function("Morton: Iter Dense", |b| {
        b.iter(|| {
            for pos in dense_mo.iter_set_bits() {
                black_box(pos);
            }
        })
    });

    group.finish();
}

criterion_group!(benches, bench_set_get, bench_bitwise, bench_iteration);
criterion_main!(benches);
