package main

import (
	"image/color"
	"math"
	"testing"
)

func contrast(a, b color.Color) float64 {
	luminance := func(c color.Color) float64 {
		r, g, b, _ := c.RGBA()
		component := func(v uint32) float64 {
			s := float64(v) / 65535
			if s <= 0.04045 {
				return s / 12.92
			}
			return math.Pow((s+0.055)/1.055, 2.4)
		}
		return 0.2126*component(r) + 0.7152*component(g) + 0.0722*component(b)
	}
	x, y := luminance(a), luminance(b)
	return (math.Max(x, y) + 0.05) / (math.Min(x, y) + 0.05)
}

func TestThemePresetContrast(t *testing.T) {
	for _, name := range appThemes {
		p := palettes[name]
		for _, background := range []color.Color{p.background, p.surface, p.input, p.button, p.overlay} {
			if ratio := contrast(p.foreground, background); ratio < 4.5 {
				t.Errorf("%s text contrast %.2f is too low", name, ratio)
			}
		}
		if ratio := contrast(p.onPrimary, p.primary); ratio < 4.5 {
			t.Errorf("%s primary button contrast %.2f is too low", name, ratio)
		}
		for _, category := range categoryColors[1:] {
			if ratio := contrast(p.foreground, categoryColor(category, name)); ratio < 4.5 {
				t.Errorf("%s text on %s category contrast %.2f is too low", name, category, ratio)
			}
		}
	}
}
