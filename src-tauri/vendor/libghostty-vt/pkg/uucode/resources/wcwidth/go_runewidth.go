// copyv: https://github.com/mattn/go-runewidth/blob/7770d045cdc691f0fcb87b0364a83f0de2d1a421/runewidth.go#L115-L156 begin
// RuneWidth returns the number of cells in r.
// See http://www.unicode.org/reports/tr11/
func (c *Condition) RuneWidth(r rune) int {
	if r < 0 || r > 0x10FFFF {
		return 0
	}
	if len(c.combinedLut) > 0 {
		return int(c.combinedLut[r>>1]>>(uint(r&1)*4)) & 3
	}
	// optimized version, verified by TestRuneWidthChecksums()
	if !c.EastAsianWidth {
		switch {
		case r < 0x20:
			return 0
		case (r >= 0x7F && r <= 0x9F) || r == 0xAD: // nonprint
			return 0
		case r < 0x300:
			return 1
		case inTable(r, narrow):
			return 1
		case inTables(r, nonprint, combining):
			return 0
		case inTable(r, doublewidth):
			return 2
		default:
			return 1
		}
	} else {
		switch {
		case inTables(r, nonprint, combining):
			return 0
		case inTable(r, narrow):
			return 1
		case inTables(r, ambiguous, doublewidth):
			return 2
		case !c.StrictEmojiNeutral && inTables(r, ambiguous, emoji, narrow):
			return 2
		default:
			return 1
		}
	}
}
// copyv: end

// copyv: https://github.com/mattn/go-runewidth/blob/7770d045cdc691f0fcb87b0364a83f0de2d1a421/runewidth.go#L179-L193 begin
// StringWidth return width as you can see
func (c *Condition) StringWidth(s string) (width int) {
	g := graphemes.FromString(s)
	for g.Next() {
		var chWidth int
		for _, r := range g.Value() {
			chWidth = c.RuneWidth(r)
			if chWidth > 0 {
				break // Our best guess at this point is to use the width of the first non-zero-width rune.
			}
		}
		width += chWidth
	}
	return
}
// copyv: end
