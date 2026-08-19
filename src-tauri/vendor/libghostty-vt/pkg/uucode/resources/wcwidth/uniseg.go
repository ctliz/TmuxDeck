// copyv: https://github.com/rivo/uniseg/blob/087b3e4194c1feb0856b68d0e7c425c0994829cf/width.go#L3-L61 begin
// EastAsianAmbiguousWidth specifies the monospace width for East Asian
// characters classified as Ambiguous. The default is 1 but some rare fonts
// render them with a width of 2.
var EastAsianAmbiguousWidth = 1

// runeWidth returns the monospace width for the given rune. The provided
// grapheme property is a value mapped by the [graphemeCodePoints] table.
//
// Every rune has a width of 1, except for runes with the following properties
// (evaluated in this order):
//
//   - Control, CR, LF, Extend, ZWJ: Width of 0
//   - \u2e3a, TWO-EM DASH: Width of 3
//   - \u2e3b, THREE-EM DASH: Width of 4
//   - East-Asian width Fullwidth and Wide: Width of 2 (Ambiguous and Neutral
//     have a width of 1)
//   - Regional Indicator: Width of 2
//   - Extended Pictographic: Width of 2, unless Emoji Presentation is "No".
func runeWidth(r rune, graphemeProperty int) int {
	switch graphemeProperty {
	case prControl, prCR, prLF, prExtend, prZWJ:
		return 0
	case prRegionalIndicator:
		return 2
	case prExtendedPictographic:
		if property(emojiPresentation, r) == prEmojiPresentation {
			return 2
		}
		return 1
	}

	switch r {
	case 0x2e3a:
		return 3
	case 0x2e3b:
		return 4
	}

	switch propertyEastAsianWidth(r) {
	case prW, prF:
		return 2
	case prA:
		return EastAsianAmbiguousWidth
	}

	return 1
}

// StringWidth returns the monospace width for the given string, that is, the
// number of same-size cells to be occupied by the string.
func StringWidth(s string) (width int) {
	state := -1
	for len(s) > 0 {
		var w int
		_, s, w, state = FirstGraphemeClusterInString(s, state)
		width += w
	}
	return
}
// copyv: end

// copyv: https://github.com/rivo/uniseg/blob/087b3e4194c1feb0856b68d0e7c425c0994829cf/grapheme.go#L287-L345 begin
// FirstGraphemeClusterInString is like [FirstGraphemeCluster] but its input and
// outputs are strings.
func FirstGraphemeClusterInString(str string, state int) (cluster, rest string, width, newState int) {
	// An empty string returns nothing.
	if len(str) == 0 {
		return
	}

	// Extract the first rune.
	r, length := utf8.DecodeRuneInString(str)
	if len(str) <= length { // If we're already past the end, there is nothing else to parse.
		var prop int
		if state < 0 {
			prop = propertyGraphemes(r)
		} else {
			prop = state >> shiftGraphemePropState
		}
		return str, "", runeWidth(r, prop), grAny | (prop << shiftGraphemePropState)
	}

	// If we don't know the state, determine it now.
	var firstProp int
	if state < 0 {
		state, firstProp, _ = transitionGraphemeState(state, r)
	} else {
		firstProp = state >> shiftGraphemePropState
	}
	width += runeWidth(r, firstProp)

	// Transition until we find a boundary.
	for {
		var (
			prop     int
			boundary bool
		)

		r, l := utf8.DecodeRuneInString(str[length:])
		state, prop, boundary = transitionGraphemeState(state&maskGraphemeState, r)

		if boundary {
			return str[:length], str[length:], width, state | (prop << shiftGraphemePropState)
		}

		if firstProp == prExtendedPictographic {
			if r == vs15 {
				width = 1
			} else if r == vs16 {
				width = 2
			}
		} else if firstProp != prRegionalIndicator && firstProp != prL {
			width += runeWidth(r, prop)
		}

		length += l
		if len(str) <= length {
			return str, "", width, grAny | (prop << shiftGraphemePropState)
		}
	}
}
// copyv: end
