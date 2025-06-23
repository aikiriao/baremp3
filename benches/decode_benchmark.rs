use baremp3::decoder::*;
use criterion::{criterion_group, criterion_main, Criterion};

pub fn decode_benchmark(c: &mut Criterion) {
    c.bench_function("MP3 stereo decode", |b| {
        b.iter(|| {
            // データ読み込み
            let data = std::fs::read("./tests/data/y004_128_encffmpeg.mp3").unwrap();
            let format = get_format_information(&data).unwrap();

            // デコード
            let mut output = vec![0.0f32; format.num_samples * format.num_channels];
            let (left, right) = output.split_at_mut(format.num_samples);
            let mut decoder = MP3Decoder::new();
            let _ = decoder.decode_whole(&data, &mut [left, right]).unwrap();
        })
    });
}

criterion_group!(benches, decode_benchmark);
criterion_main!(benches);
