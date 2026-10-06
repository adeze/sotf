use symphonia::core::formats::FormatReader;
use symphonia::core::meta::Metadata;

pub fn get_latest_metadata(format: &mut dyn FormatReader) -> Option<Metadata<'_>> {
    // Symphonia 0.6 queues probe metadata directly on the format reader.
    let mut metadata = format.metadata();
    metadata.skip_to_latest()?;
    Some(metadata)
}
