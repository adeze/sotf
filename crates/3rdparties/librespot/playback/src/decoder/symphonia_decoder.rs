use std::io;

use symphonia::core::{
    codecs::audio::{AudioDecoder as SymphoniaAudioDecoder, AudioDecoderOptions},
    errors::Error,
    formats::probe::Hint,
    formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType},
    io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions},
    meta::{MetadataOptions, StandardTag, Tag},
    units::{Duration as MediaDuration, Time, TimeBase, Timestamp},
};

use super::{AudioDecoder, AudioPacket, AudioPacketPosition, DecoderError, DecoderResult};

use crate::{NUM_CHANNELS, PAGES_PER_MS, SAMPLE_RATE, player::NormalisationData, symphonia_util};

pub struct SymphoniaDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaAudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    duration: Option<MediaDuration>,
    sample_buffer: Vec<f64>,
}

#[derive(Default)]
pub(crate) struct LocalFileMetadata {
    pub name: Option<String>,
    pub language: Option<String>,
    pub album: Option<String>,
    pub artists: Option<String>,
    pub album_artists: Option<String>,
    pub number: Option<u32>,
    pub disc_number: Option<u32>,
}

impl SymphoniaDecoder {
    pub fn new<R>(input: R, hint: Hint) -> DecoderResult<Self>
    where
        R: MediaSource + 'static,
    {
        let mss_opts = MediaSourceStreamOptions {
            buffer_len: librespot_audio::AudioFetchParams::get().minimum_download_size,
        };
        let mss = MediaSourceStream::new(Box::new(input), mss_opts);

        let format_opts = FormatOptions::default();
        let metadata_opts: MetadataOptions = Default::default();

        let format =
            symphonia::default::get_probe().probe(&hint, mss, format_opts, metadata_opts)?;

        let track = format.default_track(TrackType::Audio).ok_or_else(|| {
            DecoderError::SymphoniaDecoder("Could not retrieve default track".into())
        })?;

        let codec_params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .ok_or_else(|| {
                DecoderError::SymphoniaDecoder("Missing audio codec parameters".into())
            })?;
        let decoder_opts = AudioDecoderOptions::default().gapless(true);
        let decoder =
            symphonia::default::get_codecs().make_audio_decoder(codec_params, &decoder_opts)?;
        let track_id = track.id;
        let time_base = track.time_base;
        let duration = track.duration;

        let rate = decoder.codec_params().sample_rate.ok_or_else(|| {
            DecoderError::SymphoniaDecoder("Could not retrieve sample rate".into())
        })?;

        // TODO: The official client supports local files with sample rates other than 44,100 kHz.
        // To play these accurately, we need to either resample the input audio, or introduce a way
        // to change the player's current sample rate (likely by closing and re-opening the sink
        // with new parameters).
        if rate != SAMPLE_RATE {
            return Err(DecoderError::SymphoniaDecoder(format!(
                "Unsupported sample rate: {rate}"
            )));
        }

        let channels = decoder.codec_params().channels.as_ref().ok_or_else(|| {
            DecoderError::SymphoniaDecoder("Could not retrieve channel configuration".into())
        })?;
        if channels.count() != NUM_CHANNELS as usize {
            return Err(DecoderError::SymphoniaDecoder(format!(
                "Unsupported number of channels: {channels}"
            )));
        }

        Ok(Self {
            format,
            decoder,
            track_id,
            time_base,
            duration,
            sample_buffer: Vec::new(),
        })
    }

    pub fn normalisation_data(&mut self) -> Option<NormalisationData> {
        let metadata = symphonia_util::get_latest_metadata(self.format.as_mut())?;
        normalisation_from_tags(&metadata.current()?.media.tags)
    }

    pub(crate) fn local_file_metadata(&mut self) -> Option<LocalFileMetadata> {
        let metadata = symphonia_util::get_latest_metadata(self.format.as_mut())?;
        Some(local_metadata_from_tags(&metadata.current()?.media.tags))
    }

    #[inline]
    fn ts_to_ms(&self, ts: Timestamp) -> u32 {
        match self.time_base.and_then(|base| base.calc_time(ts)) {
            Some(time) => time.as_millis().clamp(0, i128::from(u32::MAX)) as u32,
            None => {
                ((ts.get().max(0) as f64 * PAGES_PER_MS) as u64).min(u64::from(u32::MAX)) as u32
            }
        }
    }
}

fn normalisation_from_tags(tags: &[Tag]) -> Option<NormalisationData> {
    if tags.is_empty() {
        return None;
    }
    let mut data = NormalisationData::default();
    for tag in tags {
        let parsed = |text: &str| {
            text.trim()
                .trim_end_matches("dB")
                .trim()
                .parse::<f64>()
                .ok()
        };
        match &tag.std {
            Some(StandardTag::ReplayGainAlbumGain(value)) => {
                if let Some(value) = parsed(value) {
                    data.album_gain_db = value;
                }
            }
            Some(StandardTag::ReplayGainAlbumPeak(value)) => {
                if let Some(value) = parsed(value) {
                    data.album_peak = value;
                }
            }
            Some(StandardTag::ReplayGainTrackGain(value)) => {
                if let Some(value) = parsed(value) {
                    data.track_gain_db = value;
                }
            }
            Some(StandardTag::ReplayGainTrackPeak(value)) => {
                if let Some(value) = parsed(value) {
                    data.track_peak = value;
                }
            }
            _ => (),
        }
    }
    Some(data)
}

fn local_metadata_from_tags(tags: &[Tag]) -> LocalFileMetadata {
    let mut metadata = LocalFileMetadata::default();
    for tag in tags {
        match &tag.std {
            Some(StandardTag::TrackTitle(value)) => metadata.name = Some((**value).clone()),
            Some(StandardTag::Language(value)) => metadata.language = Some((**value).clone()),
            Some(StandardTag::Artist(value)) => metadata.artists = Some((**value).clone()),
            Some(StandardTag::AlbumArtist(value)) => {
                metadata.album_artists = Some((**value).clone())
            }
            Some(StandardTag::Album(value)) => metadata.album = Some((**value).clone()),
            Some(StandardTag::TrackNumber(value)) => metadata.number = u32::try_from(*value).ok(),
            Some(StandardTag::DiscNumber(value)) => {
                metadata.disc_number = u32::try_from(*value).ok()
            }
            _ => (),
        }
    }
    metadata
}

impl AudioDecoder for SymphoniaDecoder {
    fn seek(&mut self, position_ms: u32) -> Result<u32, DecoderError> {
        // "Saturate" the position_ms to the duration of the track if it exceeds it.
        let mut target = Time::try_new(
            i64::from(position_ms / 1_000),
            (position_ms % 1_000) * 1_000_000,
        )
        .expect("millisecond position is a valid time");
        if let Some(duration) = self
            .duration
            .and_then(|duration| self.time_base.and_then(|base| base.calc_duration(duration)))
        {
            target = target.min(duration);
        }

        // `track_id: None` implies the default track ID (of the container, not of Spotify).
        let seeked_to_ts = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time: target,
                track_id: None,
            },
        )?;

        // Seeking is a `FormatReader` operation, so the decoder cannot reliably
        // know when a seek took place. Reset it to avoid audio glitches.
        self.decoder.reset();

        Ok(self.ts_to_ms(seeked_to_ts.actual_ts))
    }

    fn next_packet(&mut self) -> DecoderResult<Option<(AudioPacketPosition, AudioPacket)>> {
        let mut skipped = false;

        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(None),
                Err(Error::IoError(err)) => {
                    if err.kind() == io::ErrorKind::UnexpectedEof {
                        return Ok(None);
                    } else {
                        return Err(DecoderError::SymphoniaDecoder(err.to_string()));
                    }
                }
                Err(err) => {
                    return Err(err.into());
                }
            };

            if packet.track_id != self.track_id {
                continue;
            }
            let position_ms = self.ts_to_ms(packet.pts);
            let packet_position = AudioPacketPosition {
                position_ms,
                skipped,
            };

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    decoded.copy_to_vec_interleaved(&mut self.sample_buffer);
                    let samples = AudioPacket::Samples(self.sample_buffer.clone());

                    return Ok(Some((packet_position, samples)));
                }
                Err(Error::DecodeError(_)) => {
                    // The packet failed to decode due to corrupted or invalid data, get a new
                    // packet and try again.
                    warn!("Skipping malformed audio packet at {position_ms} ms");
                    skipped = true;
                    continue;
                }
                Err(err) => return Err(err.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn stereo_wav() -> Vec<u8> {
        let frames: [[i16; 2]; 4] = [[-32768, 32767], [-16384, 16384], [0, 0], [16384, -16384]];
        let data_len = (frames.len() * 4) as u32;
        let mut wav = Vec::new();
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
        for frame in frames {
            for sample in frame {
                wav.extend_from_slice(&sample.to_le_bytes());
            }
        }
        wav
    }

    #[test]
    fn symphonia_six_decodes_stereo_pcm_without_changing_channel_order() {
        let mut hint = Hint::new();
        hint.with_extension("wav");
        let mut decoder = SymphoniaDecoder::new(Cursor::new(stereo_wav()), hint).unwrap();
        let mut samples = Vec::new();
        while let Some((position, packet)) = decoder.next_packet().unwrap() {
            assert_eq!(position.position_ms, 0);
            assert!(!position.skipped);
            samples.extend_from_slice(packet.samples().unwrap());
        }
        assert_eq!(samples.len(), 8);
        assert!(samples.iter().all(|sample| sample.is_finite()));
        assert!(samples[0] < -0.99 && samples[1] > 0.99);
        assert!(samples[2] < -0.49 && samples[3] > 0.49);
        assert_eq!(&samples[4..6], &[0.0, 0.0]);
        assert!(samples[6] > 0.49 && samples[7] < -0.49);
        assert!(decoder.next_packet().unwrap().is_none());

        assert_eq!(decoder.seek(0).unwrap(), 0);
        let mut replay = Vec::new();
        while let Some((_, packet)) = decoder.next_packet().unwrap() {
            replay.extend_from_slice(packet.samples().unwrap());
        }
        assert_eq!(replay, samples);
    }

    #[test]
    fn symphonia_six_standard_tags_preserve_replaygain_and_local_metadata() {
        use std::sync::Arc;
        let tag = |std| Tag::new_from_parts("test", "value", Some(std));
        let tags = vec![
            tag(StandardTag::ReplayGainAlbumGain(Arc::new("-4.5 dB".into()))),
            tag(StandardTag::ReplayGainAlbumPeak(Arc::new("0.91".into()))),
            tag(StandardTag::ReplayGainTrackGain(Arc::new(
                "-3.25 dB".into(),
            ))),
            tag(StandardTag::ReplayGainTrackPeak(Arc::new("0.98".into()))),
            tag(StandardTag::TrackTitle(Arc::new("A Title".into()))),
            tag(StandardTag::Artist(Arc::new("An Artist".into()))),
            tag(StandardTag::Album(Arc::new("An Album".into()))),
            tag(StandardTag::TrackNumber(7)),
            tag(StandardTag::DiscNumber(2)),
        ];
        let replay = normalisation_from_tags(&tags).unwrap();
        assert_eq!(replay.album_gain_db, -4.5);
        assert_eq!(replay.album_peak, 0.91);
        assert_eq!(replay.track_gain_db, -3.25);
        assert_eq!(replay.track_peak, 0.98);
        let local = local_metadata_from_tags(&tags);
        assert_eq!(local.name.as_deref(), Some("A Title"));
        assert_eq!(local.artists.as_deref(), Some("An Artist"));
        assert_eq!(local.album.as_deref(), Some("An Album"));
        assert_eq!(local.number, Some(7));
        assert_eq!(local.disc_number, Some(2));
    }
}
