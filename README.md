# mapsforge-rs

A Rust parser for Mapsforge binary map files. This library provides functionality to read and parse Mapsforge map files (.map), which are commonly used for offline mapping applications.

## Features

- [x] Map header parsing
  - Magic bytes validation
  - Version checking
  - Bounding box parsing
- [x] Map tile parsing
- [x] POI data structure
- [x] Sub-file structure parsing 

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mapsforge-rs = "0.1.0"
```

## Usage

### Reading Map Header

```rust
use std::fs::File;
use std::io::BufReader;
use mapsforge_rs::MapHeader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open and read the map file
    let file = File::open("path/to/map.map")?;
    let mut reader = BufReader::new(file);
    
    // Parse the header
    let header = MapHeader::read_from_file(&mut reader)?;
    
    // Access header information
    println!("Map bounds: {:?}", header.bounding_box);
    println!("File version: {}", header.file_version);
    println!("Tile size: {}", header.tile_size);
    
    Ok(())
}
```



## Requirements

- Rust 1.56 or higher
- `byteorder` crate for handling endianness
- No other external dependencies required

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Based on the [Mapsforge binary format](https://github.com/mapsforge/mapsforge/blob/master/docs/Specification-Binary-Map-File.md)
- Inspired by the Java implementation in the main Mapsforge project

## Development Status

This project is in active development. Current focus is on implementing basic file parsing capabilities.
