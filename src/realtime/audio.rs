use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};

pub const TARGET_SAMPLE_RATE: u32 = 24_000;

pub fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels as usize)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

pub fn resample_linear(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == target_rate {
        return samples.to_vec();
    }
    let target_len =
        ((samples.len() as f64 / source_rate as f64) * target_rate as f64).round() as usize;
    let target_len = target_len.max(1);
    let ratio = source_rate as f64 / target_rate as f64;
    (0..target_len)
        .map(|idx| {
            let pos = idx as f64 * ratio;
            let left = pos.floor() as usize;
            let right = (left + 1).min(samples.len() - 1);
            let frac = (pos - left as f64) as f32;
            samples[left] * (1.0 - frac) + samples[right] * frac
        })
        .collect()
}

pub fn pcm16_le(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|sample| {
            let clipped = sample.clamp(-1.0, 1.0);
            let value = if clipped < 0.0 {
                (clipped * 32768.0) as i16
            } else {
                (clipped * 32767.0) as i16
            };
            value.to_le_bytes()
        })
        .collect()
}

pub fn chunk_pcm16(pcm: &[u8], sample_rate: u32, chunk_ms: u32) -> Vec<Vec<u8>> {
    let bytes_per_sample = 2usize;
    let mut chunk_size = ((sample_rate * chunk_ms / 1000) as usize).max(1) * bytes_per_sample;
    chunk_size -= chunk_size % bytes_per_sample;
    pcm.chunks(chunk_size).map(|chunk| chunk.to_vec()).collect()
}

pub fn base64_pcm16(pcm: &[u8]) -> String {
    BASE64_STANDARD.encode(pcm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_and_chunks_pcm() {
        let mono = downmix_to_mono(&[1.0, -1.0, 0.5, 0.5], 2);
        assert_eq!(mono, vec![0.0, 0.5]);
        let pcm = pcm16_le(&mono);
        assert_eq!(pcm.len(), 4);
        assert_eq!(chunk_pcm16(&pcm, TARGET_SAMPLE_RATE, 20).len(), 1);
        assert!(!base64_pcm16(&pcm).is_empty());
    }
}
