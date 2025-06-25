use crate::huffman::*;
use crate::hybrid_synthesis::*;
use crate::maindata_buffer::*;
use crate::types::*;

use bitreader::BitReader;
use core::cmp::min;
use core::error;
use core::fmt;

/// 同期コード
const MP3_SYNC_CODE: u32 = 0xFFF;
/// 同期コード長(bit)
const MP3_SYNC_CODE_LENGTH: usize = 12;
/// フレームヘッダサイズ(byte)
const MP3_FRAMEHEADER_SIZE: usize = 4;
/// モノラルのサイドインフォメーションサイズ(byte)
const MP3_SIDEINFORMATION_SIZE_MONO: usize = 17;
/// ステレオのサイドインフォメーションサイズ(byte)
const MP3_SIDEINFORMATION_SIZE_STEREO: usize = 32;

/// 1グラニュールのスケールファクタ
struct GranuleScaleFactor {
    /// longブロックのクリティカルバンド
    long: [u8; MP3_NUM_CRITICAL_BANDS_LONG],
    /// shortブロックのクリティカルバンド(3つ分)
    short: [[u8; MP3_NUM_CRITICAL_BANDS_SHORT]; 3],
}

/// 1フレームのスケールファクタ
struct FrameScaleFactor {
    /// グラニュールのスケールファクタ
    gr: [GranuleScaleFactor; MP3_NUM_GRANLES_PER_FRAME],
}

/// スケールファクタ
struct MP3ScaleFactor {
    /// 各チャンネルのスケールファクタ
    ch: [FrameScaleFactor; MP3_MAX_NUM_CHANNELS],
}

impl Default for MP3ScaleFactor {
    fn default() -> Self {
        Self {
            ch: [
                FrameScaleFactor {
                    gr: [
                        GranuleScaleFactor {
                            long: [0u8; MP3_NUM_CRITICAL_BANDS_LONG],
                            short: [[0u8; MP3_NUM_CRITICAL_BANDS_SHORT]; 3],
                        },
                        GranuleScaleFactor {
                            long: [0u8; MP3_NUM_CRITICAL_BANDS_LONG],
                            short: [[0u8; MP3_NUM_CRITICAL_BANDS_SHORT]; 3],
                        },
                    ],
                },
                FrameScaleFactor {
                    gr: [
                        GranuleScaleFactor {
                            long: [0u8; MP3_NUM_CRITICAL_BANDS_LONG],
                            short: [[0u8; MP3_NUM_CRITICAL_BANDS_SHORT]; 3],
                        },
                        GranuleScaleFactor {
                            long: [0u8; MP3_NUM_CRITICAL_BANDS_LONG],
                            short: [[0u8; MP3_NUM_CRITICAL_BANDS_SHORT]; 3],
                        },
                    ],
                },
            ],
        }
    }
}

/// MP3デコーダ
pub struct MP3Decoder {
    /// メインデータバッファ
    maindata_buffer: MP3MainDataBuffer,
    /// ハイブリッド合成フィルタバンクのバッファ
    synth_buffer: [MP3SynthesisBuffer; MP3_MAX_NUM_CHANNELS],
    /// メインデータ開始位置
    maindata_start: usize,
}

/// スケールファクタのビット幅テーブル
const SCALEFACTOR_BITS_TABLE: [[u8; 16]; 2] = [
    [0, 0, 0, 0, 3, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4],
    [0, 1, 2, 3, 0, 1, 2, 3, 1, 2, 3, 1, 2, 3, 2, 3],
];

/// スケールファクタの分割バンド開始インデックステーブル(long)
const SCALEFACTOR_DIVISION_START_INDEX_TABLE_LONG: [usize; 5] = [0, 6, 11, 16, 21];

/// スケールファクタの分割バンド開始インデックステーブル(short)
const SCALEFACTOR_DIVISION_START_INDEX_TABLE_SHORT: [usize; 3] = [0, 6, 12];

impl Default for MP3GranuleInformation {
    fn default() -> Self {
        Self {
            part2_3_length: 0,
            big_values: 0,
            global_gain: 0,
            scalefac_compress: 0,
            window_switching_flag: false,
            block_type: MP3BlockType::Normal,
            mixed_block_flag: false,
            table_select: [0; 3],
            subblock_gain: [0; 3],
            region0_count: 0,
            region1_count: 0,
            preflag: false,
            scalefac_scale: 0,
            count1table_select: 0,
        }
    }
}

/// サイドインフォメーションのサイズを計算
macro_rules! get_sideinformation_size {
    ($header:expr) => {{
        match $header.channel_mode {
            MP3ChannelMode::Monoral => MP3_SIDEINFORMATION_SIZE_MONO,
            _ => MP3_SIDEINFORMATION_SIZE_STEREO,
        }
    }};
}

/// メインデータに含まれるデータサイズ(byte)を取得
fn get_maindata_size(header: &MP3FrameHeader) -> usize {
    let mut size: usize = 0;

    // 1152(1フレームあたりサンプル数) * bits_per_second / sampling_rate(Hz) をバイト単位に換算
    size += 144 * header.bit_rate as usize / header.sampling_rate as usize;

    // ヘッダ分（同期コード含む）を減算
    size -= MP3_FRAMEHEADER_SIZE;

    // サイドインフォメーション分を減算
    size -= get_sideinformation_size!(header);

    // パディングがある場合は1byte増加
    if header.padding {
        size += 1;
    }

    // CRC16の2byteを減算
    if header.error_protection {
        size -= 2;
    }

    size
}

/// 同期コードの検索
fn find_sync_code(data: &[u8]) -> Option<usize> {
    // 同期コードの照合パターン
    const MP3_SYNC_CODE_SHIFT: u32 = 16 - MP3_SYNC_CODE_LENGTH as u32;
    const MP3_SYNC_CODE_PATTERN: u32 = MP3_SYNC_CODE << MP3_SYNC_CODE_SHIFT;

    // データ不足
    if data.len() < 2 {
        return None;
    }

    // パターン一致するインデックスをバイト単位で探索
    let mut pattern = data[0] as u32;
    for pos in 1..(data.len() - 1) {
        pattern = (pattern << 8) | data[pos] as u32;
        if (pattern & MP3_SYNC_CODE_PATTERN) == MP3_SYNC_CODE_PATTERN {
            return Some(pos - 1);
        }
    }

    None
}

/// フレームヘッダのデコード
fn decode_frame_header(data: &[u8]) -> Option<MP3FrameHeader> {
    // データサイズ不足
    if data.len() < MP3_FRAMEHEADER_SIZE {
        return None;
    }

    // ビットリーダ作成
    let mut breader = BitReader::new(data);

    // 同期コードのチェック
    if breader.read_u32(MP3_SYNC_CODE_LENGTH as u8).unwrap() != MP3_SYNC_CODE {
        return None;
    }

    // ヘッダの内容読み取り
    Some(MP3FrameHeader {
        version: match breader.read_u8(1).unwrap() {
            0 => MPEGVersion::MPEGVersion2,
            1 => MPEGVersion::MPEGVersion1,
            _ => return None,
        },
        layer: match 4 - breader.read_u8(2).unwrap() {
            1 => MP3Layer::Layer1,
            2 => MP3Layer::Layer2,
            3 => MP3Layer::Layer3,
            _ => return None,
        },
        error_protection: !breader.read_bool().unwrap(),
        bit_rate: match breader.read_u8(4).unwrap() {
            0 => MP3BitRate::Kbps0,
            1 => MP3BitRate::Kbps32,
            2 => MP3BitRate::Kbps40,
            3 => MP3BitRate::Kbps48,
            4 => MP3BitRate::Kbps56,
            5 => MP3BitRate::Kbps64,
            6 => MP3BitRate::Kbps80,
            7 => MP3BitRate::Kbps96,
            8 => MP3BitRate::Kbps112,
            9 => MP3BitRate::Kbps128,
            10 => MP3BitRate::Kbps160,
            11 => MP3BitRate::Kbps192,
            12 => MP3BitRate::Kbps224,
            13 => MP3BitRate::Kbps256,
            14 => MP3BitRate::Kbps320,
            _ => return None,
        },
        sampling_rate: match breader.read_u8(2).unwrap() {
            0 => MP3SamplingRate::Hz44100,
            1 => MP3SamplingRate::Hz48000,
            2 => MP3SamplingRate::Hz32000,
            _ => return None,
        },
        padding: breader.read_bool().unwrap(),
        extension: breader.read_u8(1).unwrap() as u8,
        channel_mode: match breader.read_u8(2).unwrap() {
            0 => MP3ChannelMode::Stereo,
            1 => MP3ChannelMode::JointStereo,
            2 => MP3ChannelMode::DualChannel,
            3 => MP3ChannelMode::Monoral,
            _ => return None,
        },
        ext_channel_mode: {
            let flags = breader.read_u8(2).unwrap();
            if (flags & 0x1) != 0 {
                MP3ExtChannelMode::IntensityStereo
            } else if (flags & 0x2) != 0 {
                MP3ExtChannelMode::MSStereo
            } else {
                MP3ExtChannelMode::NONE
            }
        },
        copyright: breader.read_bool().unwrap(),
        original: breader.read_bool().unwrap(),
        emphasis: match breader.read_u32(2).unwrap() {
            0 => MP3EmphasisMode::NONE,
            1 => MP3EmphasisMode::FiftyFifteenMs,
            2 => MP3EmphasisMode::Reserved,
            3 => MP3EmphasisMode::CCITTJ17,
            _ => return None,
        },
    })
}

/// サイドインフォメーションのデコード
fn decode_side_information(header: &MP3FrameHeader, data: &[u8]) -> Option<MP3SideInformation> {
    // MPEG1以外は対応していない
    match header.version {
        MPEGVersion::MPEGVersion1 => {}
        MPEGVersion::MPEGVersion2 => return None,
    }

    // データサイズ不足
    if data.len() < get_sideinformation_size!(header) {
        return None;
    }

    // チャンネル数の判定
    let num_channels = match header.channel_mode {
        MP3ChannelMode::Monoral => 1,
        _ => 2,
    };

    let mut side_info = MP3SideInformation {
        maindata_begin: 0,
        private_bits: 0,
        ch: [
            MP3ChannelSideInformation {
                scfsi: [false; 4],
                gr: [
                    MP3GranuleInformation::default(),
                    MP3GranuleInformation::default(),
                ],
            },
            MP3ChannelSideInformation {
                scfsi: [false; 4],
                gr: [
                    MP3GranuleInformation::default(),
                    MP3GranuleInformation::default(),
                ],
            },
        ],
    };

    // ビットリーダ作成
    let mut breader = BitReader::new(data);

    // メインデータ開始位置（負のオフセット）
    side_info.maindata_begin = breader.read_u16(9).unwrap();
    // プライベートビット
    side_info.private_bits = if num_channels == 1 {
        breader.read_u8(5).unwrap()
    } else {
        breader.read_u8(3).unwrap()
    };
    // scfsi
    for ch in 0..num_channels {
        for i in 0..4 {
            side_info.ch[ch].scfsi[i] = breader.read_bool().unwrap();
        }
    }
    // グラニュール
    for gr in 0..2 {
        for ch in 0..num_channels {
            let granule: &mut MP3GranuleInformation = &mut side_info.ch[ch].gr[gr];
            granule.part2_3_length = breader.read_u16(12).unwrap();
            granule.big_values = breader.read_u16(9).unwrap();
            granule.global_gain = breader.read_u8(8).unwrap();
            granule.scalefac_compress = breader.read_u8(4).unwrap();
            granule.window_switching_flag = breader.read_bool().unwrap();
            if granule.window_switching_flag {
                granule.block_type = match breader.read_u8(2).unwrap() {
                    1 => MP3BlockType::Start,
                    2 => MP3BlockType::Short,
                    3 => MP3BlockType::Stop,
                    // 窓関数の切り替わりでlong(normal)は無効
                    0 => return None,
                    _ => return None,
                };

                granule.mixed_block_flag = breader.read_bool().unwrap();
                for i in 0..2 {
                    granule.table_select[i] = breader.read_u8(5).unwrap();
                }
                for i in 0..3 {
                    granule.subblock_gain[i] = breader.read_u8(3).unwrap();
                }

                granule.region0_count = match granule.block_type {
                    MP3BlockType::Short if !granule.mixed_block_flag => 8,
                    _ => 7,
                };
                granule.region1_count = 20 - granule.region0_count;
            } else {
                granule.block_type = MP3BlockType::Normal;
                for i in 0..3 {
                    granule.table_select[i] = breader.read_u8(5).unwrap();
                }
                granule.region0_count = breader.read_u8(4).unwrap();
                granule.region1_count = breader.read_u8(3).unwrap();
            }
            granule.preflag = breader.read_bool().unwrap();
            granule.scalefac_scale = breader.read_u8(1).unwrap();
            granule.count1table_select = breader.read_u8(1).unwrap();
        }
    }

    Some(side_info)
}

/// スケールファクタのデコード
fn decode_granule_scale_factor(
    buffer: &mut MP3MainDataBuffer,
    granule: &MP3GranuleInformation,
    second_granule: bool,
    scfsi: &[bool; 4],
    first_gr_scale_factor: &GranuleScaleFactor,
) -> GranuleScaleFactor {
    let mut gr_scale_factor = GranuleScaleFactor {
        long: [0u8; MP3_NUM_CRITICAL_BANDS_LONG],
        short: [[0u8; MP3_NUM_CRITICAL_BANDS_SHORT]; 3],
    };

    match granule.block_type {
        MP3BlockType::Short if granule.window_switching_flag => {
            if granule.mixed_block_flag {
                // ミックスドブロック
                for sfb in 0..8 {
                    gr_scale_factor.long[sfb] = buffer
                        .get_bits(SCALEFACTOR_BITS_TABLE[0][granule.scalefac_compress as usize])
                        as u8;
                }
                for sfb in 0..6 {
                    for win in 0..3 {
                        gr_scale_factor.short[win][sfb] = buffer
                            .get_bits(SCALEFACTOR_BITS_TABLE[0][granule.scalefac_compress as usize])
                            as u8;
                    }
                }
                for sfb in 6..12 {
                    for win in 0..3 {
                        gr_scale_factor.short[win][sfb] = buffer
                            .get_bits(SCALEFACTOR_BITS_TABLE[1][granule.scalefac_compress as usize])
                            as u8;
                    }
                }
            } else {
                // ショートブロック
                for i in 0..2 {
                    for sfb in SCALEFACTOR_DIVISION_START_INDEX_TABLE_SHORT[i]
                        ..SCALEFACTOR_DIVISION_START_INDEX_TABLE_SHORT[i + 1]
                    {
                        for win in 0..3 {
                            gr_scale_factor.short[win][sfb] = buffer.get_bits(
                                SCALEFACTOR_BITS_TABLE[i][granule.scalefac_compress as usize],
                            ) as u8;
                        }
                    }
                }
                // 末尾は0埋め
                for win in 0..3 {
                    gr_scale_factor.short[win][12] = 0;
                }
            }
        }
        _ => {
            // ロングブロック
            for i in 0..4 {
                if scfsi[i] && second_granule {
                    // スケールファクタの共有
                    for sfb in SCALEFACTOR_DIVISION_START_INDEX_TABLE_LONG[i]
                        ..SCALEFACTOR_DIVISION_START_INDEX_TABLE_LONG[i + 1]
                    {
                        gr_scale_factor.long[sfb] = first_gr_scale_factor.long[sfb];
                    }
                } else {
                    for sfb in SCALEFACTOR_DIVISION_START_INDEX_TABLE_LONG[i]
                        ..SCALEFACTOR_DIVISION_START_INDEX_TABLE_LONG[i + 1]
                    {
                        // 高域ではインデックス1を使用
                        let index = if i < 2 { 0 } else { 1 };
                        gr_scale_factor.long[sfb] = buffer.get_bits(
                            SCALEFACTOR_BITS_TABLE[index][granule.scalefac_compress as usize],
                        ) as u8;
                    }
                }
            }
        }
    }

    gr_scale_factor
}

/// 量子化データのハフマン符号デコード
fn decode_huffman(
    buffer: &mut MP3MainDataBuffer,
    header: &MP3FrameHeader,
    granule: &MP3GranuleInformation,
    part2_start: u64,
    output: &mut [f32; MP3_NUM_SAMPLES_PER_GRANULE],
) {
    /// ビット読み出し位置positionがcount1 data内にあるか判定
    macro_rules! positon_isin_count1data {
        ($position:expr,$part2_start:expr,$part3_end:expr) => {{
            if $part3_end >= $part2_start {
                ($position >= $part2_start) && ($position < $part3_end)
            } else {
                ($position >= $part2_start) || ($position < $part3_end)
            }
        }};
    }

    let part3_end = (part2_start + granule.part2_3_length as u64) % MP3_MAINDATA_BUFFER_SIZE_BITS;

    // region1(-1, 0, 1のみ), region2(0のみ)開始位置の取得
    let (region1_start, region2_start) = match granule.block_type {
        MP3BlockType::Short if granule.window_switching_flag => {
            // ショートブロックではregion2がない
            (36, MP3_NUM_SAMPLES_PER_GRANULE)
        }
        _ => {
            let long_table = &get_scalefactorband_index_table!(header.sampling_rate).long;
            (
                long_table[granule.region0_count as usize + 1] as usize,
                long_table[granule.region0_count as usize + 1 + granule.region1_count as usize + 1]
                    as usize,
            )
        }
    };

    // bigvalueの復号
    assert!((2 * granule.big_values as usize) <= MP3_NUM_SAMPLES_PER_GRANULE);
    for i in (0..(2 * granule.big_values as usize)).step_by(2) {
        let index = if i < region1_start {
            granule.table_select[0]
        } else if i < region2_start {
            granule.table_select[1]
        } else {
            granule.table_select[2]
        };
        // 2つ組で復号
        let xy = mp3_huffman_decode_big_value(index as usize, buffer);
        output[i + 0] = xy.0 as f32;
        output[i + 1] = xy.1 as f32;
    }

    // count1(-1,0,1)の復号
    let mut i = 2 * granule.big_values as usize;
    let mut position = buffer.get_total_read_bits();
    while i < MP3_NUM_SAMPLES_PER_GRANULE
        && positon_isin_count1data!(position, part2_start, part3_end)
    {
        // 4つ組(x,y,v,w)で復号
        let xyvw = mp3_huffman_decode_count1_data(granule.count1table_select as usize, buffer);
        output[i + 0] = xyvw.0 as f32;
        output[i + 1] = xyvw.1 as f32;
        // たとえばi == 574のときオーバーランするため範囲チェック
        if (i + 2) < MP3_NUM_SAMPLES_PER_GRANULE {
            output[i + 2] = xyvw.2 as f32;
            output[i + 3] = xyvw.3 as f32;
        }
        i += 4;
        position = buffer.get_total_read_bits();
    }

    // count1の読み出しが領域の外に出てしまったら4サンプル分捨てる
    if (i > (2 * granule.big_values as usize))
        && (position != part3_end)
        && !positon_isin_count1data!(position, part2_start, part3_end)
    {
        i -= 4;
    }

    // 残りは0で埋める
    if i < MP3_NUM_SAMPLES_PER_GRANULE {
        output[i..].fill(0.0f32);
    }

    // part3_endの位置にシーク
    if position != part3_end {
        buffer.seek(part3_end);
    }
}

/// 逆量子化
fn dequantize(
    header: &MP3FrameHeader,
    granule: &MP3GranuleInformation,
    scale_factor: &GranuleScaleFactor,
    output: &mut [f32; MP3_NUM_SAMPLES_PER_GRANULE],
) {
    // プリエンファシス時の増幅値テーブル
    const PREEMPHASIS_TABLE: [u8; 22] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 3, 2, 0,
    ];

    // グローバルゲイン計算
    let global_gain = 2.0f64.powf(0.25 * (granule.global_gain as f64 - 210.0));

    match granule.block_type {
        MP3BlockType::Short if granule.window_switching_flag && !granule.mixed_block_flag => {
            // ショートブロック
            let sfb_short_index = &get_scalefactorband_index_table!(header.sampling_rate).short;
            // クリティカルバンド境界の初期化
            let mut next_cb_bound = 3 * sfb_short_index[1];
            let mut cb_width = sfb_short_index[1];
            let mut cb_begin = 0;
            let mut cb = 0;

            for i in 0..MP3_NUM_SAMPLES_PER_GRANULE {
                // クリティカルバンド境界の更新
                if i == next_cb_bound as usize {
                    cb += 1;
                    cb_begin = next_cb_bound;
                    next_cb_bound = 3 * sfb_short_index[cb + 1];
                    cb_width = sfb_short_index[cb + 1] - sfb_short_index[cb];
                }

                // 量子化ステップ幅計算（スケールファクタ適用）
                // TODO: もっと呼び出しを減らせるはず
                let short_index = (i - cb_begin as usize) / cb_width as usize;
                let gain = global_gain
                    * 2.0f64.powf(
                        -2.0 * granule.subblock_gain[short_index] as f64
                            - 0.5
                                * (1.0 + granule.scalefac_scale as f64)
                                * (scale_factor.short[short_index][cb] as f64),
                    );

                // べき乗(3/4)の復元・符号適用
                let mut iqout = output[i].abs().powf(4.0 / 3.0);
                iqout = if output[i] < 0.0 { -iqout } else { iqout };

                // ゲインを適用して結果出力
                output[i] = gain as f32 * iqout;
            }
        }
        MP3BlockType::Short if granule.window_switching_flag && granule.mixed_block_flag => {
            // ミックスドブロック
            let sfb_long_index = &get_scalefactorband_index_table!(header.sampling_rate).long;
            let sfb_short_index = &get_scalefactorband_index_table!(header.sampling_rate).short;
            // クリティカルバンド境界の初期化
            let mut next_cb_bound = sfb_long_index[1] as usize;
            let mut cb_width = sfb_short_index[1];
            let mut cb_begin = 0;
            let mut cb = 0;

            // TODO: iの範囲で分けた方がよさそう
            for i in 0..MP3_NUM_SAMPLES_PER_GRANULE {
                // クリティカルバンド境界の更新
                if i == next_cb_bound as usize {
                    cb += 1;
                    if i < sfb_long_index[8] as usize {
                        next_cb_bound = sfb_long_index[cb + 1] as usize;
                    } else if i == sfb_long_index[8] as usize {
                        next_cb_bound = 3 * sfb_short_index[4] as usize;
                        cb = 3;
                        cb_width = sfb_short_index[cb + 1] - sfb_short_index[cb];
                        cb_begin = 3 * sfb_short_index[cb] as usize;
                    } else {
                        cb_begin = next_cb_bound;
                        next_cb_bound = 3 * sfb_short_index[cb + 1] as usize;
                        cb_width = sfb_short_index[cb + 1] - sfb_short_index[cb];
                    }
                }

                // サブバンド内インデックス
                // 量子化ステップ幅計算（スケールファクタ適用）
                let gain = global_gain
                    * if (i / MP3_DCT_SIZE) >= 2 {
                        let short_index = (i - cb_begin as usize) / cb_width as usize;
                        2.0f64.powf(
                            -2.0 * granule.subblock_gain[short_index] as f64
                                - 0.5
                                    * (1.0 + granule.scalefac_scale as f64)
                                    * (scale_factor.short[short_index][cb] as f64),
                        )
                    } else {
                        2.0f64.powf(
                            -0.5 * (1.0 + granule.scalefac_scale as f64)
                                * (scale_factor.long[cb] as f64
                                    + granule.preflag as u8 as f64 * PREEMPHASIS_TABLE[cb] as f64),
                        )
                    };

                // べき乗(3/4)の復元・符号適用
                let mut iqout = output[i].abs().powf(4.0 / 3.0);
                iqout = if output[i] < 0.0 { -iqout } else { iqout };

                // ゲインを適用して結果出力
                output[i] = gain as f32 * iqout;
            }
        }
        _ => {
            // ロングブロック
            let sfb_long_index = &get_scalefactorband_index_table!(header.sampling_rate).long;

            // クリティカルバンド境界の初期化
            let mut next_cb_bound = sfb_long_index[1] as usize;
            let mut cb = 0;
            // 量子化ステップ幅計算
            let mut gain = global_gain
                * 2.0f64.powf(
                    -0.5 * (1.0 + granule.scalefac_scale as f64)
                        * (scale_factor.long[0] as f64
                            + granule.preflag as u8 as f64 * PREEMPHASIS_TABLE[0] as f64),
                );

            for i in 0..MP3_NUM_SAMPLES_PER_GRANULE {
                // クリティカルバンド境界の更新
                if i == next_cb_bound {
                    cb += 1;
                    next_cb_bound = sfb_long_index[cb + 1] as usize;
                    // 量子化ステップ幅計算（スケールファクタ適用）
                    gain = global_gain
                        * 2.0f64.powf(
                            -0.5 * (1.0 + granule.scalefac_scale as f64)
                                * (scale_factor.long[cb] as f64
                                    + granule.preflag as u8 as f64 * PREEMPHASIS_TABLE[cb] as f64),
                        );
                }

                // べき乗(3/4)の復元・符号適用
                let mut iqout = output[i].abs().powf(4.0 / 3.0);
                iqout = if output[i] < 0.0 { -iqout } else { iqout };

                // ゲインを適用して結果出力
                output[i] = gain as f32 * iqout;
            }
        }
    };
}

/// デコードエラー
#[derive(Debug)]
pub enum MP3DecodeError {
    /// ストリーム終端に達した
    EndOfStream,
    /// 不正なヘッダ
    InvalidHeader,
    /// 不正なサイドインフォメーション
    InvalidSideInformation,
    /// 不正なフォーマット
    InvalidFormat,
    /// バッファサイズが不十分
    InsufficientBuffer,
}

impl fmt::Display for MP3DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MP3 Decoding Error!")
    }
}

impl error::Error for MP3DecodeError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

/// フレーム情報のデコード
fn decode_frame_information(
    data: &[u8],
) -> Result<(usize, usize, MP3FrameHeader, MP3SideInformation), MP3DecodeError> {
    let mut read_pos = 0;

    // 同期コードまでシーク
    if let Some(sync_pos) = find_sync_code(data) {
        read_pos += sync_pos;
    } else {
        return Err(MP3DecodeError::EndOfStream);
    }

    // ヘッダデコード
    let Some(header) = decode_frame_header(&data[read_pos..]) else {
        return Err(MP3DecodeError::InvalidHeader);
    };
    read_pos += MP3_FRAMEHEADER_SIZE;

    // サイドインフォメーションをデコード
    let Some(side_info) = decode_side_information(&header, &data[read_pos..]) else {
        return Err(MP3DecodeError::InvalidSideInformation);
    };
    read_pos += get_sideinformation_size!(header);

    // CRC16の読み飛ばし
    if header.error_protection {
        read_pos += 2;
    }

    // メインデータサイズの計算
    let maindata_size = min(data.len() - read_pos, get_maindata_size(&header));

    Ok((read_pos, maindata_size, header, side_info))
}

/// フォーマット情報の取得
pub fn get_format_information(data: &[u8]) -> Result<MP3FormatInformation, MP3DecodeError> {
    // 仮のフォーマットを作成
    let mut format = MP3FormatInformation {
        num_channels: 1,
        num_samples: 0,
        sampling_rate: MP3SamplingRate::Hz44100,
        bit_rate: MP3BitRate::Kbps128,
    };

    // 先頭からフレーム情報のみを取得
    let mut read_pos = 0;
    loop {
        match decode_frame_information(&data[read_pos..]) {
            Ok((header_size, maindata_size, header, _)) => {
                // ステレオチャンネルを検知
                format.num_channels = match header.channel_mode {
                    MP3ChannelMode::Stereo
                    | MP3ChannelMode::JointStereo
                    | MP3ChannelMode::DualChannel => 2,
                    _ => format.num_channels,
                };
                format.sampling_rate = header.sampling_rate;
                format.bit_rate = header.bit_rate;
                format.num_samples += MP3_NUM_SAMPLES_PER_FRAME;
                read_pos += header_size + maindata_size;
            }
            Err(e) => match e {
                MP3DecodeError::EndOfStream => break,
                _ => return Err(e),
            },
        }
    }

    Ok(format)
}

/// ID3v2タグ全体のサイズを計算
pub fn get_id3v2tag_size(data: &[u8]) -> Result<usize, MP3DecodeError> {
    const ID3V2HEADER_SIZE: usize = 10;

    // サイズ不足
    if data.len() < ID3V2HEADER_SIZE {
        return Err(MP3DecodeError::InvalidHeader);
    }

    // タグがない場合
    if data[0] != b'I' || data[1] != b'D' || data[2] != b'3' {
        return Ok(0);
    }

    Ok(((data[6] as usize) << 21)
        + ((data[7] as usize) << 14)
        + ((data[8] as usize) << 7)
        + ((data[9] as usize) << 0))
}

impl MP3Decoder {
    /// デコーダ生成
    pub fn new() -> Self {
        Self {
            maindata_buffer: MP3MainDataBuffer::new(),
            synth_buffer: [MP3SynthesisBuffer::new(), MP3SynthesisBuffer::new()],
            maindata_start: 0,
        }
    }

    /// デコーダ内部状態リセット
    pub fn reset(&mut self) {
        self.maindata_buffer.reset();
        for buf in &mut self.synth_buffer {
            buf.reset();
        }
        self.maindata_start = 0;
    }

    /// メインデータのデコード
    fn decode_maindata(
        &mut self,
        header: &MP3FrameHeader,
        side_info: &MP3SideInformation,
        output: &mut [[f32; MP3_NUM_SAMPLES_PER_FRAME]],
    ) {
        // バイト境界に揃える
        self.maindata_buffer.align_next_byte();

        // 読み捨てバイト数の計算
        let prev_maindata_end = (self.maindata_buffer.get_total_read_bits() / 8) as usize;
        let maindata_offset = prev_maindata_end + side_info.maindata_begin as usize;

        let discard_bytes = if self.maindata_start >= maindata_offset {
            self.maindata_start - maindata_offset
        } else {
            // maindata_beginの後でバッファを折り返して先頭に戻った場合、負値になるためバッファ一周分補正
            if (MP3_MAINDATA_BUFFER_SIZE + self.maindata_start) < maindata_offset {
                // 必要なデータ不足（フレーム破棄などで対処）
                return;
            }
            MP3_MAINDATA_BUFFER_SIZE + self.maindata_start - maindata_offset
        };

        // 不要なバイトの読み捨て
        self.maindata_buffer.skip(discard_bytes as u64 * 8);

        // メインデータ開始位置の更新
        self.maindata_start += get_maindata_size(&header);
        // バッファの回り込み
        if self.maindata_start > MP3_MAINDATA_BUFFER_SIZE {
            self.maindata_start -= MP3_MAINDATA_BUFFER_SIZE;
        }

        // 処理チャンネル数
        let num_channels = match header.channel_mode {
            MP3ChannelMode::Monoral => 1,
            _ => 2,
        };

        let mut scale_factor = MP3ScaleFactor::default();

        for gr in 0..MP3_NUM_GRANLES_PER_FRAME {
            for ch in 0..num_channels {
                let output_ref = <&mut [f32; MP3_NUM_SAMPLES_PER_GRANULE]>::try_from(
                    &mut output[ch]
                        [gr * MP3_NUM_SAMPLES_PER_GRANULE..(gr + 1) * MP3_NUM_SAMPLES_PER_GRANULE],
                )
                .unwrap();
                let part2_start = self.maindata_buffer.get_total_read_bits();

                // スケールファクタのデコード
                scale_factor.ch[ch].gr[gr] = decode_granule_scale_factor(
                    &mut self.maindata_buffer,
                    &side_info.ch[ch].gr[gr],
                    gr == (MP3_NUM_GRANLES_PER_FRAME - 1),
                    &side_info.ch[ch].scfsi,
                    &scale_factor.ch[ch].gr[0],
                );

                // ハフマン符号による量子化データデコード
                decode_huffman(
                    &mut self.maindata_buffer,
                    header,
                    &side_info.ch[ch].gr[gr],
                    part2_start,
                    output_ref,
                );

                // 逆量子化
                dequantize(
                    header,
                    &side_info.ch[ch].gr[gr],
                    &scale_factor.ch[ch].gr[gr],
                    output_ref,
                );
            }
        }

        // ハイブリッドフィルタバンク合成
        mp3_hybrid_synthesis(&header, &side_info, &mut self.synth_buffer, output);
    }

    /// 1フレームデコード
    pub fn decode_frame(
        &mut self,
        data: &[u8],
        buffer: &mut [[f32; MP3_NUM_SAMPLES_PER_FRAME]],
    ) -> Result<(usize, MP3FrameHeader, MP3SideInformation), MP3DecodeError> {
        // フレーム情報をデコード
        let (header_size, maindata_size, header, side_info) = decode_frame_information(data)?;

        // バッファチャンネル数チェック
        match header.channel_mode {
            MP3ChannelMode::Stereo | MP3ChannelMode::JointStereo | MP3ChannelMode::DualChannel
                if buffer.len() < 2 =>
            {
                return Err(MP3DecodeError::InsufficientBuffer);
            }
            MP3ChannelMode::Monoral if buffer.len() < 1 => {
                return Err(MP3DecodeError::InsufficientBuffer);
            }
            _ => {}
        }

        // メインデータをバッファに入力
        self.maindata_buffer
            .put_data(&data[header_size..header_size + maindata_size]);

        // メインデータのデコード
        self.decode_maindata(&header, &side_info, buffer);

        Ok((header_size + maindata_size, header, side_info))
    }

    /// 全データフレームデコード
    pub fn decode_whole(
        &mut self,
        data: &[u8],
        output: &mut [&mut [f32]],
    ) -> Result<(usize, usize), MP3DecodeError> {
        // ハンドルをリセット
        self.reset();

        let num_channels = if output.len() == 2 {
            if output[1].len() > 0 {
                2
            } else {
                1
            }
        } else {
            1
        };

        // 出力バッファ確保
        let mut buffer = [[0.0f32; MP3_NUM_SAMPLES_PER_FRAME]; MP3_MAX_NUM_CHANNELS];
        let mut num_samples = 0;
        // ID3v2タグをスキップ
        let mut read_pos = get_id3v2tag_size(data)?;
        loop {
            // 1フレームデコードを繰り返す
            match self.decode_frame(&data[read_pos..], &mut buffer) {
                Ok((size, _, _)) => {
                    for ch in 0..num_channels {
                        output[ch][num_samples..num_samples + MP3_NUM_SAMPLES_PER_FRAME]
                            .copy_from_slice(&buffer[ch])
                    }
                    read_pos += size;
                    num_samples += MP3_NUM_SAMPLES_PER_FRAME;
                }
                Err(e) => match e {
                    MP3DecodeError::EndOfStream => break,
                    _ => return Err(e),
                },
            }
        }

        Ok((read_pos, num_samples))
    }
}
