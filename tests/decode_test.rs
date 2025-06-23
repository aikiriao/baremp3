use baremp3::decoder::*;
use baremp3::types::*;
use hound;
use std::cmp::max;

#[test]
fn get_format_test() -> Result<(), Box<dyn std::error::Error>> {
    // テストケース
    struct FormatTestCase<'a> {
        path: &'a str,                // mp3ファイルパス
        format: MP3FormatInformation, // 正解フォーマット（サンプル数はdist10デコーダの結果）
    }
    let testcases = [
        FormatTestCase {
            path: "./tests/data/alphabet02all_01_32_encffmpeg.mp3",
            format: MP3FormatInformation {
                num_channels: 1,
                num_samples: 609408,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps32,
            },
        },
        FormatTestCase {
            path: "./tests/data/alphabet02all_01_128_encffmpeg.mp3",
            format: MP3FormatInformation {
                num_channels: 1,
                num_samples: 609408,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps128,
            },
        },
        FormatTestCase {
            path: "./tests/data/alphabet02all_01_320_encffmpeg.mp3",
            format: MP3FormatInformation {
                num_channels: 1,
                num_samples: 609408,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps320,
            },
        },
        FormatTestCase {
            path: "./tests/data/y004_32_encdist10.mpg",
            format: MP3FormatInformation {
                num_channels: 2,
                num_samples: 1323200,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps32,
            },
        },
        FormatTestCase {
            path: "./tests/data/y004_128_encdist10.mpg",
            format: MP3FormatInformation {
                num_channels: 2,
                num_samples: 1323200,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps128,
            },
        },
        FormatTestCase {
            path: "./tests/data/y004_320_encdist10.mpg",
            format: MP3FormatInformation {
                num_channels: 2,
                num_samples: 1323200,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps320,
            },
        },
        FormatTestCase {
            path: "./tests/data/y004_64_encgogo.mp3",
            format: MP3FormatInformation {
                num_channels: 2,
                num_samples: 1324800,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps64,
            },
        },
        FormatTestCase {
            path: "./tests/data/y004_128_encgogo.mp3",
            format: MP3FormatInformation {
                num_channels: 2,
                num_samples: 1324800,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps128,
            },
        },
        FormatTestCase {
            path: "./tests/data/y004_320_encgogo.mp3",
            format: MP3FormatInformation {
                num_channels: 2,
                num_samples: 1324800,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps320,
            },
        },
        FormatTestCase {
            path: "./tests/data/y004_32_encffmpeg.mp3",
            format: MP3FormatInformation {
                num_channels: 2,
                num_samples: 1325952,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps32,
            },
        },
        FormatTestCase {
            path: "./tests/data/y004_128_encffmpeg.mp3",
            format: MP3FormatInformation {
                num_channels: 2,
                num_samples: 1325952,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps128,
            },
        },
        FormatTestCase {
            path: "./tests/data/y004_320_encffmpeg.mp3",
            format: MP3FormatInformation {
                num_channels: 2,
                num_samples: 1325952,
                sampling_rate: MP3SamplingRate::Hz44100,
                bit_rate: MP3BitRate::Kbps320,
            },
        },
    ];

    for case in &testcases {
        let data = std::fs::read(case.path)?;
        let format = get_format_information(&data)?;
        assert_eq!(case.format.num_channels, format.num_channels);
        // サンプル数が減っていなければよいとする
        assert!(case.format.num_samples <= format.num_samples);
        assert!(case.format.sampling_rate == format.sampling_rate);
        assert!(case.format.bit_rate == format.bit_rate);
    }

    Ok(())
}

#[test]
fn decode_test() -> Result<(), Box<dyn std::error::Error>> {
    // デコードテストケース
    struct DecodeTestCase<'a> {
        mp3_path: &'a str,     // mp3ファイルパス
        ref_wav_path: &'a str, // 正解デコードデータ
    }

    let testcases = [
        DecodeTestCase {
            mp3_path: "./tests/data/alphabet02all_01_32_encffmpeg.mp3",
            ref_wav_path: "./tests/data/alphabet02all_01_32_encffmpeg_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/alphabet02all_01_128_encffmpeg.mp3",
            ref_wav_path: "./tests/data/alphabet02all_01_128_encffmpeg_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/alphabet02all_01_320_encffmpeg.mp3",
            ref_wav_path: "./tests/data/alphabet02all_01_320_encffmpeg_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/y004_32_encdist10.mpg",
            ref_wav_path: "./tests/data/y004_32_encdist10_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/y004_32_encffmpeg.mp3",
            ref_wav_path: "./tests/data/y004_32_encffmpeg_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/y004_64_encgogo.mp3",
            ref_wav_path: "./tests/data/y004_64_encgogo_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/y004_128_encdist10.mpg",
            ref_wav_path: "./tests/data/y004_128_encdist10_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/y004_128_encffmpeg.mp3",
            ref_wav_path: "./tests/data/y004_128_encffmpeg_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/y004_128_encgogo.mp3",
            ref_wav_path: "./tests/data/y004_128_encgogo_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/y004_320_encdist10.mpg",
            ref_wav_path: "./tests/data/y004_320_encdist10_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/y004_320_encffmpeg.mp3",
            ref_wav_path: "./tests/data/y004_320_encffmpeg_decdist10.wav",
        },
        DecodeTestCase {
            mp3_path: "./tests/data/y004_320_encgogo.mp3",
            ref_wav_path: "./tests/data/y004_320_encgogo_decdist10.wav",
        },
    ];

    for case in &testcases {
        let mut reader = hound::WavReader::open(case.ref_wav_path).unwrap();
        let spec = reader.spec();
        let data = std::fs::read(case.mp3_path)?;
        let format = get_format_information(&data)?;

        // サンプル数のチェック
        // dist10と一致させるのは困難なので、減っていなければよいとする
        let num_total_samples = format.num_channels * format.num_samples;
        assert!(
            num_total_samples >= spec.channels as usize * reader.duration() as usize,
            "failed to check samples for mp3:{} wav:{}", case.mp3_path, case.ref_wav_path
        );

        // リファレンス波形のPCM読み込み
        let mut ref_pcm = vec![0i16; num_total_samples];
        for smpl in 0..reader.duration() as usize {
            // インターリーブで読み出されるので、サンプル数だけ離して配置
            for ch in 0..spec.channels as usize {
                ref_pcm[smpl + ch * format.num_samples] =
                    reader.samples::<i16>().next().unwrap().unwrap();
            }
        }

        // デコード
        let mut output = vec![0.0f32; num_total_samples];
        let (left, right) = output.split_at_mut(format.num_samples);
        let mut decoder = MP3Decoder::new();
        let (_, num_decoded_samples) = decoder.decode_whole(&data, &mut [left, right])?;
        assert_eq!(num_decoded_samples, format.num_samples);

        // 末尾の遅延サンプル分(=1057)除いて比較
        // （モノラルデータでは、dist10のデコード結果末尾で成分が発生...）
        let mut max_abs_error = 0;
        for smpl in 0..(format.num_samples - 1057) {
            for ch in 0..format.num_channels {
                const AMPLITUDE: f32 = i16::MAX as f32;
                let index = smpl + ch * format.num_channels;
                let out = (output[index] * AMPLITUDE).round() as i16;
                max_abs_error = max(max_abs_error, (ref_pcm[index] - out).abs());
            }
        }
        assert!(max_abs_error <= 1);
    }

    Ok(())
}
