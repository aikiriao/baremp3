use baremp3::decoder::*;
use hound;
use std::env;
use std::fmt::Error;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    // 引数が合っていないときは説明を表示
    if args.len() != 3 {
        println!("Usage: {} INPUT_MP3 OUTPUT_WAV", args[0]);
        return Err(Box::new(Error));
    }

    // データ読み込み
    let data = std::fs::read(&args[1])?;
    let format = get_format_information(&data)?;

    // デコード
    let mut output = vec![0.0f32; format.num_samples * format.num_channels];
    let (left, right) = output.split_at_mut(format.num_samples);
    let mut decoder = MP3Decoder::new();
    let _ = decoder.decode_whole(&data, &mut [left, right])?;

    // 出力wavのフォーマット
    let spec = hound::WavSpec {
        channels: format.num_channels as u16,
        sample_rate: format.sampling_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    // wav書き出し
    let mut writer = hound::WavWriter::create(&args[2], spec).unwrap();
    for smpl in 0..format.num_samples {
        for ch in 0..format.num_channels {
            const AMPLITUDE: f32 = i16::MAX as f32;
            writer
                .write_sample((output[smpl + ch * format.num_channels] * AMPLITUDE) as i16)
                .unwrap();
        }
    }
    writer.finalize().unwrap();

    Ok(())
}
