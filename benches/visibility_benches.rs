use bitgrid::{BitBoard, MortonLayout, RowMajorLayout};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rand::{Rng, SeedableRng, rngs::StdRng};

const W: usize = 256;
const H: usize = 256;

fn bench_visibility(c: &mut Criterion) {
    let mut group = c.benchmark_group("Visibility");

    // Empty board
    let empty_rm = BitBoard::<W, H, RowMajorLayout>::new();
    let empty_mo = BitBoard::<W, H, MortonLayout>::new();

    // Board with random obstacles (approx 10% density)
    let mut rng = StdRng::seed_from_u64(42);
    let mut obstacles_rm = BitBoard::<W, H, RowMajorLayout>::new();
    let mut obstacles_mo = BitBoard::<W, H, MortonLayout>::new();
    for _ in 0..(W * H / 10) {
        let x = rng.gen_range(0..W);
        let y = rng.gen_range(0..H);
        obstacles_rm.set(x as i32, y as i32, true);
        obstacles_mo.set(x as i32, y as i32, true);
    }

    let origin_x = (W / 2) as i32;
    let origin_y = (H / 2) as i32;
    let radius = 30.0;

    group.bench_function("RowMajor: FOV (Empty)", |b| {
        b.iter(|| {
            black_box(BitBoard::<W, H, RowMajorLayout>::mask_visibility(
                origin_x, origin_y, radius, &empty_rm,
            ))
        })
    });
    group.bench_function("Morton: FOV (Empty)", |b| {
        b.iter(|| {
            black_box(BitBoard::<W, H, MortonLayout>::mask_visibility(
                origin_x, origin_y, radius, &empty_mo,
            ))
        })
    });

    group.bench_function("RowMajor: FOV (Obstacles)", |b| {
        b.iter(|| {
            black_box(BitBoard::<W, H, RowMajorLayout>::mask_visibility(
                origin_x,
                origin_y,
                radius,
                &obstacles_rm,
            ))
        })
    });
    group.bench_function("Morton: FOV (Obstacles)", |b| {
        b.iter(|| {
            black_box(BitBoard::<W, H, MortonLayout>::mask_visibility(
                origin_x,
                origin_y,
                radius,
                &obstacles_mo,
            ))
        })
    });

    group.finish();
}

fn bench_masking(c: &mut Criterion) {
    let mut group = c.benchmark_group("Masking");

    group.bench_function("RowMajor: Rect Mask", |b| {
        b.iter(|| {
            black_box(BitBoard::<W, H, RowMajorLayout>::mask_rect(
                10, 10, 100, 100,
            ))
        })
    });
    group.bench_function("Morton: Rect Mask", |b| {
        b.iter(|| black_box(BitBoard::<W, H, MortonLayout>::mask_rect(10, 10, 100, 100)))
    });

    group.bench_function("RowMajor: Sector Mask", |b| {
        b.iter(|| {
            black_box(BitBoard::<W, H, RowMajorLayout>::mask_sector(
                128, 128, 50.0, 0.0, 360.0,
            ))
        })
    });
    group.bench_function("Morton: Sector Mask", |b| {
        b.iter(|| {
            black_box(BitBoard::<W, H, MortonLayout>::mask_sector(
                128, 128, 50.0, 0.0, 360.0,
            ))
        })
    });

    group.finish();
}

criterion_group!(benches, bench_visibility, bench_masking);
criterion_main!(benches);
