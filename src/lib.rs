// test時はno_stdを無効に設定
#![cfg_attr(not(test), no_std)]
pub mod types;
pub mod decoder;
mod maindata_buffer;
mod huffman;
mod hybrid_synthesis;
