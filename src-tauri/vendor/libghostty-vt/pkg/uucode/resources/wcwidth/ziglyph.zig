//! copyv: https://codeberg.org/dude_the_builder/ziglyph/src/commit/29760d237219cc4d486f5cd654262d7b0d62d511/src/display_width.zig#L15-L124 begin
fn isAsciiStr(str: []const u8) bool {
    return for (str) |b| {
        if (b > 127) break false;
    } else true;
}

/// AmbiguousWidth determines the width of ambiguous characters according to the context. In an
/// East Asian context, the width of ambiguous code points should be 2 (full), and 1 (half)
/// in non-East Asian contexts. The most common use case is `half`.
pub const AmbiguousWidth = enum(u2) {
    half = 1,
    full = 2,
};

/// codePointWidth returns how many cells (or columns) wide `cp` should be when rendered in a
/// fixed-width font.
pub fn codePointWidth(cp: u21, am_width: AmbiguousWidth) i3 {
    if (cp == 0x000 or cp == 0x0005 or cp == 0x0007 or (cp >= 0x000A and cp <= 0x000F)) {
        // Control.
        return 0;
    } else if (cp == 0x0008 or cp == 0x007F) {
        // backspace and DEL
        return -1;
    } else if (cp == 0x00AD) {
        // soft-hyphen
        return 1;
    } else if (cp == 0x2E3A) {
        // two-em dash
        return 2;
    } else if (cp == 0x2E3B) {
        // three-em dash
        return 3;
    } else if (cats.isEnclosingMark(cp) or cats.isNonspacingMark(cp)) {
        // Combining Marks.
        return 0;
    } else if (cats.isFormat(cp) and (!(cp >= 0x0600 and cp <= 0x0605) and cp != 0x061C and
        cp != 0x06DD and cp != 0x08E2))
    {
        // Format except Arabic.
        return 0;
    } else if ((cp >= 0x1160 and cp <= 0x11FF) or (cp >= 0x2060 and cp <= 0x206F) or
        (cp >= 0xFFF0 and cp <= 0xFFF8) or (cp >= 0xE0000 and cp <= 0xE0FFF))
    {
        // Hangul syllable and ignorable.
        return 0;
    } else if ((cp >= 0x3400 and cp <= 0x4DBF) or (cp >= 0x4E00 and cp <= 0x9FFF) or
        (cp >= 0xF900 and cp <= 0xFAFF) or (cp >= 0x20000 and cp <= 0x2FFFD) or
        (cp >= 0x30000 and cp <= 0x3FFFD))
    {
        return 2;
    } else if (eaw.isWide(cp) or eaw.isFullwidth(cp)) {
        return 2;
    } else if (gbp.isRegionalIndicator(cp)) {
        return 2;
    } else if (eaw.isAmbiguous(cp)) {
        return @intFromEnum(am_width);
    } else {
        return 1;
    }
}

/// strWidth returns how many cells (or columns) wide `str` should be when rendered in a
/// fixed-width font.
pub fn strWidth(str: []const u8, am_width: AmbiguousWidth) !usize {
    var total: isize = 0;

    // ASCII bytes are all width == 1.
    if (isAsciiStr(str)) {
        for (str) |b| {
            // Backspace and DEL
            if (b == 8 or b == 127) {
                total -= 1;
                continue;
            }

            // Control
            if (b < 32) continue;

            // All other ASCII.
            total += 1;
        }

        return if (total > 0) @intCast(total) else 0;
    }

    var giter = GraphemeIterator.init(str);

    while (giter.next()) |gc| {
        var cp_iter = (try unicode.Utf8View.init(str[gc.offset .. gc.offset + gc.len])).iterator();

        while (cp_iter.nextCodepoint()) |cp| {
            var w = codePointWidth(cp, am_width);

            if (w != 0) {
                // Only adding width of first non-zero-width code point.
                if (emoji.isExtendedPictographic(cp)) {
                    if (cp_iter.nextCodepoint()) |ncp| {
                        // emoji text sequence.
                        if (ncp == 0xFE0E) w = 1;
                        if (ncp == 0xFE0F) w = 2;
                    }
                }
                total += w;
                break;
            }
        }
    }

    return if (total > 0) @intCast(total) else 0;
}
// copyv: end
