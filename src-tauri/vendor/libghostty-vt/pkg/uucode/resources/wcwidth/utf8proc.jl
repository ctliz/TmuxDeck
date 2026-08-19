# copyv: https://github.com/JuliaStrings/utf8proc/blob/e5e799221b45bbb90f5fdc5c69b6b8dfbf017e78/data/data_generator.jl#L202-L249 begin
let ea_widths = read_east_asian_widths("EastAsianWidth.txt")
    # Following work by @jiahao, we compute character widths using a combination of
    #   * character category
    #   * UAX 11: East Asian Width
    #   * a few exceptions as needed
    # Adapted from http://nbviewer.ipython.org/gist/jiahao/07e8b08bf6d8671e9734
    global function derive_char_width(code, category)
        # Use a default width of 1 for all character categories that are
        # letter/symbol/number-like, as well as for unassigned/private-use chars.
        # This provides a useful nonzero fallback for new codepoints when a new
        # Unicode version has been released.
        width = 1

        # Various zero-width categories
        #
        # "Sk" not included in zero width - see issue #167
        if category in ("Mn", "Mc", "Me", "Zl", "Zp", "Cc", "Cf", "Cs")
            width = 0
        end

        # Widths from UAX #11: East Asian Width
        eaw = get(ea_widths, code, nothing)
        if !isnothing(eaw)
            width = eaw < 0 ? 1 : eaw
        end

        # A few exceptional cases, found by manual comparison to other wcwidth
        # functions and similar checks.
        if category == "Mn"
            width = 0
        end

        if code == 0x00ad
            # Soft hyphen is typically printed as a hyphen (-) in terminals.
            width = 1
        elseif code == 0x2028 || code == 0x2029
            #By definition, should have zero width (on the same line)
            #0x002028 '\u2028' category: Zl name: LINE SEPARATOR/
            #0x002029 '\u2029' category: Zp name: PARAGRAPH SEPARATOR/
            width = 0
        end

        return width
    end
    global function is_ambiguous_width(code)
        return get(ea_widths, code, 0) < 0
    end
end
# copyv: end
