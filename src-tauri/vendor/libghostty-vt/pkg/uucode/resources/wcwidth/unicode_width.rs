//! copyv: https://github.com/unicode-rs/unicode-width/blob/b10a4d5fbefe3f3dddd5e59cae2b889083ca58e7/src/lib.rs#L11-L169 begin
//! Determine displayed width of `char` and `str` types according to
//! [Unicode Standard Annex #11](http://www.unicode.org/reports/tr11/)
//! and other portions of the Unicode standard.
//! See the [Rules for determining width](#rules-for-determining-width) section
//! for the exact rules.
//!
//! This crate is `#![no_std]`.
//!
//! ```rust
//! use unicode_width::UnicodeWidthStr;
//!
//! let teststr = "Ｈｅｌｌｏ, ｗｏｒｌｄ!";
//! let width = UnicodeWidthStr::width(teststr);
//! println!("{}", teststr);
//! println!("The above string is {} columns wide.", width);
//! ```
//!
//! # `"cjk"` feature flag
//!
//! This crate has one Cargo feature flag, `"cjk"`
//! (enabled by default).
//! It enables the [`UnicodeWidthChar::width_cjk`]
//! and [`UnicodeWidthStr::width_cjk`],
//! which perform an alternate width calculation
//! more suited to CJK contexts. The flag also unseals the
//! [`UnicodeWidthChar`] and [`UnicodeWidthStr`] traits.
//!
//! Disabling the flag (with `no_default_features` in `Cargo.toml`)
//! will reduce the amount of static data needed by the crate.
//!
//! ```rust
//! use unicode_width::UnicodeWidthStr;
//!
//! let teststr = "“𘀀”";
//! assert_eq!(teststr.width(), 4);
//!
//! #[cfg(feature = "cjk")]
//! assert_eq!(teststr.width_cjk(), 6);
//! ```
//!
//! # Rules for determining width
//!
//! This crate currently uses the following rules to determine the width of a
//! character or string, in order of decreasing precedence. These may be tweaked in the future.
//!
//! 1. In the following cases, the width of a string differs from the sum of the widths of its constituent characters:
//!    - The sequence `"\r\n"` has width 1.
//!    - Emoji-specific ligatures:
//!      - Well-formed, fully-qualified [emoji ZWJ sequences] have width 2.
//!      - [Emoji modifier sequences] have width 2.
//!      - [Emoji presentation sequences] have width 2.
//!      - Outside of an East Asian context, [text presentation sequences] have width 1 if their base character:
//!        - Has the [`Emoji_Presentation`] property, and
//!        - Is not in the [Enclosed Ideographic Supplement] block.
//!    - [`'\u{2018}'`, `'\u{2019}'`, `'\u{201C}'`, and `'\u{201D}'`][General Punctuation] always have width 1
//!      when followed by '\u{FE00}' or '\u{FE02}', and width 2 when followed by '\u{FE01}'.
//!    - Script-specific ligatures:
//!      - For all the following ligatures, the insertion of any number of [default-ignorable][`Default_Ignorable_Code_Point`]
//!        [combining marks] anywhere in the sequence will not change the total width. In addition, for all non-Arabic
//!        ligatures, the insertion of any number of [`'\u{200D}'` ZERO WIDTH JOINER](https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-23/#G23126)s
//!        will not affect the width.
//!      - **[Arabic]**: A character sequence consisting of one character with [`Joining_Group`]`=Lam`,
//!        followed by any number of characters with [`Joining_Type`]`=Transparent`, followed by one character
//!        with [`Joining_Group`]`=Alef`, has total width 1. For example: `لا`‎, `لآ`‎, `ڸا`‎, `لٟٞأ`
//!      - **[Buginese]**: `"\u{1A15}\u{1A17}\u{200D}\u{1A10}"` (<a, -i> ya, `ᨕᨗ‍ᨐ`) has total width 1.
//!      - **[Hebrew]**: `"א\u{200D}ל"` (Alef-Lamed, `א‍ל`) has total width 1.
//!      - **[Khmer]**: Coeng signs consisting of `'\u{17D2}'` followed by a character in
//!        `'\u{1780}'..='\u{1782}' | '\u{1784}'..='\u{1787}' | '\u{1789}'..='\u{178C}' | '\u{178E}'..='\u{1793}' | '\u{1795}'..='\u{1798}' | '\u{179B}'..='\u{179D}' | '\u{17A0}' | '\u{17A2}'  | '\u{17A7}' | '\u{17AB}'..='\u{17AC}' | '\u{17AF}'`
//!        have width 0.
//!      - **[Kirat Rai]**: Any sequence canonically equivalent to `'\u{16D68}'`, `'\u{16D69}'`, or `'\u{16D6A}'` has total width 1.
//!      - **[Lisu]**: Tone letter combinations consisting of a character in the range `'\u{A4F8}'..='\u{A4FB}'`
//!        followed by a character in the range `'\u{A4FC}'..='\u{A4FD}'` have width 1. For example: `ꓹꓼ`
//!      - **[Old Turkic]**: `"\u{10C32}\u{200D}\u{10C03}"` (`𐰲‍𐰃`) has total width 1.
//!      - **[Tifinagh]**: A sequence of a Tifinagh consonant in the range `'\u{2D31}'..='\u{2D65}' | '\u{2D6F}'`, followed by either
//!        [`'\u{2D7F}'` TIFINAGH CONSONANT JOINER] or `'\u{200D}'`, followed by another Tifinangh consonant, has total width 1.
//!        For example: `ⵏ⵿ⴾ`
//!    - In an East Asian context only, `<`, `=`, or `>` have width 2 when followed by [`'\u{0338}'` COMBINING LONG SOLIDUS OVERLAY].
//!      The two characters may be separated by any number of characters whose canonical decompositions consist only of characters meeting
//!      one of the following requirements:
//!      - Has [`Canonical_Combining_Class`] greater than 1, or
//!      - Is a [default-ignorable][`Default_Ignorable_Code_Point`] [combining mark][combining marks].
//! 2. In all other cases, the width of the string equals the sum of its character widths:
//!    1. [`'\u{2D7F}'` TIFINAGH CONSONANT JOINER] has width 1 (outside of the ligatures described previously).
//!    2. [`'\u{115F}'` HANGUL CHOSEONG FILLER](https://util.unicode.org/UnicodeJsps/character.jsp?a=115F) and
//!       [`'\u{17A4}'` KHMER INDEPENDENT VOWEL QAA](https://util.unicode.org/UnicodeJsps/character.jsp?a=17A4) have width 2.
//!    3. [`'\u{17D8}'` KHMER SIGN BEYYAL](https://util.unicode.org/UnicodeJsps/character.jsp?a=17D8) has width 3.
//!    4. The following have width 0:
//!       - [Characters](https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5Cp%7BDefault_Ignorable_Code_Point%7D)
//!         with the [`Default_Ignorable_Code_Point`] property.
//!       - [Characters](https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5Cp%7BGrapheme_Extend%7D)
//!         with the [`Grapheme_Extend`] property.
//!       - [Characters](https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5Cp%7BHangul_Syllable_Type%3DV%7D%5Cp%7BHangul_Syllable_Type%3DT%7D)
//!         with a [`Hangul_Syllable_Type`] of `Vowel_Jamo` (`V`) or `Trailing_Jamo` (`T`).
//!       - The following [`Prepended_Concatenation_Mark`]s:
//!         - [`'\u{0605}'` NUMBER MARK ABOVE](https://util.unicode.org/UnicodeJsps/character.jsp?a=0605),
//!         - [`'\u{070F}'` SYRIAC ABBREVIATION MARK](https://util.unicode.org/UnicodeJsps/character.jsp?a=070F),
//!         - [`'\u{0890}'` POUND MARK ABOVE](https://util.unicode.org/UnicodeJsps/character.jsp?a=0890),
//!         - [`'\u{0891}'` PIASTRE MARK ABOVE](https://util.unicode.org/UnicodeJsps/character.jsp?a=0891), and
//!         - [`'\u{08E2}'` DISPUTED END OF AYAH](https://util.unicode.org/UnicodeJsps/character.jsp?a=08E2).
//!       - [Characters](https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5Cp%7BGrapheme_Cluster_Break%3DPrepend%7D-%5Cp%7BPrepended_Concatenation_Mark%7D)
//!         with the [`Grapheme_Extend=Prepend`] property, that are not also [`Prepended_Concatenation_Mark`]s.
//!       - [`'\u{A8FA}'` DEVANAGARI CARET](https://util.unicode.org/UnicodeJsps/character.jsp?a=A8FA).
//!    5. [Characters](https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5Cp%7BEast_Asian_Width%3DF%7D%5Cp%7BEast_Asian_Width%3DW%7D)
//!       with an [`East_Asian_Width`] of [`Fullwidth`] or [`Wide`] have width 2.
//!    6. Characters fulfilling all of the following conditions have width 2 in an East Asian context, and width 1 otherwise:
//!       - Fulfills one of the following conditions:
//!         - Has an [`East_Asian_Width`] of [`Ambiguous`], or
//!         - Has a [`Line_Break`] of [`AI`], or
//!         - Has a canonical decomposition to an [`Ambiguous`] character followed by [`'\u{0338}'` COMBINING LONG SOLIDUS OVERLAY], or
//!         - Is [`'\u{0387}'` GREEK ANO TELEIA](https://util.unicode.org/UnicodeJsps/character.jsp?a=0387); and
//!       - Does not have a [`General_Category`] of `Letter` or `Modifier_Symbol`.
//!    7. All other characters have width 1.
//!
//! [`'\u{0338}'` COMBINING LONG SOLIDUS OVERLAY]: https://util.unicode.org/UnicodeJsps/character.jsp?a=0338
//! [`'\u{2D7F}'` TIFINAGH CONSONANT JOINER]: https://util.unicode.org/UnicodeJsps/character.jsp?a=2D7F
//!
//! [`Canonical_Combining_Class`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-3/#G50313
//! [`Default_Ignorable_Code_Point`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-5/#G40095
//! [`East_Asian_Width`]: https://www.unicode.org/reports/tr11/#ED1
//! [`Emoji_Presentation`]: https://unicode.org/reports/tr51/#def_emoji_presentation
//! [`General_Category`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-4/#G124142
//! [`Grapheme_Extend=Prepend`]: https://www.unicode.org/reports/tr29/#Prepend
//! [`Grapheme_Extend`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-3/#G52443
//! [`Hangul_Syllable_Type`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-3/#G45593
//! [`Joining_Group`]: https://www.unicode.org/versions/Unicode14.0.0/ch09.pdf#G36862
//! [`Joining_Type`]: http://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-9/#G50009
//! [`Line_Break`]: https://www.unicode.org/reports/tr14/#LD5
//! [`Prepended_Concatenation_Mark`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-23/#G37908
//! [`Script`]: https://www.unicode.org/reports/tr24/#Script
//!
//! [`Fullwidth`]: https://www.unicode.org/reports/tr11/#ED2
//! [`Wide`]: https://www.unicode.org/reports/tr11/#ED4
//! [`Ambiguous`]: https://www.unicode.org/reports/tr11/#ED6
//!
//! [`AI`]: https://www.unicode.org/reports/tr14/#AI
//!
//! [combining marks]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-3/#G30602
//!
//! [emoji ZWJ sequences]: https://www.unicode.org/reports/tr51/#def_emoji_sequence
//! [Emoji modifier sequences]: https://www.unicode.org/reports/tr51/#def_emoji_modifier_sequence
//! [Emoji presentation sequences]: https://unicode.org/reports/tr51/#def_emoji_presentation_sequence
//! [text presentation sequences]: https://unicode.org/reports/tr51/#def_text_presentation_sequence
//!
//! [General Punctuation]: https://www.unicode.org/charts/PDF/Unicode-16.0/U160-2000.pdf
//! [Enclosed Ideographic Supplement]: https://unicode.org/charts/nameslist/n_1F200.html
//!
//! [Arabic]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-9/#G7480
//! [Buginese]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-17/#G26743
//! [Hebrew]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-9/#G6528
//! [Khmer]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-16/#G64642
//! [Kirat Rai]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-13/#G746409
//! [Lisu]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-18/#G44587
//! [Old Turkic]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-14/#G41975
//! [Tifinagh]: http://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-19/#G43184
//!
//!
//! ## Canonical equivalence
//!
//! Canonically equivalent strings are assigned the same width (CJK and non-CJK).
// copyv: end

// copyv: https://github.com/unicode-rs/unicode-width/blob/b10a4d5fbefe3f3dddd5e59cae2b889083ca58e7/src/tables.rs#L1-L944 begin
// Copyright 2012-2025 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// NOTE: The following code was generated by "scripts/unicode.py", do not edit directly

use core::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WidthInfo(u16);

const LIGATURE_TRANSPARENT_MASK: u16 = 0b0010_0000_0000_0000;

impl WidthInfo {
    /// No special handling necessary
    const DEFAULT: Self = Self(0);
    const LINE_FEED: Self = Self(0b0000000000000001);
    const EMOJI_MODIFIER: Self = Self(0b0000000000000010);
    const REGIONAL_INDICATOR: Self = Self(0b0000000000000011);
    const SEVERAL_REGIONAL_INDICATOR: Self = Self(0b0000000000000100);
    const EMOJI_PRESENTATION: Self = Self(0b0000000000000101);
    const ZWJ_EMOJI_PRESENTATION: Self = Self(0b0001000000000110);
    const VS16_ZWJ_EMOJI_PRESENTATION: Self = Self(0b1001000000000110);
    const KEYCAP_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0001000000000111);
    const VS16_KEYCAP_ZWJ_EMOJI_PRESENTATION: Self = Self(0b1001000000000111);
    const REGIONAL_INDICATOR_ZWJ_PRESENTATION: Self = Self(0b0000000000001001);
    const EVEN_REGIONAL_INDICATOR_ZWJ_PRESENTATION: Self = Self(0b0000000000001010);
    const ODD_REGIONAL_INDICATOR_ZWJ_PRESENTATION: Self = Self(0b0000000000001011);
    const TAG_END_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0000000000010000);
    const TAG_D1_END_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0000000000010001);
    const TAG_D2_END_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0000000000010010);
    const TAG_D3_END_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0000000000010011);
    const TAG_A1_END_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0000000000011001);
    const TAG_A2_END_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0000000000011010);
    const TAG_A3_END_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0000000000011011);
    const TAG_A4_END_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0000000000011100);
    const TAG_A5_END_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0000000000011101);
    const TAG_A6_END_ZWJ_EMOJI_PRESENTATION: Self = Self(0b0000000000011110);
    const KIRAT_RAI_VOWEL_SIGN_E: Self = Self(0b0000000000100000);
    const KIRAT_RAI_VOWEL_SIGN_AI: Self = Self(0b0000000000100001);
    const VARIATION_SELECTOR_1_2_OR_3: Self = Self(0b0000001000000000);
    const VARIATION_SELECTOR_15: Self = Self(0b0100000000000000);
    const VARIATION_SELECTOR_16: Self = Self(0b1000000000000000);
    const JOINING_GROUP_ALEF: Self = Self(0b0011000011111111);
    #[cfg(feature = "cjk")]
    const COMBINING_LONG_SOLIDUS_OVERLAY: Self = Self(0b0011110011111111);
    #[cfg(feature = "cjk")]
    const SOLIDUS_OVERLAY_ALEF: Self = Self(0b0011100011111111);
    const HEBREW_LETTER_LAMED: Self = Self(0b0011100000000000);
    const ZWJ_HEBREW_LETTER_LAMED: Self = Self(0b0011110000000000);
    const BUGINESE_LETTER_YA: Self = Self(0b0011100000000001);
    const ZWJ_BUGINESE_LETTER_YA: Self = Self(0b0011110000000001);
    const BUGINESE_VOWEL_SIGN_I_ZWJ_LETTER_YA: Self = Self(0b0011110000000010);
    const TIFINAGH_CONSONANT: Self = Self(0b0011100000000011);
    const ZWJ_TIFINAGH_CONSONANT: Self = Self(0b0011110000000011);
    const TIFINAGH_JOINER_CONSONANT: Self = Self(0b0011110000000100);
    const LISU_TONE_LETTER_MYA_NA_JEU: Self = Self(0b0011110000000101);
    const OLD_TURKIC_LETTER_ORKHON_I: Self = Self(0b0011100000000110);
    const ZWJ_OLD_TURKIC_LETTER_ORKHON_I: Self = Self(0b0011110000000110);
    const KHMER_COENG_ELIGIBLE_LETTER: Self = Self(0b0011110000000111);

    /// Whether this width mode is ligature_transparent
    /// (has 5th MSB set.)
    fn is_ligature_transparent(self) -> bool {
        (self.0 & 0b0000_1000_0000_0000) == 0b0000_1000_0000_0000
    }

    /// Sets 6th MSB.
    fn set_zwj_bit(self) -> Self {
        Self(self.0 | 0b0000_0100_0000_0000)
    }

    /// Has top bit set
    fn is_emoji_presentation(self) -> bool {
        (self.0 & WidthInfo::VARIATION_SELECTOR_16.0) == WidthInfo::VARIATION_SELECTOR_16.0
    }

    fn is_zwj_emoji_presentation(self) -> bool {
        (self.0 & 0b1011_0000_0000_0000) == 0b1001_0000_0000_0000
    }

    /// Set top bit
    fn set_emoji_presentation(self) -> Self {
        if (self.0 & LIGATURE_TRANSPARENT_MASK) == LIGATURE_TRANSPARENT_MASK
            || (self.0 & 0b1001_0000_0000_0000) == 0b0001_0000_0000_0000
        {
            Self(
                self.0
                    | WidthInfo::VARIATION_SELECTOR_16.0
                        & !WidthInfo::VARIATION_SELECTOR_15.0
                        & !WidthInfo::VARIATION_SELECTOR_1_2_OR_3.0,
            )
        } else {
            Self::VARIATION_SELECTOR_16
        }
    }

    /// Clear top bit
    fn unset_emoji_presentation(self) -> Self {
        if (self.0 & LIGATURE_TRANSPARENT_MASK) == LIGATURE_TRANSPARENT_MASK {
            Self(self.0 & !WidthInfo::VARIATION_SELECTOR_16.0)
        } else {
            Self::DEFAULT
        }
    }

    /// Has 2nd bit set
    fn is_text_presentation(self) -> bool {
        (self.0 & WidthInfo::VARIATION_SELECTOR_15.0) == WidthInfo::VARIATION_SELECTOR_15.0
    }

    /// Set 2nd bit
    fn set_text_presentation(self) -> Self {
        if (self.0 & LIGATURE_TRANSPARENT_MASK) == LIGATURE_TRANSPARENT_MASK {
            Self(
                self.0
                    | WidthInfo::VARIATION_SELECTOR_15.0
                        & !WidthInfo::VARIATION_SELECTOR_16.0
                        & !WidthInfo::VARIATION_SELECTOR_1_2_OR_3.0,
            )
        } else {
            Self(WidthInfo::VARIATION_SELECTOR_15.0)
        }
    }

    /// Clear 2nd bit
    fn unset_text_presentation(self) -> Self {
        Self(self.0 & !WidthInfo::VARIATION_SELECTOR_15.0)
    }

    /// Has 7th bit set
    fn is_vs1_2_3(self) -> bool {
        (self.0 & WidthInfo::VARIATION_SELECTOR_1_2_OR_3.0)
            == WidthInfo::VARIATION_SELECTOR_1_2_OR_3.0
    }

    /// Set 7th bit
    fn set_vs1_2_3(self) -> Self {
        if (self.0 & LIGATURE_TRANSPARENT_MASK) == LIGATURE_TRANSPARENT_MASK {
            Self(
                self.0
                    | WidthInfo::VARIATION_SELECTOR_1_2_OR_3.0
                        & !WidthInfo::VARIATION_SELECTOR_15.0
                        & !WidthInfo::VARIATION_SELECTOR_16.0,
            )
        } else {
            Self(WidthInfo::VARIATION_SELECTOR_1_2_OR_3.0)
        }
    }

    /// Clear 7th bit
    fn unset_vs1_2_3(self) -> Self {
        Self(self.0 & !WidthInfo::VARIATION_SELECTOR_1_2_OR_3.0)
    }
}

/// The version of [Unicode](http://www.unicode.org/)
/// that this version of unicode-width is based on.
pub const UNICODE_VERSION: (u8, u8, u8) = (17, 0, 0);

/// Returns the [UAX #11](https://www.unicode.org/reports/tr11/) based width of `c` by
/// consulting a multi-level lookup table.
///
/// # Maintenance
/// The tables themselves are autogenerated but this function is hardcoded. You should have
/// nothing to worry about if you re-run `unicode.py` (for example, when updating Unicode.)
/// However, if you change the *actual structure* of the lookup tables (perhaps by editing the
/// `make_tables` function in `unicode.py`) you must ensure that this code reflects those changes.
#[inline]
fn lookup_width(c: char) -> (u8, WidthInfo) {
    let cp = c as usize;

    let t1_offset = WIDTH_ROOT.0[cp >> 13];

    // Each sub-table in WIDTH_MIDDLE is 7 bits, and each stored entry is a byte,
    // so each sub-table is 128 bytes in size.
    // (Sub-tables are selected using the computed offset from the previous table.)
    let t2_offset = WIDTH_MIDDLE.0[usize::from(t1_offset)][cp >> 7 & 0x3F];

    // Each sub-table in WIDTH_LEAVES is 6 bits, but each stored entry is 2 bits.
    // This is accomplished by packing four stored entries into one byte.
    // So each sub-table is 2**(7-2) == 32 bytes in size.
    // Since this is the last table, each entry represents an encoded width.
    let packed_widths = WIDTH_LEAVES.0[usize::from(t2_offset)][cp >> 2 & 0x1F];

    // Extract the packed width
    let width = packed_widths >> (2 * (cp & 0b11)) & 0b11;

    if width < 3 {
        (width, WidthInfo::DEFAULT)
    } else {
        match c {
            '\u{A}' => (1, WidthInfo::LINE_FEED),
            '\u{5DC}' => (1, WidthInfo::HEBREW_LETTER_LAMED),
            '\u{622}'..='\u{882}' => (1, WidthInfo::JOINING_GROUP_ALEF),
            '\u{1780}'..='\u{17AF}' => (1, WidthInfo::KHMER_COENG_ELIGIBLE_LETTER),
            '\u{17D8}' => (3, WidthInfo::DEFAULT),
            '\u{1A10}' => (1, WidthInfo::BUGINESE_LETTER_YA),
            '\u{2D31}'..='\u{2D6F}' => (1, WidthInfo::TIFINAGH_CONSONANT),
            '\u{A4FC}'..='\u{A4FD}' => (1, WidthInfo::LISU_TONE_LETTER_MYA_NA_JEU),
            '\u{FE01}' => (0, WidthInfo::VARIATION_SELECTOR_1_2_OR_3),
            '\u{FE0E}' => (0, WidthInfo::VARIATION_SELECTOR_15),
            '\u{FE0F}' => (0, WidthInfo::VARIATION_SELECTOR_16),
            '\u{10C03}' => (1, WidthInfo::OLD_TURKIC_LETTER_ORKHON_I),
            '\u{16D67}' => (1, WidthInfo::KIRAT_RAI_VOWEL_SIGN_E),
            '\u{16D68}' => (1, WidthInfo::KIRAT_RAI_VOWEL_SIGN_AI),
            '\u{1F1E6}'..='\u{1F1FF}' => (1, WidthInfo::REGIONAL_INDICATOR),
            '\u{1F3FB}'..='\u{1F3FF}' => (2, WidthInfo::EMOJI_MODIFIER),
            _ => (2, WidthInfo::EMOJI_PRESENTATION),
        }
    }
}

/// Returns the [UAX #11](https://www.unicode.org/reports/tr11/) based width of `c`, or
/// `None` if `c` is a control character.
/// Ambiguous width characters are treated as narrow.
#[inline]
pub fn single_char_width(c: char) -> Option<usize> {
    if c < '\u{7F}' {
        if c >= '\u{20}' {
            // U+0020 to U+007F (exclusive) are single-width ASCII codepoints
            Some(1)
        } else {
            // U+0000 to U+0020 (exclusive) are control codes
            None
        }
    } else if c >= '\u{A0}' {
        // No characters >= U+00A0 are control codes, so we can consult the lookup tables
        Some(lookup_width(c).0.into())
    } else {
        // U+007F to U+00A0 (exclusive) are control codes
        None
    }
}

/// Returns the [UAX #11](https://www.unicode.org/reports/tr11/) based width of `c`.
/// Ambiguous width characters are treated as narrow.
#[inline]
fn width_in_str(c: char, mut next_info: WidthInfo) -> (i8, WidthInfo) {
    if next_info.is_emoji_presentation() {
        if starts_emoji_presentation_seq(c) {
            let width = if next_info.is_zwj_emoji_presentation() {
                0
            } else {
                2
            };
            return (width, WidthInfo::EMOJI_PRESENTATION);
        } else {
            next_info = next_info.unset_emoji_presentation();
        }
    }
    if c <= '\u{A0}' {
        match c {
            '\n' => (1, WidthInfo::LINE_FEED),
            '\r' if next_info == WidthInfo::LINE_FEED => (0, WidthInfo::DEFAULT),
            _ => (1, WidthInfo::DEFAULT),
        }
    } else {
        // Fast path
        if next_info != WidthInfo::DEFAULT {
            if c == '\u{FE0F}' {
                return (0, next_info.set_emoji_presentation());
            }
            if c == '\u{FE01}' {
                return (0, next_info.set_vs1_2_3());
            }
            if c == '\u{FE0E}' {
                return (0, next_info.set_text_presentation());
            }
            if next_info.is_text_presentation() {
                if starts_non_ideographic_text_presentation_seq(c) {
                    return (1, WidthInfo::DEFAULT);
                } else {
                    next_info = next_info.unset_text_presentation();
                }
            } else if next_info.is_vs1_2_3() {
                if matches!(c, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}') {
                    return (2, WidthInfo::DEFAULT);
                } else {
                    next_info = next_info.unset_vs1_2_3();
                }
            }
            if next_info.is_ligature_transparent() {
                if c == '\u{200D}' {
                    return (0, next_info.set_zwj_bit());
                } else if is_ligature_transparent(c) {
                    return (0, next_info);
                }
            }

            match (next_info, c) {
                // Arabic Lam-Alef ligature
                (
                    WidthInfo::JOINING_GROUP_ALEF,
                    '\u{644}' | '\u{6B5}'..='\u{6B8}' | '\u{76A}' | '\u{8A6}' | '\u{8C7}',
                ) => return (0, WidthInfo::DEFAULT),
                (WidthInfo::JOINING_GROUP_ALEF, _) if is_transparent_zero_width(c) => {
                    return (0, WidthInfo::JOINING_GROUP_ALEF);
                }

                // Hebrew Alef-ZWJ-Lamed ligature
                (WidthInfo::ZWJ_HEBREW_LETTER_LAMED, '\u{05D0}') => {
                    return (0, WidthInfo::DEFAULT);
                }

                // Khmer coeng signs
                (WidthInfo::KHMER_COENG_ELIGIBLE_LETTER, '\u{17D2}') => {
                    return (-1, WidthInfo::DEFAULT);
                }

                // Buginese <a, -i> ZWJ ya ligature
                (WidthInfo::ZWJ_BUGINESE_LETTER_YA, '\u{1A17}') => {
                    return (0, WidthInfo::BUGINESE_VOWEL_SIGN_I_ZWJ_LETTER_YA)
                }
                (WidthInfo::BUGINESE_VOWEL_SIGN_I_ZWJ_LETTER_YA, '\u{1A15}') => {
                    return (0, WidthInfo::DEFAULT)
                }

                // Tifinagh bi-consonants
                (WidthInfo::TIFINAGH_CONSONANT | WidthInfo::ZWJ_TIFINAGH_CONSONANT, '\u{2D7F}') => {
                    return (1, WidthInfo::TIFINAGH_JOINER_CONSONANT);
                }
                (WidthInfo::ZWJ_TIFINAGH_CONSONANT, '\u{2D31}'..='\u{2D65}' | '\u{2D6F}') => {
                    return (0, WidthInfo::DEFAULT);
                }
                (WidthInfo::TIFINAGH_JOINER_CONSONANT, '\u{2D31}'..='\u{2D65}' | '\u{2D6F}') => {
                    return (-1, WidthInfo::DEFAULT);
                }

                // Lisu tone letter combinations
                (WidthInfo::LISU_TONE_LETTER_MYA_NA_JEU, '\u{A4F8}'..='\u{A4FB}') => {
                    return (0, WidthInfo::DEFAULT);
                }

                // Old Turkic ligature
                (WidthInfo::ZWJ_OLD_TURKIC_LETTER_ORKHON_I, '\u{10C32}') => {
                    return (0, WidthInfo::DEFAULT);
                }
                // Emoji modifier
                (WidthInfo::EMOJI_MODIFIER, _) if is_emoji_modifier_base(c) => {
                    return (0, WidthInfo::EMOJI_PRESENTATION);
                }

                // Regional indicator
                (
                    WidthInfo::REGIONAL_INDICATOR | WidthInfo::SEVERAL_REGIONAL_INDICATOR,
                    '\u{1F1E6}'..='\u{1F1FF}',
                ) => return (1, WidthInfo::SEVERAL_REGIONAL_INDICATOR),

                // ZWJ emoji
                (
                    WidthInfo::EMOJI_PRESENTATION
                    | WidthInfo::SEVERAL_REGIONAL_INDICATOR
                    | WidthInfo::EVEN_REGIONAL_INDICATOR_ZWJ_PRESENTATION
                    | WidthInfo::ODD_REGIONAL_INDICATOR_ZWJ_PRESENTATION
                    | WidthInfo::EMOJI_MODIFIER,
                    '\u{200D}',
                ) => return (0, WidthInfo::ZWJ_EMOJI_PRESENTATION),
                (WidthInfo::ZWJ_EMOJI_PRESENTATION, '\u{20E3}') => {
                    return (0, WidthInfo::KEYCAP_ZWJ_EMOJI_PRESENTATION);
                }
                (WidthInfo::VS16_ZWJ_EMOJI_PRESENTATION, _) if starts_emoji_presentation_seq(c) => {
                    return (0, WidthInfo::EMOJI_PRESENTATION)
                }
                (WidthInfo::VS16_KEYCAP_ZWJ_EMOJI_PRESENTATION, '0'..='9' | '#' | '*') => {
                    return (0, WidthInfo::EMOJI_PRESENTATION)
                }
                (WidthInfo::ZWJ_EMOJI_PRESENTATION, '\u{1F1E6}'..='\u{1F1FF}') => {
                    return (1, WidthInfo::REGIONAL_INDICATOR_ZWJ_PRESENTATION);
                }
                (
                    WidthInfo::REGIONAL_INDICATOR_ZWJ_PRESENTATION
                    | WidthInfo::ODD_REGIONAL_INDICATOR_ZWJ_PRESENTATION,
                    '\u{1F1E6}'..='\u{1F1FF}',
                ) => return (-1, WidthInfo::EVEN_REGIONAL_INDICATOR_ZWJ_PRESENTATION),
                (
                    WidthInfo::EVEN_REGIONAL_INDICATOR_ZWJ_PRESENTATION,
                    '\u{1F1E6}'..='\u{1F1FF}',
                ) => return (3, WidthInfo::ODD_REGIONAL_INDICATOR_ZWJ_PRESENTATION),
                (WidthInfo::ZWJ_EMOJI_PRESENTATION, '\u{1F3FB}'..='\u{1F3FF}') => {
                    return (0, WidthInfo::EMOJI_MODIFIER);
                }
                (WidthInfo::ZWJ_EMOJI_PRESENTATION, '\u{E007F}') => {
                    return (0, WidthInfo::TAG_END_ZWJ_EMOJI_PRESENTATION);
                }
                (WidthInfo::TAG_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A1_END_ZWJ_EMOJI_PRESENTATION);
                }
                (WidthInfo::TAG_A1_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A2_END_ZWJ_EMOJI_PRESENTATION)
                }
                (WidthInfo::TAG_A2_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A3_END_ZWJ_EMOJI_PRESENTATION)
                }
                (WidthInfo::TAG_A3_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A4_END_ZWJ_EMOJI_PRESENTATION)
                }
                (WidthInfo::TAG_A4_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A5_END_ZWJ_EMOJI_PRESENTATION)
                }
                (WidthInfo::TAG_A5_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A6_END_ZWJ_EMOJI_PRESENTATION)
                }
                (
                    WidthInfo::TAG_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A1_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A2_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A3_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A4_END_ZWJ_EMOJI_PRESENTATION,
                    '\u{E0030}'..='\u{E0039}',
                ) => return (0, WidthInfo::TAG_D1_END_ZWJ_EMOJI_PRESENTATION),
                (WidthInfo::TAG_D1_END_ZWJ_EMOJI_PRESENTATION, '\u{E0030}'..='\u{E0039}') => {
                    return (0, WidthInfo::TAG_D2_END_ZWJ_EMOJI_PRESENTATION);
                }
                (WidthInfo::TAG_D2_END_ZWJ_EMOJI_PRESENTATION, '\u{E0030}'..='\u{E0039}') => {
                    return (0, WidthInfo::TAG_D3_END_ZWJ_EMOJI_PRESENTATION);
                }
                (
                    WidthInfo::TAG_A3_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A4_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A5_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A6_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_D3_END_ZWJ_EMOJI_PRESENTATION,
                    '\u{1F3F4}',
                ) => return (0, WidthInfo::EMOJI_PRESENTATION),
                (WidthInfo::ZWJ_EMOJI_PRESENTATION, _)
                    if lookup_width(c).1 == WidthInfo::EMOJI_PRESENTATION =>
                {
                    return (0, WidthInfo::EMOJI_PRESENTATION)
                }

                (WidthInfo::KIRAT_RAI_VOWEL_SIGN_E, '\u{16D63}') => {
                    return (0, WidthInfo::DEFAULT);
                }
                (WidthInfo::KIRAT_RAI_VOWEL_SIGN_E, '\u{16D67}') => {
                    return (0, WidthInfo::KIRAT_RAI_VOWEL_SIGN_AI);
                }
                (WidthInfo::KIRAT_RAI_VOWEL_SIGN_E, '\u{16D68}') => {
                    return (1, WidthInfo::KIRAT_RAI_VOWEL_SIGN_E);
                }
                (WidthInfo::KIRAT_RAI_VOWEL_SIGN_E, '\u{16D69}') => {
                    return (0, WidthInfo::DEFAULT);
                }
                (WidthInfo::KIRAT_RAI_VOWEL_SIGN_AI, '\u{16D63}') => {
                    return (0, WidthInfo::DEFAULT);
                }

                // Fallback
                _ => {}
            }
        }

        let ret = lookup_width(c);
        (ret.0 as i8, ret.1)
    }
}

#[inline]
pub fn str_width(s: &str) -> usize {
    s.chars()
        .rfold(
            (0, WidthInfo::DEFAULT),
            |(sum, next_info), c| -> (usize, WidthInfo) {
                let (add, info) = width_in_str(c, next_info);
                (sum.wrapping_add_signed(isize::from(add)), info)
            },
        )
        .0
}

/// Returns the [UAX #11](https://www.unicode.org/reports/tr11/) based width of `c` by
/// consulting a multi-level lookup table.
///
/// # Maintenance
/// The tables themselves are autogenerated but this function is hardcoded. You should have
/// nothing to worry about if you re-run `unicode.py` (for example, when updating Unicode.)
/// However, if you change the *actual structure* of the lookup tables (perhaps by editing the
/// `make_tables` function in `unicode.py`) you must ensure that this code reflects those changes.
#[cfg(feature = "cjk")]
#[inline]
fn lookup_width_cjk(c: char) -> (u8, WidthInfo) {
    let cp = c as usize;

    let t1_offset = WIDTH_ROOT_CJK.0[cp >> 13];

    // Each sub-table in WIDTH_MIDDLE is 7 bits, and each stored entry is a byte,
    // so each sub-table is 128 bytes in size.
    // (Sub-tables are selected using the computed offset from the previous table.)
    let t2_offset = WIDTH_MIDDLE.0[usize::from(t1_offset)][cp >> 7 & 0x3F];

    // Each sub-table in WIDTH_LEAVES is 6 bits, but each stored entry is 2 bits.
    // This is accomplished by packing four stored entries into one byte.
    // So each sub-table is 2**(7-2) == 32 bytes in size.
    // Since this is the last table, each entry represents an encoded width.
    let packed_widths = WIDTH_LEAVES.0[usize::from(t2_offset)][cp >> 2 & 0x1F];

    // Extract the packed width
    let width = packed_widths >> (2 * (cp & 0b11)) & 0b11;

    if width < 3 {
        (width, WidthInfo::DEFAULT)
    } else {
        match c {
            '\u{A}' => (1, WidthInfo::LINE_FEED),
            '\u{338}' => (0, WidthInfo::COMBINING_LONG_SOLIDUS_OVERLAY),
            '\u{5DC}' => (1, WidthInfo::HEBREW_LETTER_LAMED),
            '\u{622}'..='\u{882}' => (1, WidthInfo::JOINING_GROUP_ALEF),
            '\u{1780}'..='\u{17AF}' => (1, WidthInfo::KHMER_COENG_ELIGIBLE_LETTER),
            '\u{17D8}' => (3, WidthInfo::DEFAULT),
            '\u{1A10}' => (1, WidthInfo::BUGINESE_LETTER_YA),
            '\u{2D31}'..='\u{2D6F}' => (1, WidthInfo::TIFINAGH_CONSONANT),
            '\u{A4FC}'..='\u{A4FD}' => (1, WidthInfo::LISU_TONE_LETTER_MYA_NA_JEU),
            '\u{FE00}'..='\u{FE02}' => (0, WidthInfo::VARIATION_SELECTOR_1_2_OR_3),
            '\u{FE0F}' => (0, WidthInfo::VARIATION_SELECTOR_16),
            '\u{10C03}' => (1, WidthInfo::OLD_TURKIC_LETTER_ORKHON_I),
            '\u{16D67}' => (1, WidthInfo::KIRAT_RAI_VOWEL_SIGN_E),
            '\u{16D68}' => (1, WidthInfo::KIRAT_RAI_VOWEL_SIGN_AI),
            '\u{1F1E6}'..='\u{1F1FF}' => (1, WidthInfo::REGIONAL_INDICATOR),
            '\u{1F3FB}'..='\u{1F3FF}' => (2, WidthInfo::EMOJI_MODIFIER),
            _ => (2, WidthInfo::EMOJI_PRESENTATION),
        }
    }
}

/// Returns the [UAX #11](https://www.unicode.org/reports/tr11/) based width of `c`, or
/// `None` if `c` is a control character.
/// Ambiguous width characters are treated as wide.
#[cfg(feature = "cjk")]
#[inline]
pub fn single_char_width_cjk(c: char) -> Option<usize> {
    if c < '\u{7F}' {
        if c >= '\u{20}' {
            // U+0020 to U+007F (exclusive) are single-width ASCII codepoints
            Some(1)
        } else {
            // U+0000 to U+0020 (exclusive) are control codes
            None
        }
    } else if c >= '\u{A0}' {
        // No characters >= U+00A0 are control codes, so we can consult the lookup tables
        Some(lookup_width_cjk(c).0.into())
    } else {
        // U+007F to U+00A0 (exclusive) are control codes
        None
    }
}

/// Returns the [UAX #11](https://www.unicode.org/reports/tr11/) based width of `c`.
/// Ambiguous width characters are treated as wide.
#[cfg(feature = "cjk")]
#[inline]
fn width_in_str_cjk(c: char, mut next_info: WidthInfo) -> (i8, WidthInfo) {
    if next_info.is_emoji_presentation() {
        if starts_emoji_presentation_seq(c) {
            let width = if next_info.is_zwj_emoji_presentation() {
                0
            } else {
                2
            };
            return (width, WidthInfo::EMOJI_PRESENTATION);
        } else {
            next_info = next_info.unset_emoji_presentation();
        }
    }
    if (matches!(
        next_info,
        WidthInfo::COMBINING_LONG_SOLIDUS_OVERLAY | WidthInfo::SOLIDUS_OVERLAY_ALEF
    ) && matches!(c, '<' | '=' | '>'))
    {
        return (2, WidthInfo::DEFAULT);
    }
    if c <= '\u{A0}' {
        match c {
            '\n' => (1, WidthInfo::LINE_FEED),
            '\r' if next_info == WidthInfo::LINE_FEED => (0, WidthInfo::DEFAULT),
            _ => (1, WidthInfo::DEFAULT),
        }
    } else {
        // Fast path
        if next_info != WidthInfo::DEFAULT {
            if c == '\u{FE0F}' {
                return (0, next_info.set_emoji_presentation());
            }
            if matches!(c, '\u{FE00}' | '\u{FE02}') {
                return (0, next_info.set_vs1_2_3());
            }
            if next_info.is_vs1_2_3() {
                if matches!(c, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}') {
                    return (1, WidthInfo::DEFAULT);
                } else {
                    next_info = next_info.unset_vs1_2_3();
                }
            }
            if next_info.is_ligature_transparent() {
                if c == '\u{200D}' {
                    return (0, next_info.set_zwj_bit());
                } else if is_ligature_transparent(c) {
                    return (0, next_info);
                }
            }

            match (next_info, c) {
                (WidthInfo::COMBINING_LONG_SOLIDUS_OVERLAY, _) if is_solidus_transparent(c) => {
                    return (
                        lookup_width_cjk(c).0 as i8,
                        WidthInfo::COMBINING_LONG_SOLIDUS_OVERLAY,
                    );
                }
                (WidthInfo::JOINING_GROUP_ALEF, '\u{0338}') => {
                    return (0, WidthInfo::SOLIDUS_OVERLAY_ALEF);
                }
                // Arabic Lam-Alef ligature
                (
                    WidthInfo::JOINING_GROUP_ALEF | WidthInfo::SOLIDUS_OVERLAY_ALEF,
                    '\u{644}' | '\u{6B5}'..='\u{6B8}' | '\u{76A}' | '\u{8A6}' | '\u{8C7}',
                ) => return (0, WidthInfo::DEFAULT),
                (WidthInfo::JOINING_GROUP_ALEF, _) if is_transparent_zero_width(c) => {
                    return (0, WidthInfo::JOINING_GROUP_ALEF);
                }

                // Hebrew Alef-ZWJ-Lamed ligature
                (WidthInfo::ZWJ_HEBREW_LETTER_LAMED, '\u{05D0}') => {
                    return (0, WidthInfo::DEFAULT);
                }

                // Khmer coeng signs
                (WidthInfo::KHMER_COENG_ELIGIBLE_LETTER, '\u{17D2}') => {
                    return (-1, WidthInfo::DEFAULT);
                }

                // Buginese <a, -i> ZWJ ya ligature
                (WidthInfo::ZWJ_BUGINESE_LETTER_YA, '\u{1A17}') => {
                    return (0, WidthInfo::BUGINESE_VOWEL_SIGN_I_ZWJ_LETTER_YA)
                }
                (WidthInfo::BUGINESE_VOWEL_SIGN_I_ZWJ_LETTER_YA, '\u{1A15}') => {
                    return (0, WidthInfo::DEFAULT)
                }

                // Tifinagh bi-consonants
                (WidthInfo::TIFINAGH_CONSONANT | WidthInfo::ZWJ_TIFINAGH_CONSONANT, '\u{2D7F}') => {
                    return (1, WidthInfo::TIFINAGH_JOINER_CONSONANT);
                }
                (WidthInfo::ZWJ_TIFINAGH_CONSONANT, '\u{2D31}'..='\u{2D65}' | '\u{2D6F}') => {
                    return (0, WidthInfo::DEFAULT);
                }
                (WidthInfo::TIFINAGH_JOINER_CONSONANT, '\u{2D31}'..='\u{2D65}' | '\u{2D6F}') => {
                    return (-1, WidthInfo::DEFAULT);
                }

                // Lisu tone letter combinations
                (WidthInfo::LISU_TONE_LETTER_MYA_NA_JEU, '\u{A4F8}'..='\u{A4FB}') => {
                    return (0, WidthInfo::DEFAULT);
                }

                // Old Turkic ligature
                (WidthInfo::ZWJ_OLD_TURKIC_LETTER_ORKHON_I, '\u{10C32}') => {
                    return (0, WidthInfo::DEFAULT);
                }
                // Emoji modifier
                (WidthInfo::EMOJI_MODIFIER, _) if is_emoji_modifier_base(c) => {
                    return (0, WidthInfo::EMOJI_PRESENTATION);
                }

                // Regional indicator
                (
                    WidthInfo::REGIONAL_INDICATOR | WidthInfo::SEVERAL_REGIONAL_INDICATOR,
                    '\u{1F1E6}'..='\u{1F1FF}',
                ) => return (1, WidthInfo::SEVERAL_REGIONAL_INDICATOR),

                // ZWJ emoji
                (
                    WidthInfo::EMOJI_PRESENTATION
                    | WidthInfo::SEVERAL_REGIONAL_INDICATOR
                    | WidthInfo::EVEN_REGIONAL_INDICATOR_ZWJ_PRESENTATION
                    | WidthInfo::ODD_REGIONAL_INDICATOR_ZWJ_PRESENTATION
                    | WidthInfo::EMOJI_MODIFIER,
                    '\u{200D}',
                ) => return (0, WidthInfo::ZWJ_EMOJI_PRESENTATION),
                (WidthInfo::ZWJ_EMOJI_PRESENTATION, '\u{20E3}') => {
                    return (0, WidthInfo::KEYCAP_ZWJ_EMOJI_PRESENTATION);
                }
                (WidthInfo::VS16_ZWJ_EMOJI_PRESENTATION, _) if starts_emoji_presentation_seq(c) => {
                    return (0, WidthInfo::EMOJI_PRESENTATION)
                }
                (WidthInfo::VS16_KEYCAP_ZWJ_EMOJI_PRESENTATION, '0'..='9' | '#' | '*') => {
                    return (0, WidthInfo::EMOJI_PRESENTATION)
                }
                (WidthInfo::ZWJ_EMOJI_PRESENTATION, '\u{1F1E6}'..='\u{1F1FF}') => {
                    return (1, WidthInfo::REGIONAL_INDICATOR_ZWJ_PRESENTATION);
                }
                (
                    WidthInfo::REGIONAL_INDICATOR_ZWJ_PRESENTATION
                    | WidthInfo::ODD_REGIONAL_INDICATOR_ZWJ_PRESENTATION,
                    '\u{1F1E6}'..='\u{1F1FF}',
                ) => return (-1, WidthInfo::EVEN_REGIONAL_INDICATOR_ZWJ_PRESENTATION),
                (
                    WidthInfo::EVEN_REGIONAL_INDICATOR_ZWJ_PRESENTATION,
                    '\u{1F1E6}'..='\u{1F1FF}',
                ) => return (3, WidthInfo::ODD_REGIONAL_INDICATOR_ZWJ_PRESENTATION),
                (WidthInfo::ZWJ_EMOJI_PRESENTATION, '\u{1F3FB}'..='\u{1F3FF}') => {
                    return (0, WidthInfo::EMOJI_MODIFIER);
                }
                (WidthInfo::ZWJ_EMOJI_PRESENTATION, '\u{E007F}') => {
                    return (0, WidthInfo::TAG_END_ZWJ_EMOJI_PRESENTATION);
                }
                (WidthInfo::TAG_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A1_END_ZWJ_EMOJI_PRESENTATION);
                }
                (WidthInfo::TAG_A1_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A2_END_ZWJ_EMOJI_PRESENTATION)
                }
                (WidthInfo::TAG_A2_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A3_END_ZWJ_EMOJI_PRESENTATION)
                }
                (WidthInfo::TAG_A3_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A4_END_ZWJ_EMOJI_PRESENTATION)
                }
                (WidthInfo::TAG_A4_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A5_END_ZWJ_EMOJI_PRESENTATION)
                }
                (WidthInfo::TAG_A5_END_ZWJ_EMOJI_PRESENTATION, '\u{E0061}'..='\u{E007A}') => {
                    return (0, WidthInfo::TAG_A6_END_ZWJ_EMOJI_PRESENTATION)
                }
                (
                    WidthInfo::TAG_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A1_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A2_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A3_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A4_END_ZWJ_EMOJI_PRESENTATION,
                    '\u{E0030}'..='\u{E0039}',
                ) => return (0, WidthInfo::TAG_D1_END_ZWJ_EMOJI_PRESENTATION),
                (WidthInfo::TAG_D1_END_ZWJ_EMOJI_PRESENTATION, '\u{E0030}'..='\u{E0039}') => {
                    return (0, WidthInfo::TAG_D2_END_ZWJ_EMOJI_PRESENTATION);
                }
                (WidthInfo::TAG_D2_END_ZWJ_EMOJI_PRESENTATION, '\u{E0030}'..='\u{E0039}') => {
                    return (0, WidthInfo::TAG_D3_END_ZWJ_EMOJI_PRESENTATION);
                }
                (
                    WidthInfo::TAG_A3_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A4_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A5_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_A6_END_ZWJ_EMOJI_PRESENTATION
                    | WidthInfo::TAG_D3_END_ZWJ_EMOJI_PRESENTATION,
                    '\u{1F3F4}',
                ) => return (0, WidthInfo::EMOJI_PRESENTATION),
                (WidthInfo::ZWJ_EMOJI_PRESENTATION, _)
                    if lookup_width_cjk(c).1 == WidthInfo::EMOJI_PRESENTATION =>
                {
                    return (0, WidthInfo::EMOJI_PRESENTATION)
                }

                (WidthInfo::KIRAT_RAI_VOWEL_SIGN_E, '\u{16D63}') => {
                    return (0, WidthInfo::DEFAULT);
                }
                (WidthInfo::KIRAT_RAI_VOWEL_SIGN_E, '\u{16D67}') => {
                    return (0, WidthInfo::KIRAT_RAI_VOWEL_SIGN_AI);
                }
                (WidthInfo::KIRAT_RAI_VOWEL_SIGN_E, '\u{16D68}') => {
                    return (1, WidthInfo::KIRAT_RAI_VOWEL_SIGN_E);
                }
                (WidthInfo::KIRAT_RAI_VOWEL_SIGN_E, '\u{16D69}') => {
                    return (0, WidthInfo::DEFAULT);
                }
                (WidthInfo::KIRAT_RAI_VOWEL_SIGN_AI, '\u{16D63}') => {
                    return (0, WidthInfo::DEFAULT);
                }

                // Fallback
                _ => {}
            }
        }

        let ret = lookup_width_cjk(c);
        (ret.0 as i8, ret.1)
    }
}

#[cfg(feature = "cjk")]
#[inline]
pub fn str_width_cjk(s: &str) -> usize {
    s.chars()
        .rfold(
            (0, WidthInfo::DEFAULT),
            |(sum, next_info), c| -> (usize, WidthInfo) {
                let (add, info) = width_in_str_cjk(c, next_info);
                (sum.wrapping_add_signed(isize::from(add)), info)
            },
        )
        .0
}

/// Whether this character is a zero-width character with
/// `Joining_Type=Transparent`. Used by the Alef-Lamed ligatures.
/// See also [`is_ligature_transparent`], a near-subset of this (only ZWJ is excepted)
/// which is transparent for non-Arabic ligatures.
fn is_transparent_zero_width(c: char) -> bool {
    if lookup_width(c).0 != 0 {
        // Not zero-width
        false
    } else {
        let cp: u32 = c.into();
        NON_TRANSPARENT_ZERO_WIDTHS
            .binary_search_by(|&(lo, hi)| {
                let lo = u32::from_le_bytes([lo[0], lo[1], lo[2], 0]);
                let hi = u32::from_le_bytes([hi[0], hi[1], hi[2], 0]);
                if cp < lo {
                    Ordering::Greater
                } else if cp > hi {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            })
            .is_err()
    }
}

/// Whether this character is a default-ignorable combining mark
/// or ZWJ. These characters won't interrupt non-Arabic ligatures.
fn is_ligature_transparent(c: char) -> bool {
    matches!(c, '\u{34F}' | '\u{17B4}'..='\u{17B5}' | '\u{180B}'..='\u{180D}' | '\u{180F}' | '\u{200D}' | '\u{FE00}'..='\u{FE0F}' | '\u{E0100}'..='\u{E01EF}')
}

/// Whether this character is transparent wrt the effect of
/// U+0338 COMBINING LONG SOLIDUS OVERLAY
/// on its base character.
#[cfg(feature = "cjk")]
fn is_solidus_transparent(c: char) -> bool {
    let cp: u32 = c.into();
    is_ligature_transparent(c)
        || SOLIDUS_TRANSPARENT
            .binary_search_by(|&(lo, hi)| {
                let lo = u32::from_le_bytes([lo[0], lo[1], lo[2], 0]);
                let hi = u32::from_le_bytes([hi[0], hi[1], hi[2], 0]);
                if cp < lo {
                    Ordering::Greater
                } else if cp > hi {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            })
            .is_ok()
}

/// Whether this character forms an [emoji presentation sequence]
/// (https://www.unicode.org/reports/tr51/#def_emoji_presentation_sequence)
/// when followed by `'\u{FEOF}'`.
/// Emoji presentation sequences are considered to have width 2.
#[inline]
pub fn starts_emoji_presentation_seq(c: char) -> bool {
    let cp: u32 = c.into();
    // First level of lookup uses all but 10 LSB
    let top_bits = cp >> 10;
    let idx_of_leaf: usize = match top_bits {
        0x0 => 0,
        0x8 => 1,
        0x9 => 2,
        0xA => 3,
        0xC => 4,
        0x7C => 5,
        0x7D => 6,
        _ => return false,
    };
    // Extract the 3-9th (0-indexed) least significant bits of `cp`,
    // and use them to index into `leaf_row`.
    let idx_within_leaf = usize::try_from((cp >> 3) & 0x7F).unwrap();
    let leaf_byte = EMOJI_PRESENTATION_LEAVES.0[idx_of_leaf][idx_within_leaf];
    // Use the 3 LSB of `cp` to index into `leaf_byte`.
    ((leaf_byte >> (cp & 7)) & 1) == 1
}

/// Returns `true` if `c` has default emoji presentation, but forms a [text presentation sequence]
/// (https://www.unicode.org/reports/tr51/#def_text_presentation_sequence)
/// when followed by `'\u{FEOE}'`, and is not ideographic.
/// Such sequences are considered to have width 1.
#[inline]
pub fn starts_non_ideographic_text_presentation_seq(c: char) -> bool {
    let cp: u32 = c.into();
    // First level of lookup uses all but 8 LSB
    let top_bits = cp >> 8;
    let leaf: &[(u8, u8)] = match top_bits {
        0x23 => &TEXT_PRESENTATION_LEAF_0,
        0x25 => &TEXT_PRESENTATION_LEAF_1,
        0x26 => &TEXT_PRESENTATION_LEAF_2,
        0x27 => &TEXT_PRESENTATION_LEAF_3,
        0x2B => &TEXT_PRESENTATION_LEAF_4,
        0x1F0 => &TEXT_PRESENTATION_LEAF_5,
        0x1F3 => &TEXT_PRESENTATION_LEAF_6,
        0x1F4 => &TEXT_PRESENTATION_LEAF_7,
        0x1F5 => &TEXT_PRESENTATION_LEAF_8,
        0x1F6 => &TEXT_PRESENTATION_LEAF_9,
        _ => return false,
    };

    let bottom_bits = (cp & 0xFF) as u8;
    leaf.binary_search_by(|&(lo, hi)| {
        if bottom_bits < lo {
            Ordering::Greater
        } else if bottom_bits > hi {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    })
    .is_ok()
}

/// Returns `true` if `c` is an `Emoji_Modifier_Base`.
#[inline]
pub fn is_emoji_modifier_base(c: char) -> bool {
    let cp: u32 = c.into();
    // First level of lookup uses all but 8 LSB
    let top_bits = cp >> 8;
    let leaf: &[(u8, u8)] = match top_bits {
        0x26 => &EMOJI_MODIFIER_LEAF_0,
        0x27 => &EMOJI_MODIFIER_LEAF_1,
        0x1F3 => &EMOJI_MODIFIER_LEAF_2,
        0x1F4 => &EMOJI_MODIFIER_LEAF_3,
        0x1F5 => &EMOJI_MODIFIER_LEAF_4,
        0x1F6 => &EMOJI_MODIFIER_LEAF_5,
        0x1F9 => &EMOJI_MODIFIER_LEAF_6,
        0x1FA => &EMOJI_MODIFIER_LEAF_7,
        _ => return false,
    };

    let bottom_bits = (cp & 0xFF) as u8;
    leaf.binary_search_by(|&(lo, hi)| {
        if bottom_bits < lo {
            Ordering::Greater
        } else if bottom_bits > hi {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    })
    .is_ok()
}
// copyv: end
