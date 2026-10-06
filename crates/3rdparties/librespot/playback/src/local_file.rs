use crate::symphonia_util;
use librespot_core::{Error, SpotifyUri};
use std::{
    collections::HashMap,
    fs,
    fs::File,
    io,
    path::{Path, PathBuf},
    time::Duration,
};
use symphonia::core::{
    formats::probe::Hint,
    formats::{FormatOptions, FormatReader, TrackType},
    io::MediaSourceStream,
    meta::{MetadataOptions, StandardTag, Tag},
};

// "Spotify supports .mp3, .mp4, and .m4p files. It doesn’t support .mp4 files that contain video,
// or the iTunes lossless format (M4A)."
// https://community.spotify.com/t5/FAQs/Local-Files/ta-p/5186118
//
// There are some indications online that FLAC is supported, so check for this as well.
const SUPPORTED_FILE_EXTENSIONS: &[&str; 4] = &["mp3", "mp4", "m4p", "flac"];

#[derive(Default)]
pub struct LocalFileLookup(HashMap<SpotifyUri, PathBuf>);

impl LocalFileLookup {
    pub fn get(&self, uri: &SpotifyUri) -> Option<&Path> {
        self.0.get(uri).map(|p| p.as_path())
    }
}

pub fn create_local_file_lookup(directories: &[PathBuf]) -> LocalFileLookup {
    let mut lookup = LocalFileLookup(HashMap::new());

    for path in directories {
        if !path.is_dir() {
            warn!(
                "Ignoring local file source {}: not a directory",
                path.display()
            );
            continue;
        }

        if let Err(e) = visit_dir(path, &mut lookup) {
            warn!(
                "Failed to load entries from local file source {}: {}",
                path.display(),
                e
            );
        }
    }

    lookup
}

fn visit_dir(dir: &Path, accumulator: &mut LocalFileLookup) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            visit_dir(&path, accumulator)?;
        } else {
            let Some(file_extension) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };

            let lowercase_extension = file_extension.to_lowercase();

            if SUPPORTED_FILE_EXTENSIONS.contains(&lowercase_extension.as_str()) {
                let uri = match get_uri_from_file(path.as_path(), file_extension) {
                    Ok(uri) => uri,
                    Err(e) => {
                        warn!(
                            "Failed to determine URI of local file {}: {}",
                            path.display(),
                            e
                        );
                        continue;
                    }
                };

                accumulator.0.insert(uri, path);
            }
        }
    }

    Ok(())
}

fn get_uri_from_file(audio_path: &Path, file_extension: &str) -> Result<SpotifyUri, Error> {
    let src = File::open(audio_path)?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let mut hint = Hint::new();
    hint.with_extension(file_extension);

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, fmt_opts, meta_opts)
        .map_err(|_| Error::internal("Failed to probe file"))?;

    let mut artist: Option<String> = None;
    let mut album_title: Option<String> = None;
    let mut track_title: Option<String> = None;

    fn get_tags(format: &mut dyn FormatReader) -> Vec<Tag> {
        symphonia_util::get_latest_metadata(format)
            .and_then(|metadata| {
                metadata
                    .current()
                    .map(|revision| revision.media.tags.clone())
            })
            .unwrap_or_default()
    }

    for tag in get_tags(format.as_mut()) {
        match tag.std {
            Some(StandardTag::Album(value)) => album_title = Some((*value).clone()),
            Some(StandardTag::Artist(value)) => artist = Some((*value).clone()),
            Some(StandardTag::TrackTitle(value)) => track_title = Some((*value).clone()),
            _ => (),
        }
    }

    let first_track = format
        .default_track(TrackType::Audio)
        .ok_or(Error::internal("Failed to find an audio track"))?;

    let time_base = first_track
        .time_base
        .ok_or(Error::internal("Failed to calculate track duration"))?;

    let duration = first_track
        .duration
        .ok_or(Error::internal("Failed to calculate track duration"))?;

    let time = time_base
        .calc_duration(duration)
        .ok_or(Error::internal("Track duration is out of range"))?;

    fn format_uri_part(input: Option<String>) -> String {
        input
            .map(|s| {
                let bytes = s.into_bytes();
                let encoded = form_urlencoded::byte_serialize(bytes.as_slice());
                encoded.collect::<String>()
            })
            .unwrap_or("".to_owned())
    }

    Ok(SpotifyUri::Local {
        artist: format_uri_part(artist),
        album_title: format_uri_part(album_title),
        track_title: format_uri_part(track_title),
        duration: Duration::from_secs(time.as_secs().max(0) as u64),
    })
}

#[cfg(test)]
mod symphonia_six_tests {
    use super::*;
    use crate::SAMPLE_RATE;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn local_wav_fixture_preserves_one_second_duration() {
        let path = std::env::temp_dir().join(format!(
            "librespot-symphonia-six-{}-{}.wav",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let frames = SAMPLE_RATE;
        let data_len = frames * 4;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        wav.extend_from_slice(&(SAMPLE_RATE * 4).to_le_bytes());
        wav.extend_from_slice(&4_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.resize(44 + data_len as usize, 0);
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let fixture = Fixture(path);
        output.write_all(&wav).unwrap();
        drop(output);

        let uri = get_uri_from_file(&fixture.0, "wav").unwrap();
        match uri {
            SpotifyUri::Local { duration, .. } => assert_eq!(duration, Duration::from_secs(1)),
            other => panic!("expected a local URI, got {other:?}"),
        }
    }
}
