# Lexaos BitBoard

A high-performance, flexible bitboard and bitgrid library for Rust, designed for spatial queries, pathfinding, and visibility calculations.

[![Crates.io](https://img.shields.io/crates/v/lexaos_bitboard.svg)](https://crates.io/crates/lexaos_bitboard)
[![Docs.rs](https://docs.rs/lexaos_bitboard/badge.svg)](https://docs.rs/lexaos_bitboard)

## Features

- **Const Generics**: Board dimensions ($W \times H$) are fixed at the type level for maximum performance and zero-cost abstractions.
- **Custom Memory Layouts**:
  - **Row-Major**: Standard row-aligned layout, optimized for horizontal/vertical operations.
  - **Morton Order (Z-order curve)**: Optimized for spatial locality and 2D caching.
- **Hierarchical Mask (L1 Mask)**: Fast skipping of empty regions during iteration and bitwise operations.
- **Advanced Spatial Operations**:
  - **Visibility**: Fast recursive shadowcasting for Field of View (FOV).
  - **Morphology**: Optimized dilation and erosion.
  - **Bulk Operations**: Efficient rectangular and circular (sector) masking.
- **No-std Friendly**: Core logic has zero external dependencies (except for optional `serde` support).

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
lexaos_bitboard = "0.1.0"
```

## Quick Start

```rust
use lexaos_bitboard::{BitBoard, RowMajorLayout};

fn main() {
    // Create a 256x256 bitboard with the default Row-Major layout
    let mut board = BitBoard::<256, 256>::default();

    // Set and get bits using coordinates
    board.set(10, 20, true);
    assert!(board.get(10, 20));

    // Perform bitwise operations
    let other = BitBoard::<256, 256>::mask_rect(5, 5, 20, 20);
    let intersection = &board & &other;

    // Iterate over set bits efficiently
    for (x, y) in intersection.iter_set_bits() {
        println!("Set bit at ({}, {})", x, y);
    }
}
```

## Advanced Usage

### Field of View (Visibility)

```rust
let opaque = BitBoard::<128, 128>::mask_rect(50, 50, 2, 20); // A wall
let fov = BitBoard::mask_visibility(40, 60, 15.0, &opaque);

if fov.get(60, 60) {
    println!("Target is visible!");
}
```

### Morphology (Dilation & Erosion)

```rust
let mut obstacles = BitBoard::<128, 128>::new();
obstacles.set(64, 64, true);

// Expand obstacles by 2 tiles in all directions (useful for unit pathfinding)
let expanded = obstacles.dilate(2);
```

### Coordinate Conversion

```rust
use lexaos_bitboard::RowMajorLayout as L;

// Convert continuous world points to discrete grid coordinates
let (x, y) = L::point_to_coord((10.5, 20.9));
assert_eq!((x, y), (10, 20));

// Convert back to world points (center of the tile)
let (px, py) = L::coord_to_point(10, 20);
assert_eq!((px, py), (10.0, 20.0));
```

## Performance

Lexaos BitBoard is built for speed. It uses a 2-level hierarchical mask to skip empty 64-bit words, making operations on sparse boards significantly faster than simple bit arrays. 

Bitwise operations (AND, OR, XOR, NOT) and shifts are highly optimized for both Row-Major and Morton layouts, leveraging modern CPU bit-manipulation instructions.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
