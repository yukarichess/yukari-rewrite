use std::{simd::{u16x64, u8x32, u8x64}, str::FromStr};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Colour {
    White = 0,
    Black = 1,
}

impl Colour {
    const MAX: usize = 2;
}

/// ```ignore
/// 0__ - leaper
/// 1_1 - diagonal slider
/// 11_ - horizontal slider
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum Piece {
    Pawn = 0b001,
    Knight = 0b010,
    Bishop = 0b101,
    Rook = 0b110,
    Queen = 0b111,
    King = 0b011,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Square(u8);

impl Square {
    const NONE: Square = Square(128);
}

/// ```ignore
/// c        - colour
///  iiii    - piece ID
///      ppp - piece type
/// ```
#[derive(Clone, Copy)]
struct Place(u8);

impl Place {
    pub fn colour(self) -> Colour {
        unsafe { std::mem::transmute::<u8, Colour>(self.0 >> 7) }
    }

    pub fn piece(self) -> Option<Piece> {
        let piece = self.0 & 0x07;
        if piece == 0 || piece == 4 {
            return None;
        }
        Some(unsafe { std::mem::transmute::<u8, Piece>(self.0 & 0x07) })
    }
}

pub struct Board {
    mailbox: u8x64,  // like [Place; 64]
    attacks: [u16x64; Colour::MAX],
    piece_type: u8x32, // like [Piece; 32]
    piece_square: u8x32, // like [Square; 32]
    side: Colour,
    castling: (Square, Square, Square, Square),
    en_passant: Square,
    halfmove: u8,
    fullmove: u16,
}

impl FromStr for Board {
    type Err = ();

    #[allow(clippy::too_many_lines)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut mailbox = [Place(0); 64];
        let mut piece_type = [None; 32];
        let mut piece_square = [Square::NONE; 32];
        let side;
        let mut castling = (Square::NONE, Square::NONE, Square::NONE, Square::NONE);
        let en_passant;
        let halfmove;
        let fullmove;
        let mut next_id = [1, 17];

        let mut s = s.chars();

        // Board
        for rank in (0..=7).rev() {
            let mut file = 0_u8;
            while file <= 7 {
                let c = s.next().unwrap();
                if ('1'..='8').contains(&c) {
                    let inc = u8::from_str(&c.to_string()).unwrap();
                    file += inc;
                    continue;
                }

                let side = if c.is_uppercase() { Colour::White } else { Colour::Black };
                let piece = match c.to_ascii_lowercase() {
                    'p' => Piece::Pawn,
                    'n' => Piece::Knight,
                    'b' => Piece::Bishop,
                    'r' => Piece::Rook,
                    'q' => Piece::Queen,
                    'k' => Piece::King,
                    _ => unreachable!("unrecognised piece character {c}"),
                };
                let square = 8*rank + file;
                let id = if piece == Piece::King {
                    16 * u8::from(side == Colour::Black)
                } else {
                    let id = next_id[side as usize];
                    next_id[side as usize] += 1;
                    id
                };
                let place = Place(((u8::from(side == Colour::Black)) << 7) | (id << 3) | (piece as u8));
                
                mailbox[square as usize] = place;
                piece_type[id as usize] = Some(piece);
                piece_square[id as usize] = Square(square);

                file += 1;
            }

            if rank > 0 {
                assert_eq!(s.next(), Some('/'));
            }
        }

        assert_eq!(s.next(), Some(' '));

        // Side to move
        {
            let c = s.next().unwrap();
            match c {
                'w' => side = Colour::White,
                'b' => side = Colour::Black,
                _ => unreachable!("unrecognised side character {c}"),
            }
            assert_eq!(s.next(), Some(' '));
        }

        // Castling
        {
            let mut c = s.next().unwrap();
            if c == '-' {
                castling = (Square::NONE, Square::NONE, Square::NONE, Square::NONE);
                assert_eq!(s.next(), Some(' '));
            } else {
                while c != ' ' {
                    let colour = if c.is_ascii_uppercase() {
                        Colour::White
                    } else {
                        Colour::Black
                    };
                    let king_square = piece_square[16 * usize::from(colour == Colour::Black)];
                    let mut rook_square = Square::NONE;

                    dbg!(c);
                    dbg!(king_square);

                    // we parse classical chess FENs as X-FEN, which defines KQkq to mean the outermost rooks.
                    if matches!(c, 'K' | 'Q' | 'k' | 'q') {
                        let dir = if c.eq_ignore_ascii_case(&'k') {
                            1
                        } else {
                            -1
                        };
                        let mut square = king_square.0 as i8 + dir;
                        while (dir == 1 && square % 8 != 0 && square <= 63) || (dir == -1 && square % 8 != 7 && square >= 0) {
                            let place = mailbox[square as usize];
                            dbg!((place.colour(), place.piece(), square));
                            if place.piece() == Some(Piece::Rook) && place.colour() == colour {
                                rook_square = Square(square as u8);
                            }
                            square += dir;
                        }
                    } else {
                        let file = c.to_ascii_lowercase() as u8 - b'a';
                        let rank = if colour == Colour::White {
                            0
                        } else {
                            7
                        };
                        rook_square = Square(8*rank + file);
                    }

                    assert!(rook_square != Square::NONE, "could not locate rook for castling type {c}");

                    match (colour, rook_square > king_square) {
                        (Colour::White, true) => castling.0 = rook_square,
                        (Colour::White, false) => castling.1 = rook_square,
                        (Colour::Black, true) => castling.2 = rook_square,
                        (Colour::Black, false) => castling.3 = rook_square,
                    }

                    c = s.next().unwrap();
                }
            }
        }

        // En-passant
        {
            let c = s.next().unwrap();
            if c == '-' {
                en_passant = Square::NONE;
            } else {
                let file = c as u8 - b'a';
                let c = s.next().unwrap();
                let rank = c as u8 - b'1';
                en_passant = Square(8*rank + file);
            }
            assert_eq!(s.next(), Some(' '));
        }

        // Halfmove counter
        {
            let mut hmc = String::new();
            for ch in s.by_ref() {
                if ch == ' ' {
                    break;
                }
                hmc.push(ch);
            }
            halfmove = u8::from_str(&hmc).unwrap();
        }

        // Fullmove clock
        {
            let mut fmv = String::new();
            for ch in s {
                fmv.push(ch);
            }
            fullmove = u16::from_str(&fmv).unwrap();
        }

        // SAFETY: reinterpreting specific values as their backing types is safe. I think.
        let mailbox = u8x64::from_array(unsafe { std::mem::transmute::<[Place; 64], [u8; 64]>(mailbox) });
        let piece_type = u8x32::from_array(unsafe { std::mem::transmute::<[Option<Piece>; 32], [u8; 32]>(piece_type) });
        let piece_square = u8x32::from_array(unsafe { std::mem::transmute::<[Square; 32], [u8; 32]>(piece_square) });

        Ok(Self {
            mailbox,
            attacks: [u16x64::splat(0); 2],
            piece_type,
            piece_square,
            side,
            castling,
            en_passant,
            halfmove,
            fullmove,
        })
    }
}

