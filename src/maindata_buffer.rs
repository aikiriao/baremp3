use bitreader::BitReader;

/// メインデータのバッファサイズ(byte)
pub const MP3_MAINDATA_BUFFER_SIZE: usize = 4096;
/// メインデータのビット換算量(bit)
pub const MP3_MAINDATA_BUFFER_SIZE_BITS: u64 = 8 * MP3_MAINDATA_BUFFER_SIZE as u64;

/// メインデータバッファ
pub struct MP3MainDataBuffer {
    /// データバッファ
    buffer: [u8; MP3_MAINDATA_BUFFER_SIZE],
    /// バッファ書き込み位置(byte)
    write_pos: usize,
    /// バッファ読み込み位置(!bit!)
    read_pos_bits: u64,
}

impl MP3MainDataBuffer {
    /// メインデータバッファの作成
    pub fn new() -> Self {
        Self {
            buffer: [0u8; MP3_MAINDATA_BUFFER_SIZE],
            write_pos: 0,
            read_pos_bits: 0,
        }
    }

    /// メインデータバッファのリセット
    pub fn reset(&mut self) {
        self.buffer.fill(0u8);
        self.write_pos = 0;
        self.read_pos_bits = 0;
    }

    /// 読み込んだビット数の計算
    pub fn get_total_read_bits(&self) -> u64 {
        self.read_pos_bits
    }

    /// データの入力
    pub fn put_data(&mut self, data: &[u8]) {
        let size = data.len();

        if (self.write_pos + size) > MP3_MAINDATA_BUFFER_SIZE {
            // バッファから飛び出る場合は、末尾まで書いた後に先頭に回り込む
            let tail_size = MP3_MAINDATA_BUFFER_SIZE - self.write_pos;
            let head_pos = size - tail_size;
            self.buffer[self.write_pos..self.write_pos + tail_size]
                .copy_from_slice(&data[..tail_size]);
            self.buffer[..head_pos].copy_from_slice(&data[tail_size..]);
            self.write_pos = head_pos;
        } else {
            self.buffer[self.write_pos..self.write_pos + size].copy_from_slice(data);
            self.write_pos += size;
        }
    }

    /// データ読み出し
    pub fn get_bits(&mut self, nbits: u8) -> u32 {
        if nbits == 0 {
            return 0;
        }
        // self.read_pos_bitsから読みだすビットリーダを生成
        let mut breader = BitReader::new(&self.buffer);
        breader.skip(self.read_pos_bits).unwrap();
        if self.read_pos_bits + nbits as u64 >= MP3_MAINDATA_BUFFER_SIZE_BITS {
            // バッファから飛び出る場合は、末尾まで読んだ後に再度先頭から読みだす
            let tail_bits = MP3_MAINDATA_BUFFER_SIZE_BITS - self.read_pos_bits;
            let tail = breader.read_u32(tail_bits as u8).unwrap();
            let remain_bits = nbits as u64 - tail_bits;
            let mut head_reader = BitReader::new(&self.buffer);
            let head = head_reader.read_u32(remain_bits as u8).unwrap();
            self.read_pos_bits = remain_bits;
            (tail << remain_bits) | head
        } else {
            let ret = breader.read_u32(nbits).unwrap();
            self.read_pos_bits += nbits as u64;
            ret
        }
    }

    /// 次のバイト境界に合わせる
    pub fn align_next_byte(&mut self) {
        // 8の倍数に切り上げ
        self.read_pos_bits = ((self.read_pos_bits + 7) >> 3) << 3;
        self.read_pos_bits %= MP3_MAINDATA_BUFFER_SIZE_BITS;
    }

    /// データの読み捨て
    pub fn skip(&mut self, nbits: u64) {
        self.read_pos_bits += nbits;
        self.read_pos_bits %= MP3_MAINDATA_BUFFER_SIZE_BITS;
    }

    /// ビット単位でのシーク
    pub fn seek(&mut self, position: u64) {
        self.read_pos_bits = position;
    }
}
