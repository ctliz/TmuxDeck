// copyv: https://codeberg.org/atman/zg/src/commit/d9f596626e8ec05a9f3e47f7bc83aedd5bd2f989/codegen/dwp.zig#L31-L226 begin
var flat_map = std.AutoHashMap(u21, i4).init(allocator);
defer flat_map.deinit();

// Process DerivedEastAsianWidth.txt
var deaw_reader = std.io.Reader.fixed(@embedFile("DerivedEastAsianWidth.txt"));

while (deaw_reader.takeDelimiterInclusive('\n')) |took| {
    const line = std.mem.trimRight(u8, took, "\n");
    if (line.len == 0) continue;

    // @missing ranges
    if (std.mem.startsWith(u8, line, "# @missing: ")) {
        const semi = std.mem.indexOfScalar(u8, line, ';').?;
        const field = line[12..semi];
        const dots = std.mem.indexOf(u8, field, "..").?;
        const from = try std.fmt.parseInt(u21, field[0..dots], 16);
        const to = try std.fmt.parseInt(u21, field[dots + 2 ..], 16);
        if (from == 0 and to == 0x10ffff) continue;
        for (from..to + 1) |cp| try flat_map.put(@intCast(cp), 2);
        continue;
    }

    if (line[0] == '#') continue;

    const no_comment = if (std.mem.indexOfScalar(u8, line, '#')) |octo| line[0..octo] else line;

    var field_iter = std.mem.tokenizeAny(u8, no_comment, "; ");
    var current_code: [2]u21 = undefined;

    var i: usize = 0;
    while (field_iter.next()) |field| : (i += 1) {
        switch (i) {
            0 => {
                // Code point(s)
                if (std.mem.indexOf(u8, field, "..")) |dots| {
                    current_code = .{
                        try std.fmt.parseInt(u21, field[0..dots], 16),
                        try std.fmt.parseInt(u21, field[dots + 2 ..], 16),
                    };
                } else {
                    const code = try std.fmt.parseInt(u21, field, 16);
                    current_code = .{ code, code };
                }
            },
            1 => {
                // Width
                if (std.mem.eql(u8, field, "W") or
                    std.mem.eql(u8, field, "F") or
                    (options.cjk and std.mem.eql(u8, field, "A")))
                {
                    for (current_code[0]..current_code[1] + 1) |cp| try flat_map.put(@intCast(cp), 2);
                }
            },
            else => {},
        }
    }
} else |err| switch (err) {
    error.EndOfStream => {},
    else => {
        return err;
    },
}
// Process DerivedGeneralCategory.txt
var dgc_reader = std.io.Reader.fixed(@embedFile("DerivedGeneralCategory.txt"));

while (dgc_reader.takeDelimiterInclusive('\n')) |took| {
    const line = std.mem.trimRight(u8, took, "\n");
    if (line.len == 0 or line[0] == '#') continue;
    const no_comment = if (std.mem.indexOfScalar(u8, line, '#')) |octo| line[0..octo] else line;

    var field_iter = std.mem.tokenizeAny(u8, no_comment, "; ");
    var current_code: [2]u21 = undefined;

    var i: usize = 0;
    while (field_iter.next()) |field| : (i += 1) {
        switch (i) {
            0 => {
                // Code point(s)
                if (std.mem.indexOf(u8, field, "..")) |dots| {
                    current_code = .{
                        try std.fmt.parseInt(u21, field[0..dots], 16),
                        try std.fmt.parseInt(u21, field[dots + 2 ..], 16),
                    };
                } else {
                    const code = try std.fmt.parseInt(u21, field, 16);
                    current_code = .{ code, code };
                }
            },
            1 => {
                // General category
                if (std.mem.eql(u8, field, "Mn")) {
                    // Nonspacing_Mark
                    for (current_code[0]..current_code[1] + 1) |cp| try flat_map.put(@intCast(cp), 0);
                } else if (std.mem.eql(u8, field, "Me")) {
                    // Enclosing_Mark
                    for (current_code[0]..current_code[1] + 1) |cp| try flat_map.put(@intCast(cp), 0);
                } else if (std.mem.eql(u8, field, "Mc")) {
                    // Spacing_Mark
                    for (current_code[0]..current_code[1] + 1) |cp| try flat_map.put(@intCast(cp), 0);
                } else if (std.mem.eql(u8, field, "Cf")) {
                    if (std.mem.indexOf(u8, line, "ARABIC") == null) {
                        // Format except Arabic
                        for (current_code[0]..current_code[1] + 1) |cp| try flat_map.put(@intCast(cp), 0);
                    }
                }
            },
            else => {},
        }
    }
} else |err| switch (err) {
    error.EndOfStream => {},
    else => {
        return err;
    },
}

var blocks_map = BlockMap.init(allocator);
defer blocks_map.deinit();

var stage1 = std.array_list.Managed(u16).init(allocator);
defer stage1.deinit();

var stage2 = std.array_list.Managed(i4).init(allocator);
defer stage2.deinit();

var block: Block = [_]i4{0} ** block_size;
var block_len: u16 = 0;

for (0..0x110000) |i| {
    const cp: u21 = @intCast(i);
    var width = flat_map.get(cp) orelse 1;

    // Specific overrides
    switch (cp) {
        // Three-em dash
        0x2e3b => width = 3,

        // C0/C1 control codes
        0...0x20 => width = if (options.c0_width) |c0| c0 else 0,
        0x80...0x9f => width = if (options.c1_width) |c1| c1 else 0,

        // Line separator
        0x2028,

        // Paragraph separator
        0x2029,

        // Hangul syllable and ignorable.
        0x1160...0x11ff,
        0xd7b0...0xd7ff,
        0x2060...0x206f,
        0xfff0...0xfff8,
        0xe0000...0xE0fff,
        => width = 0,

        // Two-em dash
        0x2e3a,

        // Regional indicators
        0x1f1e6...0x1f200,

        // CJK Blocks
        0x3400...0x4dbf, // CJK Unified Ideographs Extension A
        0x4e00...0x9fff, // CJK Unified Ideographs
        0xf900...0xfaff, // CJK Compatibility Ideographs
        0x20000...0x2fffd, // Plane 2
        0x30000...0x3fffd, // Plane 3
        => width = 2,

        else => {},
    }

    // ASCII
    if (0x20 <= cp and cp < 0x7f) width = 1;

    // Soft hyphen
    if (cp == 0xad) width = 1;

    // Backspace and delete
    if (cp == 0x8 or cp == 0x7f) width = if (options.c0_width) |c0| c0 else -1;

    // Process block
    block[block_len] = width;
    block_len += 1;

    if (block_len < block_size and cp != 0x10ffff) continue;

    const gop = try blocks_map.getOrPut(block);
    if (!gop.found_existing) {
        gop.value_ptr.* = @intCast(stage2.items.len);
        try stage2.appendSlice(&block);
    }

    try stage1.append(gop.value_ptr.*);
    block_len = 0;
}
// copyv: end

/// copyv: https://codeberg.org/atman/zg/src/commit/d9f596626e8ec05a9f3e47f7bc83aedd5bd2f989/src/DisplayWidth.zig#L103-L145 begin
/// strWidth returns the total display width of `str` as the number of cells
/// required in a fixed-pitch font (i.e. a terminal screen).
pub fn strWidth(dw: DisplayWidth, str: []const u8) usize {
    var total: isize = 0;

    // ASCII fast path
    if (ascii.isAsciiOnly(str)) {
        for (str) |b| total += dw.codePointWidth(b);
        return @intCast(@max(0, total));
    }

    var giter = dw.graphemes.iterator(str);

    while (giter.next()) |gc| {
        var cp_iter = CodePointIterator{ .bytes = gc.bytes(str) };
        var gc_total: isize = 0;

        while (cp_iter.next()) |cp| {
            var w = dw.codePointWidth(cp.code);

            if (w != 0) {
                // Handle text emoji sequence.
                if (cp_iter.next()) |ncp| {
                    // emoji text sequence.
                    if (ncp.code == 0xFE0E) w = 1;
                    if (ncp.code == 0xFE0F) w = 2;
                    // Skin tones
                    if (0x1F3FB <= ncp.code and ncp.code <= 0x1F3FF) w = 2;
                }

                // Only adding width of first non-zero-width code point.
                if (gc_total == 0) {
                    gc_total = w;
                    break;
                }
            }
        }

        total += gc_total;
    }

    return @intCast(@max(0, total));
}
// copyv: end
