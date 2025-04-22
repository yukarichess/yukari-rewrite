#![feature(portable_simd)]

#![warn(clippy::pedantic)]

use std::str::FromStr;

mod board;

fn main() {
    let _ = board::Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
    let _ = board::Board::from_str("rn2k1r1/ppp1pp1p/3p2p1/5bn1/P7/2N2B2/1PPPPP2/2BNK1RR w Gkq - 4 11").unwrap();

    println!("Hello, world!");
}
