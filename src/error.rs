use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MapforgeError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid magic bytes, expected 'mapsforge binary OSM'")]
    InvalidMagic,

    #[error("Unsupported file version: {0}, expected version 3 or higher")]
    UnsupportedVersion(u32),

    #[error("Invalid bounding box values")]
    InvalidBoundingBox,

    #[error("Invalid header size: {0}")]
    InvalidHeaderSize(u32),

    #[error("Invalid tile index signature")]
    InvalidTileIndexSignature,

    #[error("Invalid tile signature")]
    InvalidTileSignature,

    #[error("Invalid tile POI signature")]
    InvalidTilePOISignature,

    #[error("Invalid tile data")]
    InvalidTileData,

    #[error("Tile out of bound")]
    TileOutOfBounds,

    #[error("Invalid way signature")]
    InvalidWaySignature,

    #[error("Zoom level not supported")]
    ZoomLevelNotSupported,

    #[error("Errrrrr")]
    InvalidVBEInteger,
}
