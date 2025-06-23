/// 最大チャンネル数
pub const MP3_MAX_NUM_CHANNELS: usize = 2;
/// フレーム当たりサンプル数
pub const MP3_NUM_SAMPLES_PER_FRAME: usize = 1152;
/// フレームあたりグラニュール数
pub const MP3_NUM_GRANLES_PER_FRAME: usize = 2;
/// グラニュール当たりサンプル数
pub const MP3_NUM_SAMPLES_PER_GRANULE: usize =
    MP3_NUM_SAMPLES_PER_FRAME / MP3_NUM_GRANLES_PER_FRAME;
/// longブロックのクリティカルバンド数
pub const MP3_NUM_CRITICAL_BANDS_LONG: usize = 23;
/// shortブロックのクリティカルバンド数
pub const MP3_NUM_CRITICAL_BANDS_SHORT: usize = 13;

/// MPEGバージョン
pub enum MPEGVersion {
    /// MPEG1
    MPEGVersion1 = 1,
    /// MPEG2(LSF, Low Sampling Frequency)
    MPEGVersion2 = 0,
}

/// ブロックタイプ
pub enum MP3BlockType {
    /// 通常の窓(long)
    Normal = 0,
    /// ショートブロック開始
    Start = 1,
    /// ショートブロック
    Short = 2,
    /// ショートブロック終了
    Stop = 3,
}

/// チャンネルモード
pub enum MP3ChannelMode {
    /// ステレオ
    Stereo = 0,
    /// ジョイントステレオ
    JointStereo = 1,
    /// デュアルチャンネル
    DualChannel = 2,
    /// モノラル
    Monoral = 3,
}

/// 拡張チャンネルモード
pub enum MP3ExtChannelMode {
    /// インテンシティステレオ
    IntensityStereo = 0,
    /// MSステレオ
    MSStereo = 1,
    /// なにもしない
    NONE = 2,
}

/// レイヤー
pub enum MP3Layer {
    /// Layer1
    Layer1 = 1,
    /// Layer2
    Layer2 = 2,
    /// Layer3
    Layer3 = 3,
}

/// ビットレート(kbps)
#[repr(u32)]
#[derive(PartialEq, Copy, Clone)]
pub enum MP3BitRate {
    /// 0kbps
    Kbps0 = 0,
    /// 32kbps
    Kbps32 = 32_000,
    /// 40kbps
    Kbps40 = 40_000,
    /// 48kbps
    Kbps48 = 48_000,
    /// 56kbps
    Kbps56 = 56_000,
    /// 64kbps
    Kbps64 = 64_000,
    /// 80kbps
    Kbps80 = 80_000,
    /// 96kbps
    Kbps96 = 96_000,
    /// 112kbps
    Kbps112 = 112_000,
    /// 128kbps
    Kbps128 = 128_000,
    /// 160kbps
    Kbps160 = 160_000,
    /// 192kbps
    Kbps192 = 192_000,
    /// 224kbps
    Kbps224 = 224_000,
    /// 256kbps
    Kbps256 = 256_000,
    /// 320kbps
    Kbps320 = 320_000,
}

/// サンプリングレート
#[repr(u32)]
#[derive(PartialEq, Copy, Clone)]
pub enum MP3SamplingRate {
    /// 44.1kHz
    Hz44100 = 44100,
    /// 48.0kHz
    Hz48000 = 48000,
    /// 32.0kHz
    Hz32000 = 32000,
}

/// エンファシスモード
pub enum MP3EmphasisMode {
    /// なし
    NONE = 0,
    /// 50/15ms
    FiftyFifteenMs = 1,
    /// 予約済み
    Reserved = 2,
    /// CCITT J.17
    CCITTJ17 = 3,
}

/// フレームヘッダ情報
pub struct MP3FrameHeader {
    /// バージョン
    pub version: MPEGVersion,
    /// レイヤー
    pub layer: MP3Layer,
    /// エラー保護するか
    pub error_protection: bool,
    /// ビットレート
    pub bit_rate: MP3BitRate,
    /// サンプリングレート
    pub sampling_rate: MP3SamplingRate,
    /// フレームサイズ調整のパディングバイトがあるか
    pub padding: bool,
    /// ユーザが使用するデータ
    pub extension: u8,
    /// チャンネルモード
    pub channel_mode: MP3ChannelMode,
    /// 拡張チャンネルモード
    pub ext_channel_mode: MP3ExtChannelMode,
    /// コピーライト（trueであれば不正コピーを意味）
    pub copyright: bool,
    /// 原盤か否か
    pub original: bool,
    /// エンファシス適用モード
    pub emphasis: MP3EmphasisMode,
}

/// グラニュール情報
pub struct MP3GranuleInformation {
    /// スケールファクタとハフマン符号化されたビット数の和(12bit)
    pub part2_3_length: u16,
    /// この値の2倍がbigvalue_bandのサンプル数(9bit)
    pub big_values: u16,
    /// 量子化ステップを表すパラメータ(8bit)
    pub global_gain: u8,
    /// スケールファクタのビット幅のテーブルインデックス(4bit)
    pub scalefac_compress: u8,
    /// 1bit normalなら0, normalでないなら1
    pub window_switching_flag: bool,
    /// 窓関数タイプ
    pub block_type: MP3BlockType,
    /// 1bit(window_switching_flag == 1) mix typeのとき1
    pub mixed_block_flag: bool,
    /// big0_band, big1_band, big2_bandのハフマン符号化テーブルインデックス 10bit(window_switching_flag == 1), 5x3=15bit(window_switching_flag == 0)
    pub table_select: [u8; 3],
    /// 量子化ステップで使用(3x3=9bit)
    pub subblock_gain: [u8; 3],
    /// big1_bandの最小の周波数を定めるスケールファクタのバンドインデックス 4bit(window_switching_flag == 0)
    pub region0_count: u8,
    /// big2_bandの最小の周波数を定めるスケールファクタのバンドインデックス 3bit(window_switching_flag == 0)
    pub region1_count: u8,
    /// プリエンファシスで増幅されたら1, そうでなければ0 1bit
    pub preflag: bool,
    /// プリエンファシス, amp_scalefac_bandsで使われる値（dist10では常に0） 1bit
    pub scalefac_scale: u8,
    /// count1_bandのハフマン符号化テーブルインデックス 1bit
    pub count1table_select: u8,
}

/// チャンネルあたりのサイドインフォメーション（付加情報）
pub struct MP3ChannelSideInformation {
    /// ScaleFector Selection Information 4グループ(0-5,6-10,11-15,16-20)で、2グラニュールで同一のスケールファクタを使用しているか？ 1bit x4
    pub scfsi: [bool; 4],
    /// グラニュール情報
    pub gr: [MP3GranuleInformation; MP3_NUM_GRANLES_PER_FRAME],
}

/// サイドインフォメーション
pub struct MP3SideInformation {
    /// メインデータが始まるまでの負のオフセットバイト 9bit
    pub maindata_begin: u16,
    /// ステレオであれば3bit, モノラルであれば5bit ユーザ向けのビット(ISOは未使用)
    pub private_bits: u8,
    /// チャンネルあたりのサイドインフォメーション
    pub ch: [MP3ChannelSideInformation; MP3_MAX_NUM_CHANNELS],
}

/// フォーマット情報
#[derive(PartialEq)]
pub struct MP3FormatInformation {
    /// チャンネル数
    pub num_channels: usize,
    /// サンプル数
    pub num_samples: usize,
    /// サンプリングレート
    pub sampling_rate: MP3SamplingRate,
    /// ビットレート
    pub bit_rate: MP3BitRate,
}
