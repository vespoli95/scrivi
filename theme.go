package main

import (
	"image/color"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/theme"
)

const defaultTheme = "Pastel"

var appThemes = []string{"Pastel", "Ocean", "Sage", "Rose", "Light"}
var categoryColors = []string{"Default", "Lavender", "Ocean", "Sage", "Rose", "Amber"}

type themePalette struct {
	background, surface, input, button, overlay, foreground, muted, primary, onPrimary, border color.NRGBA
	light                                                                                      bool
}

func rgb(r, g, b uint8) color.NRGBA { return color.NRGBA{R: r, G: g, B: b, A: 255} }

var palettes = map[string]themePalette{
	"Pastel": {rgb(29, 29, 38), rgb(42, 41, 54), rgb(53, 51, 67), rgb(63, 60, 77), rgb(39, 38, 51), rgb(241, 237, 246), rgb(173, 166, 188), rgb(186, 171, 218), rgb(29, 25, 38), rgb(83, 77, 98), false},
	"Ocean":  {rgb(24, 31, 39), rgb(34, 46, 57), rgb(44, 58, 70), rgb(53, 68, 80), rgb(31, 42, 52), rgb(232, 241, 245), rgb(163, 183, 192), rgb(151, 199, 215), rgb(22, 36, 41), rgb(71, 94, 105), false},
	"Sage":   {rgb(27, 33, 30), rgb(39, 49, 43), rgb(49, 61, 54), rgb(58, 72, 63), rgb(36, 46, 40), rgb(235, 242, 235), rgb(170, 187, 172), rgb(170, 203, 174), rgb(25, 37, 27), rgb(77, 100, 82), false},
	"Rose":   {rgb(36, 28, 33), rgb(51, 39, 46), rgb(64, 49, 57), rgb(76, 58, 67), rgb(48, 36, 43), rgb(246, 235, 240), rgb(192, 170, 180), rgb(220, 170, 190), rgb(40, 24, 31), rgb(102, 77, 89), false},
	"Light":  {rgb(247, 245, 249), rgb(235, 231, 240), rgb(255, 253, 255), rgb(225, 219, 234), rgb(250, 248, 252), rgb(44, 38, 52), rgb(103, 95, 112), rgb(111, 84, 146), rgb(255, 255, 255), rgb(181, 171, 191), true},
}

var darkCategoryColors = map[string]color.NRGBA{
	"Lavender": rgb(61, 52, 74),
	"Ocean":    rgb(36, 61, 70),
	"Sage":     rgb(45, 64, 51),
	"Rose":     rgb(72, 48, 60),
	"Amber":    rgb(71, 60, 42),
}

var lightCategoryColors = map[string]color.NRGBA{
	"Lavender": rgb(231, 222, 241),
	"Ocean":    rgb(216, 234, 239),
	"Sage":     rgb(222, 236, 221),
	"Rose":     rgb(242, 222, 231),
	"Amber":    rgb(242, 232, 213),
}

func validAppTheme(name string) bool {
	_, ok := palettes[name]
	return ok
}

func validCategoryColor(name string) bool {
	if name == "" {
		return true
	}
	_, ok := darkCategoryColors[name]
	return ok
}

type pastelTheme struct{ name string }

func (t pastelTheme) Color(name fyne.ThemeColorName, _ fyne.ThemeVariant) color.Color {
	p := palettes[t.name]
	switch name {
	case theme.ColorNameBackground:
		return p.background
	case theme.ColorNameHeaderBackground:
		return p.surface
	case theme.ColorNameInputBackground:
		return p.input
	case theme.ColorNameButton, theme.ColorNameDisabledButton, theme.ColorNameMenuBackground:
		return p.button
	case theme.ColorNameOverlayBackground:
		return p.overlay
	case theme.ColorNameForeground:
		return p.foreground
	case theme.ColorNameDisabled, theme.ColorNamePlaceHolder:
		return p.muted
	case theme.ColorNamePrimary:
		return p.primary
	case theme.ColorNameForegroundOnPrimary:
		return p.onPrimary
	case theme.ColorNameInputBorder, theme.ColorNameSeparator:
		return p.border
	case theme.ColorNameFocus, theme.ColorNameSelection:
		c := p.primary
		c.A = 70
		return c
	}
	variant := theme.VariantDark
	if p.light {
		variant = theme.VariantLight
	}
	return theme.DefaultTheme().Color(name, variant)
}

func (pastelTheme) Font(style fyne.TextStyle) fyne.Resource { return theme.DefaultTheme().Font(style) }
func (pastelTheme) Icon(name fyne.ThemeIconName) fyne.Resource {
	return theme.DefaultTheme().Icon(name)
}
func (pastelTheme) Size(name fyne.ThemeSizeName) float32 { return theme.DefaultTheme().Size(name) }

func categoryColor(name, appTheme string) color.Color {
	if name == "" {
		return theme.Color(theme.ColorNameHeaderBackground)
	}
	if palettes[appTheme].light {
		return lightCategoryColors[name]
	}
	return darkCategoryColors[name]
}
