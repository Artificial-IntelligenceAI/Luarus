//! Grapheme cluster segmentation: what Luarus means by "one character".
//!
//! A character is what a reader would point at, not a Unicode scalar value.
//! `c` is one character and so is `🧑‍🧑‍🧒‍🧒`, even though the latter is seven
//! scalars welded together with zero-width joiners. Column numbers and caret
//! widths in diagnostics are counted this way, so a caret lands where the eye
//! expects it.
//!
//! This implements the UAX #29 extended grapheme cluster boundary rules
//! (GB1–GB13, GB999). The rules are complete; the character property tables
//! cover the ranges that occur in practice — emoji and their modifiers,
//! combining marks for Latin, Greek, Cyrillic, Hebrew, Arabic, Thai, Lao and
//! the Indic scripts, regional indicators, Hangul, and the variation selectors.
//! A combining mark outside those ranges degrades to standing on its own rather
//! than joining, which misplaces a caret but never loses text.

/// Grapheme cluster break property, plus the extra flag GB11 needs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gcb {
    Other,
    Cr,
    Lf,
    Control,
    Extend,
    Zwj,
    RegionalIndicator,
    Prepend,
    SpacingMark,
    L,
    V,
    T,
    Lv,
    Lvt,
}

fn in_ranges(c: u32, ranges: &[(u32, u32)]) -> bool {
    ranges.binary_search_by(|(lo, hi)| {
        if c < *lo {
            std::cmp::Ordering::Greater
        } else if c > *hi {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Equal
        }
    })
    .is_ok()
}

/// Combining marks, variation selectors, skin tones, tag characters.
const EXTEND: &[(u32, u32)] = &[
    (0x0300, 0x036F),
    (0x0483, 0x0489),
    (0x0591, 0x05BD),
    (0x05BF, 0x05BF),
    (0x05C1, 0x05C2),
    (0x05C4, 0x05C5),
    (0x05C7, 0x05C7),
    (0x0610, 0x061A),
    (0x064B, 0x065F),
    (0x0670, 0x0670),
    (0x06D6, 0x06DC),
    (0x06DF, 0x06E4),
    (0x06E7, 0x06E8),
    (0x06EA, 0x06ED),
    (0x0711, 0x0711),
    (0x0730, 0x074A),
    (0x07A6, 0x07B0),
    (0x07EB, 0x07F3),
    (0x0816, 0x0819),
    (0x081B, 0x0823),
    (0x0825, 0x0827),
    (0x0829, 0x082D),
    (0x0859, 0x085B),
    (0x08D3, 0x0902),
    (0x093A, 0x093A),
    (0x093C, 0x093C),
    (0x0941, 0x0948),
    (0x094D, 0x094D),
    (0x0951, 0x0957),
    (0x0962, 0x0963),
    (0x0981, 0x0981),
    (0x09BC, 0x09BC),
    (0x09C1, 0x09C4),
    (0x09CD, 0x09CD),
    (0x09E2, 0x09E3),
    (0x0A01, 0x0A02),
    (0x0A3C, 0x0A3C),
    (0x0A41, 0x0A42),
    (0x0A47, 0x0A48),
    (0x0A4B, 0x0A4D),
    (0x0A81, 0x0A82),
    (0x0ABC, 0x0ABC),
    (0x0AC1, 0x0AC5),
    (0x0AC7, 0x0AC8),
    (0x0ACD, 0x0ACD),
    (0x0B01, 0x0B01),
    (0x0B3C, 0x0B3C),
    (0x0B3F, 0x0B3F),
    (0x0B41, 0x0B44),
    (0x0B4D, 0x0B4D),
    (0x0B82, 0x0B82),
    (0x0BC0, 0x0BC0),
    (0x0BCD, 0x0BCD),
    (0x0C00, 0x0C00),
    (0x0C3E, 0x0C40),
    (0x0C46, 0x0C48),
    (0x0C4A, 0x0C4D),
    (0x0C81, 0x0C81),
    (0x0CBC, 0x0CBC),
    (0x0CBF, 0x0CBF),
    (0x0CC6, 0x0CC6),
    (0x0CCC, 0x0CCD),
    (0x0D01, 0x0D01),
    (0x0D41, 0x0D44),
    (0x0D4D, 0x0D4D),
    (0x0DCA, 0x0DCA),
    (0x0DD2, 0x0DD4),
    (0x0DD6, 0x0DD6),
    // Thai and Lao: above- and below-line vowels and tone marks.
    (0x0E31, 0x0E31),
    (0x0E34, 0x0E3A),
    (0x0E47, 0x0E4E),
    (0x0EB1, 0x0EB1),
    (0x0EB4, 0x0EBC),
    (0x0EC8, 0x0ECD),
    (0x0F18, 0x0F19),
    (0x0F35, 0x0F35),
    (0x0F37, 0x0F37),
    (0x0F39, 0x0F39),
    (0x0F71, 0x0F7E),
    (0x0F80, 0x0F84),
    (0x0F86, 0x0F87),
    (0x102D, 0x1030),
    (0x1032, 0x1037),
    (0x1039, 0x103A),
    (0x1058, 0x1059),
    (0x135D, 0x135F),
    (0x1712, 0x1714),
    (0x1732, 0x1734),
    (0x17B4, 0x17B5),
    (0x17B7, 0x17BD),
    (0x17C6, 0x17C6),
    (0x17C9, 0x17D3),
    (0x180B, 0x180D),
    (0x18A9, 0x18A9),
    (0x1A17, 0x1A18),
    (0x1AB0, 0x1AFF),
    (0x1B00, 0x1B03),
    (0x1B34, 0x1B34),
    (0x1B36, 0x1B3A),
    (0x1B6B, 0x1B73),
    (0x1BE6, 0x1BE6),
    (0x1C2C, 0x1C33),
    (0x1CD0, 0x1CD2),
    (0x1CD4, 0x1CE0),
    (0x1DC0, 0x1DFF),
    (0x200C, 0x200C),
    (0x20D0, 0x20F0),
    (0x2CEF, 0x2CF1),
    (0x2D7F, 0x2D7F),
    (0x2DE0, 0x2DFF),
    (0x302A, 0x302F),
    (0x3099, 0x309A),
    (0xA66F, 0xA672),
    (0xA674, 0xA67D),
    (0xA69E, 0xA69F),
    (0xA806, 0xA806),
    (0xA8E0, 0xA8F1),
    (0xA9B3, 0xA9B3),
    (0xAAB0, 0xAAB0),
    (0xAAB2, 0xAAB4),
    (0xABED, 0xABED),
    (0xFB1E, 0xFB1E),
    (0xFE00, 0xFE0F),
    (0xFE20, 0xFE2F),
    (0x101FD, 0x101FD),
    (0x10A01, 0x10A03),
    (0x11046, 0x11046),
    (0x110B9, 0x110BA),
    (0x11133, 0x11134),
    (0x111C0, 0x111C0),
    (0x11235, 0x11236),
    (0x112E9, 0x112EA),
    (0x1133C, 0x1133C),
    (0x114C2, 0x114C3),
    (0x116B6, 0x116B7),
    (0x1172B, 0x1172B),
    (0x11A34, 0x11A34),
    (0x16AF0, 0x16AF4),
    (0x16B30, 0x16B36),
    (0x1BC9D, 0x1BC9E),
    (0x1D165, 0x1D169),
    (0x1D16D, 0x1D172),
    (0x1D17B, 0x1D182),
    (0x1D185, 0x1D18B),
    (0x1D1AA, 0x1D1AD),
    (0x1D242, 0x1D244),
    (0x1E8D0, 0x1E8D6),
    // Emoji skin-tone modifiers.
    (0x1F3FB, 0x1F3FF),
    // Tag characters, used by flag sequences.
    (0xE0020, 0xE007F),
    (0xE0100, 0xE01EF),
];

/// Spacing combining marks, which attach to the preceding cluster (GB9a).
const SPACING_MARK: &[(u32, u32)] = &[
    (0x0903, 0x0903),
    (0x093B, 0x093B),
    (0x093E, 0x0940),
    (0x0949, 0x094C),
    (0x094E, 0x094F),
    (0x0982, 0x0983),
    (0x09BE, 0x09C0),
    (0x09C7, 0x09C8),
    (0x09CB, 0x09CC),
    (0x0A03, 0x0A03),
    (0x0A3E, 0x0A40),
    (0x0A83, 0x0A83),
    (0x0ABE, 0x0AC0),
    (0x0AC9, 0x0AC9),
    (0x0ACB, 0x0ACC),
    (0x0B02, 0x0B03),
    (0x0B3E, 0x0B3E),
    (0x0B40, 0x0B40),
    (0x0B47, 0x0B48),
    (0x0B4B, 0x0B4C),
    (0x0BBE, 0x0BBF),
    (0x0BC1, 0x0BC2),
    (0x0BC6, 0x0BC8),
    (0x0BCA, 0x0BCC),
    (0x0C01, 0x0C03),
    (0x0C41, 0x0C44),
    (0x0C82, 0x0C83),
    (0x0CBE, 0x0CBE),
    (0x0CC0, 0x0CC4),
    (0x0CC7, 0x0CC8),
    (0x0CCA, 0x0CCB),
    (0x0D02, 0x0D03),
    (0x0D3E, 0x0D40),
    (0x0D46, 0x0D48),
    (0x0D4A, 0x0D4C),
    (0x0D82, 0x0D83),
    (0x0DCF, 0x0DD1),
    (0x0DD8, 0x0DDF),
    (0x0DF2, 0x0DF3),
    // Thai SARA AM and its Lao counterpart.
    (0x0E33, 0x0E33),
    (0x0EB3, 0x0EB3),
    (0x0F3E, 0x0F3F),
    (0x0F7F, 0x0F7F),
    (0x1031, 0x1031),
    (0x103B, 0x103C),
    (0x1056, 0x1057),
    (0x17B6, 0x17B6),
    (0x17BE, 0x17C5),
    (0x17C7, 0x17C8),
    (0x1923, 0x1926),
    (0x1929, 0x192B),
    (0x1930, 0x1931),
    (0x1933, 0x1938),
    (0x1A19, 0x1A1A),
    (0x1B04, 0x1B04),
    (0x1B35, 0x1B35),
    (0x1B3B, 0x1B3B),
    (0x1B3D, 0x1B41),
    (0x1B43, 0x1B44),
    (0x1B82, 0x1B82),
    (0xA823, 0xA824),
    (0xA827, 0xA827),
    (0xA880, 0xA881),
    (0xA8B4, 0xA8C3),
    (0xA952, 0xA953),
    (0xA983, 0xA983),
    (0xAA2F, 0xAA30),
    (0xAA33, 0xAA34),
    (0xAA4D, 0xAA4D),
    (0xAAEB, 0xAAEB),
    (0xAAEE, 0xAAEF),
    (0xAAF5, 0xAAF5),
    (0xABE3, 0xABE4),
    (0xABE6, 0xABE7),
    (0xABE9, 0xABEA),
    (0xABEC, 0xABEC),
    (0x11000, 0x11000),
    (0x11002, 0x11002),
    (0x11082, 0x11082),
    (0x110B0, 0x110B2),
    (0x110B7, 0x110B8),
    (0x1112C, 0x1112C),
    (0x11182, 0x11182),
    (0x111B3, 0x111B5),
    (0x111BF, 0x111C0),
    (0x1122C, 0x1122E),
    (0x11232, 0x11233),
    (0x11235, 0x11235),
    (0x112E0, 0x112E2),
    (0x11302, 0x11303),
    (0x1133E, 0x1133F),
    (0x11341, 0x11344),
    (0x11347, 0x11348),
    (0x1134B, 0x1134D),
    (0x114B0, 0x114B2),
    (0x114B9, 0x114B9),
    (0x114BB, 0x114BE),
    (0x115AF, 0x115B1),
    (0x115B8, 0x115BB),
    (0x11630, 0x11632),
    (0x1163B, 0x1163C),
    (0x116AC, 0x116AC),
    (0x116AE, 0x116AF),
    (0x11720, 0x11721),
    (0x11726, 0x11726),
    (0x16F51, 0x16F7E),
    (0x1D165, 0x1D166),
    (0x1D16D, 0x1D172),
];

/// Format characters that begin a cluster rather than joining one (GB9b).
const PREPEND: &[(u32, u32)] = &[
    (0x0600, 0x0605),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x0D4E, 0x0D4E),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x111C2, 0x111C3),
    (0x1193F, 0x1193F),
    (0x11941, 0x11941),
    (0x11A3A, 0x11A3A),
    (0x11A84, 0x11A89),
    (0x11D46, 0x11D46),
];

/// Extended_Pictographic, needed by GB11 so emoji ZWJ sequences hold together.
const EXT_PICT: &[(u32, u32)] = &[
    (0x00A9, 0x00A9),
    (0x00AE, 0x00AE),
    (0x203C, 0x203C),
    (0x2049, 0x2049),
    (0x2122, 0x2122),
    (0x2139, 0x2139),
    (0x2194, 0x21AA),
    (0x231A, 0x231B),
    (0x2328, 0x2328),
    (0x2388, 0x2388),
    (0x23CF, 0x23CF),
    (0x23E9, 0x23F3),
    (0x23F8, 0x23FA),
    (0x24C2, 0x24C2),
    (0x25AA, 0x25FE),
    (0x2600, 0x27BF),
    (0x2934, 0x2935),
    (0x2B00, 0x2BFF),
    (0x3030, 0x3030),
    (0x303D, 0x303D),
    (0x3297, 0x3297),
    (0x3299, 0x3299),
    (0x1F000, 0x1FAFF),
    (0x1FC00, 0x1FFFD),
];

fn class(c: char) -> Gcb {
    let u = c as u32;
    match u {
        0x000D => return Gcb::Cr,
        0x000A => return Gcb::Lf,
        0x200D => return Gcb::Zwj,
        0x1F1E6..=0x1F1FF => return Gcb::RegionalIndicator,
        // Hangul syllable blocks: LV when the trailing jamo slot is empty.
        0xAC00..=0xD7A3 => {
            return if (u - 0xAC00) % 28 == 0 { Gcb::Lv } else { Gcb::Lvt }
        }
        0x1100..=0x115F | 0xA960..=0xA97C => return Gcb::L,
        0x1160..=0x11A7 | 0xD7B0..=0xD7C6 => return Gcb::V,
        0x11A8..=0x11FF | 0xD7CB..=0xD7FB => return Gcb::T,
        _ => {}
    }
    if in_ranges(u, EXTEND) {
        return Gcb::Extend;
    }
    if in_ranges(u, SPACING_MARK) {
        return Gcb::SpacingMark;
    }
    if in_ranges(u, PREPEND) {
        return Gcb::Prepend;
    }
    // Control: Cc, Cf, Zl, Zp, minus the joiners handled above.
    if u < 0x20 || (0x7F..=0x9F).contains(&u) {
        return Gcb::Control;
    }
    if matches!(u, 0x00AD | 0x200B | 0x200E | 0x200F | 0x2028 | 0x2029 | 0xFEFF)
        || (0x2060..=0x2064).contains(&u)
        || (0xFFF9..=0xFFFB).contains(&u)
    {
        return Gcb::Control;
    }
    Gcb::Other
}

fn is_ext_pict(c: char) -> bool {
    in_ranges(c as u32, EXT_PICT)
}

/// Iterator over the grapheme clusters of a string.
pub struct Graphemes<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Iterator for Graphemes<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.pos >= self.src.len() {
            return None;
        }
        let start = self.pos;
        let mut chars = self.src[start..].char_indices();
        let (_, first) = chars.next()?;

        let mut prev_class = class(first);
        let mut offset = first.len_utf8();

        // GB11 needs to know whether the run before a ZWJ was a pictograph
        // followed only by Extend characters.
        let mut pict_run = is_ext_pict(first);
        // GB12/GB13 join regional indicators in pairs, not in runs.
        let mut ri_run = usize::from(prev_class == Gcb::RegionalIndicator);

        for (i, next) in chars {
            let next_class = class(next);
            if breaks(prev_class, next_class, pict_run, ri_run, next) {
                offset = i;
                break;
            }

            match next_class {
                Gcb::Extend => {} // a pictograph run survives Extend characters
                Gcb::Zwj => {}    // and survives the joiner itself
                Gcb::RegionalIndicator => ri_run += 1,
                _ => {
                    pict_run = is_ext_pict(next);
                    ri_run = 0;
                }
            }
            if next_class == Gcb::Other && is_ext_pict(next) {
                pict_run = true;
            }

            prev_class = next_class;
            offset = i + next.len_utf8();
        }

        self.pos = start + offset;
        Some(&self.src[start..self.pos])
    }
}

/// The UAX #29 rules, in order. `true` means a boundary falls here.
fn breaks(prev: Gcb, next: Gcb, pict_run: bool, ri_run: usize, next_char: char) -> bool {
    use Gcb::*;
    // GB3: CR × LF
    if prev == Cr && next == Lf {
        return false;
    }
    // GB4: (Control | CR | LF) ÷
    if matches!(prev, Control | Cr | Lf) {
        return true;
    }
    // GB5: ÷ (Control | CR | LF)
    if matches!(next, Control | Cr | Lf) {
        return true;
    }
    // GB6, GB7, GB8: Hangul syllable sequences
    if prev == L && matches!(next, L | V | Lv | Lvt) {
        return false;
    }
    if matches!(prev, Lv | V) && matches!(next, V | T) {
        return false;
    }
    if matches!(prev, Lvt | T) && next == T {
        return false;
    }
    // GB9: × (Extend | ZWJ)
    if matches!(next, Extend | Zwj) {
        return false;
    }
    // GB9a: × SpacingMark
    if next == SpacingMark {
        return false;
    }
    // GB9b: Prepend ×
    if prev == Prepend {
        return false;
    }
    // GB11: ExtPict Extend* ZWJ × ExtPict
    if prev == Zwj && pict_run && is_ext_pict(next_char) {
        return false;
    }
    // GB12, GB13: pair up regional indicators
    if prev == RegionalIndicator && next == RegionalIndicator && ri_run % 2 == 1 {
        return false;
    }
    // GB999: otherwise, break
    true
}

pub fn graphemes(src: &str) -> Graphemes<'_> {
    Graphemes { src, pos: 0 }
}

/// How many characters `src` contains, as a reader would count them.
pub fn count(src: &str) -> usize {
    graphemes(src).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_ascii() {
        assert_eq!(count("c"), 1);
        assert_eq!(count("hello"), 5);
    }

    #[test]
    fn a_zwj_family_is_one_character() {
        // U+1F9D1 ZWJ U+1F9D1 ZWJ U+1F9D2 ZWJ U+1F9D2 — seven scalars, one glyph.
        let family = "🧑‍🧑‍🧒‍🧒";
        assert_eq!(family.chars().count(), 7);
        assert_eq!(count(family), 1);
    }

    #[test]
    fn skin_tones_and_variation_selectors_attach() {
        assert_eq!(count("👍🏽"), 1);
        assert_eq!(count("❤️"), 1);
    }

    #[test]
    fn flags_pair_their_regional_indicators() {
        assert_eq!(count("🇹🇭"), 1);
        assert_eq!(count("🇹🇭🇯🇵"), 2);
    }

    #[test]
    fn combining_marks_join_their_base() {
        assert_eq!(count("é"), 1); // e + U+0301
        assert_eq!(count("a\u{0300}\u{0301}"), 1);
    }

    #[test]
    fn thai_vowels_and_tone_marks_join() {
        // ก + ั + ว = one cluster, then ห, then ว, then ั, ด
        assert_eq!(count("\u{0E01}\u{0E31}"), 1);
        assert_eq!(count("ไทย"), 3);
    }

    #[test]
    fn crlf_is_one_cluster() {
        assert_eq!(count("\r\n"), 1);
        assert_eq!(count("a\r\nb"), 3);
    }

    #[test]
    fn hangul_syllables_hold_together() {
        assert_eq!(count("\u{1100}\u{1161}\u{11A8}"), 1);
    }

    #[test]
    fn slices_cover_the_whole_input() {
        for s in ["", "c", "🧑‍🧑‍🧒‍🧒", "a\u{0300}b🇹🇭", "ไทย\r\n"] {
            assert_eq!(graphemes(s).collect::<String>(), s);
        }
    }

    #[test]
    fn mixed_text_counts_as_read() {
        assert_eq!(count("c🧑‍🧑‍🧒‍🧒c"), 3);
    }
}
