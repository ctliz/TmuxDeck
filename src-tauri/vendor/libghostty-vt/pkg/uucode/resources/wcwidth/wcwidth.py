# copyv: https://github.com/jquast/wcwidth/blob/c563bfb8ddffa88684c7c4ee92ab013d590e5a3f/wcwidth/wcwidth.py#L89-L206 begin
@lru_cache(maxsize=1000)
def wcwidth(wc, unicode_version='auto'):
    r"""
    Given one Unicode codepoint, return its printable length on a terminal.

    :param str wc: A single Unicode character.
    :param str unicode_version: A Unicode version number, such as
        ``'6.0.0'``. A list of version levels suported by wcwidth
        is returned by :func:`list_versions`.

        Any version string may be specified without error -- the nearest
        matching version is selected.  When ``'auto'`` (default), the
        ``UNICODE_VERSION`` environment variable is used if set, otherwise
        the highest Unicode version level is used.

        .. deprecated:: 0.3.0

            This parameter is deprecated. Empirical data shows that Unicode
            support in terminals varies not only by unicode version, but
            by capabilities, Emojis, and specific language support.

            The default ``'auto'`` behavior is recommended for all use cases.

    :return: The width, in cells, necessary to display the character of
        Unicode string character, ``wc``.  Returns 0 if the ``wc`` argument has
        no printable effect on a terminal (such as NUL '\0'), -1 if ``wc`` is
        not printable, or has an indeterminate effect on the terminal, such as
        a control character.  Otherwise, the number of column positions the
        character occupies on a graphic terminal (1 or 2) is returned.
    :rtype: int

    See :ref:`Specification` for details of cell measurement.
    """
    ucs = ord(wc) if wc else 0

    # small optimization: early return of 1 for printable ASCII, this provides
    # approximately 40% performance improvement for mostly-ascii documents, with
    # less than 1% impact to others.
    if 32 <= ucs < 0x7f:
        return 1

    # C0/C1 control characters are -1 for compatibility with POSIX-like calls
    if ucs and ucs < 32 or 0x07F <= ucs < 0x0A0:
        return -1

    _unicode_version = _wcmatch_version(unicode_version)

    # Zero width
    if _bisearch(ucs, ZERO_WIDTH[_unicode_version]):
        return 0

    # 1 or 2 width
    return 1 + _bisearch(ucs, WIDE_EASTASIAN[_unicode_version])


def wcswidth(pwcs, n=None, unicode_version='auto'):
    """
    Given a unicode string, return its printable length on a terminal.

    :param str pwcs: Measure width of given unicode string.
    :param int n: When ``n`` is None (default), return the length of the entire
        string, otherwise only the first ``n`` characters are measured. This
        argument exists only for compatibility with the C POSIX function
        signature. It is suggested instead to use python's string slicing
        capability, ``wcswidth(pwcs[:n])``
    :param str unicode_version: A Unicode version number, such as
        ``'6.0.0'``, or ``'auto'`` (default) which uses the
        ``UNICODE_VERSION`` environment variable if defined, or the latest
        available unicode version otherwise.

        .. deprecated:: 0.3.0

            This parameter is deprecated. Empirical data shows that Unicode
            support in terminals varies not only by unicode version, but
            by capabilities, Emojis, and specific language support.

            The default ``'auto'`` behavior is recommended for all use cases.

    :rtype: int
    :returns: The width, in cells, needed to display the first ``n`` characters
        of the unicode string ``pwcs``.  Returns ``-1`` for C0 and C1 control
        characters!

    See :ref:`Specification` for details of cell measurement.
    """
    # this 'n' argument is a holdover for POSIX function
    _unicode_version = None
    end = len(pwcs) if n is None else n
    total_width = 0
    idx = 0
    last_measured_idx = -2  # Track index of last measured char for VS16
    while idx < end:
        char = pwcs[idx]
        if char == '\u200D':
            # Zero Width Joiner, do not measure this or next character
            idx += 2
            continue
        if char == '\uFE0F' and last_measured_idx >= 0:
            # VS16 following a measured character: add 1 if that character is
            # known to be converted from narrow to wide by VS16.
            if _unicode_version is None:
                _unicode_version = _wcversion_value(_wcmatch_version(unicode_version))
            if _unicode_version >= (9, 0, 0):
                total_width += _bisearch(ord(pwcs[last_measured_idx]),
                                         VS16_NARROW_TO_WIDE["9.0.0"])
            last_measured_idx = -2  # Prevent double application
            idx += 1
            continue
        # measure character at current index
        wcw = wcwidth(char, unicode_version)
        if wcw < 0:
            # early return -1 on C0 and C1 control characters
            return wcw
        if wcw > 0:
            last_measured_idx = idx
        total_width += wcw
        idx += 1
    return total_width
# copyv: end

# copyv: https://github.com/jquast/wcwidth/blob/c563bfb8ddffa88684c7c4ee92ab013d590e5a3f/bin/update-tables.py#L131-L169 begin
@dataclass(frozen=True)
class TableEntry:
    """An entry of a unicode table."""
    code_range: tuple[int, int] | None
    properties: tuple[str, ...]
    comment: str

    def filter_by_category_width(self, wide: int) -> bool:
        """
        Return whether entry matches displayed width.

        Parses both DerivedGeneralCategory.txt and EastAsianWidth.txt
        """
        if self.code_range is None:
            return False
        elif self.properties[0] == 'Sk':
            if 'EMOJI MODIFIER' in self.comment:
                # These codepoints are fullwidth when used without emoji, 0-width with.
                # Generate code that expects the best case, that is always combined
                return wide == 0
            elif 'FULLWIDTH' in self.comment:
                # Some codepoints in 'Sk' categories are fullwidth(!)
                # at this time just 3, FULLWIDTH: CIRCUMFLEX ACCENT, GRAVE ACCENT, and MACRON
                return wide == 2
            else:
                # the rest are narrow
                return wide == 1
        # Me Enclosing Mark
        # Mn Nonspacing Mark
        # Cf Format
        # Zl Line Separator
        # Zp Paragraph Separator
        if self.properties[0] in ('Me', 'Mn', 'Mc', 'Cf', 'Zl', 'Zp'):
            return wide == 0
        # F  Fullwidth
        # W  Wide
        if self.properties[0] in ('W', 'F'):
            return wide == 2
        return wide == 1
# copyv: end

# copyv: https://github.com/jquast/wcwidth/blob/c563bfb8ddffa88684c7c4ee92ab013d590e5a3f/bin/update-tables.py#L365-L420 begin
def fetch_table_wide_data() -> UnicodeTableRenderCtx:
    """Fetch east-asian tables."""
    table: dict[UnicodeVersion, TableDef] = {}
    for version in fetch_unicode_versions():
        # parse typical 'wide' characters by categories 'W' and 'F',
        table[version] = parse_category(fname=UnicodeDataFile.EastAsianWidth(version),
                                        wide=2)

        # subtract(!) wide characters that were defined above as 'W' category in EastAsianWidth,
        # but also zero-width category 'Mn' or 'Mc' in DerivedGeneralCategory!
        table[version].values = table[version].values.difference(parse_category(
            fname=UnicodeDataFile.DerivedGeneralCategory(version),
            wide=0).values)

        # Also subtract Hangul Jamo Vowels and Hangul Trailing Consonants
        table[version].values = table[version].values.difference(HANGUL_JAMO_ZEROWIDTH)

        # finally, join with atypical 'wide' characters defined by category 'Sk',
        table[version].values.update(parse_category(fname=UnicodeDataFile.DerivedGeneralCategory(version),
                                                    wide=2).values)
    return UnicodeTableRenderCtx('WIDE_EASTASIAN', table)


def fetch_table_zero_data() -> UnicodeTableRenderCtx:
    """
    Fetch zero width tables.

    See also: https://unicode.org/L2/L2002/02368-default-ignorable.html
    """
    table: dict[UnicodeVersion, TableDef] = {}
    for version in fetch_unicode_versions():
        # Determine values of zero-width character lookup table by the following category codes
        table[version] = parse_category(fname=UnicodeDataFile.DerivedGeneralCategory(version),
                                        wide=0)

        # Include NULL
        table[version].values.add(0)

        # Add Hangul Jamo Vowels and Hangul Trailing Consonants
        table[version].values.update(HANGUL_JAMO_ZEROWIDTH)

        # Remove u+00AD categoryCode=Cf name="SOFT HYPHEN",
        # > https://www.unicode.org/faq/casemap_charprop.html
        #
        # > Q: Unicode now treats the SOFT HYPHEN as format control (Cf)
        # > character when formerly it was a punctuation character (Pd).
        # > Doesn't this break ISO 8859-1 compatibility?
        #
        # > [..] In a terminal emulation environment, particularly in
        # > ISO-8859-1 contexts, one could display the SOFT HYPHEN as a hyphen
        # > in all circumstances.
        #
        # This value was wrongly measured as a width of '0' in this wcwidth
        # versions 0.2.9 - 0.2.13. Fixed in 0.2.14
        table[version].values.discard(0x00AD)  # SOFT HYPHEN
    return UnicodeTableRenderCtx('ZERO_WIDTH', table)
# copyv: end
